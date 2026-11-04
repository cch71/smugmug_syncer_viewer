//! Full size photo/video viewer with previous/next navigation and an info panel

use dioxus::prelude::*;

use super::{AppState, Route, browse::Missing, use_archive};
use crate::{
    archive::{
        file_info::{MergedField, Source, TrackKind, merge_fields, summarize},
        human_size, short_date,
    },
    platform,
};

#[component]
pub fn PhotoView(album_key: String, image_key: String) -> Element {
    let archive = use_archive();
    let images = archive.album_images(&album_key);
    let Some(pos) = images.iter().position(|v| v.image_key == image_key) else {
        return rsx! { Missing { what: "photo" } };
    };
    let image = images[pos].clone();
    let route_at = |i: usize| Route::PhotoView {
        album_key: album_key.clone(),
        image_key: images[i].image_key.clone(),
    };
    let prev = pos.checked_sub(1).map(route_at);
    let next = (pos + 1 < images.len()).then(|| route_at(pos + 1));
    let back = Route::AlbumView {
        album_key: album_key.clone(),
    };

    let mut show_info = use_signal(|| true);
    let (prev_key, next_key, back_key) = (prev.clone(), next.clone(), back.clone());
    let onkeydown = move |e: KeyboardEvent| {
        let target = match e.key() {
            Key::ArrowLeft => prev_key.clone(),
            Key::ArrowRight => next_key.clone(),
            Key::Escape => Some(back_key.clone()),
            Key::Character(c) if c == "i" => {
                show_info.toggle();
                None
            }
            _ => None,
        };
        if let Some(route) = target {
            e.prevent_default();
            navigator().replace(route);
        }
    };

    let media = if image.is_video {
        let ext = archive
            .videos
            .get(&image.image_key)
            .map_or("mp4", |v| v.ext.as_str());
        rsx! {
            video {
                key: "{image.image_key}",
                class: "media",
                src: platform::video_url(&image.image_key, ext),
                poster: platform::image_url(&image.image_key),
                controls: true,
                preload: "metadata",
            }
        }
    } else {
        rsx! {
            img {
                key: "{image.image_key}",
                class: "media",
                src: platform::image_url(&image.image_key),
                alt: "{image.display_name()}",
            }
        }
    };

    rsx! {
        div {
            class: "viewer",
            tabindex: "0",
            onkeydown,
            onmounted: move |e| async move {
                let _ = e.set_focus(true).await;
            },
            div { class: "viewer-stage",
                div { class: "viewer-bar",
                    Link { class: "viewer-button", to: back, "✕ Close" }
                    span { class: "viewer-title", "{image.display_name()}" }
                    span { class: "muted", "{pos + 1} / {images.len()}" }
                    button {
                        class: "viewer-button",
                        onclick: move |_| show_info.toggle(),
                        "ⓘ Info"
                    }
                }
                div { class: "viewer-media",
                    {media}
                    if let Some(route) = prev {
                        Link { class: "nav prev", to: route, "‹" }
                    }
                    if let Some(route) = next {
                        Link { class: "nav next", to: route, "›" }
                    }
                }
            }
            if show_info() {
                InfoPanel { key: "{image_key}", album_key: album_key.clone(), image_key: image_key.clone() }
            }
        }
    }
}

#[component]
fn InfoPanel(album_key: String, image_key: String) -> Element {
    let archive = use_archive();
    let state = use_context::<AppState>();
    let Some(image) = archive
        .album_images(&album_key)
        .iter()
        .find(|v| v.image_key == image_key)
        .cloned()
    else {
        return rsx! {};
    };
    let details = state
        .details
        .read()
        .as_ref()
        .and_then(|v| v.get(&image_key).cloned());
    let video = archive.videos.get(&image_key).cloned();
    let video_ext = image
        .is_video
        .then(|| video.as_ref().map_or("mp4".to_string(), |v| v.ext.clone()));

    // What the file itself records (EXIF, or a video's codecs), read when the panel opens.
    // The panel is keyed by image, so this never needs to re-run.
    let file_info = use_resource({
        let (location, key) = (archive.location.clone(), image_key.clone());
        move || {
            let (location, key, ext) = (location.clone(), key.clone(), video_ext.clone());
            async move {
                platform::load_file_info(location, key, ext)
                    .await
                    .map_err(|e| format!("{e:#}"))
            }
        }
    });
    let (file, file_error) = match &*file_info.read() {
        Some(Ok(info)) => (Some(info.clone()), None),
        Some(Err(err)) => (None, Some(err.clone())),
        None => (None, None),
    };

    // SmugMug's full metadata, loaded on request and merged with the file's fields
    let mut smugmug_fields = use_signal(|| None::<Result<Vec<(String, String)>, String>>);
    let location = archive.location.clone();
    let source_file = details.as_ref().and_then(|v| v.source_file.clone());
    let key = image_key.clone();
    let load_all = move |_| {
        let (location, key, source_file) = (location.clone(), key.clone(), source_file.clone());
        async move {
            let result = platform::load_metadata_json(location, key, source_file)
                .await
                .map_err(|e| format!("{e:#}"))
                .map(|json| json.map(|v| flatten_json(&v)).unwrap_or_default());
            smugmug_fields.set(Some(result));
        }
    };

    let summary = summarize(details.as_ref(), file.as_ref());
    let taken_label = if image.is_video { "Recorded" } else { "Taken" };
    let dimensions = summary
        .dimensions
        .or_else(|| video.as_ref().and_then(|v| Some((v.width?, v.height?))));
    let size = details
        .as_ref()
        .and_then(|d| d.original_size)
        .or(image.archived_size);
    let media = file.as_ref().and_then(|f| f.media.clone());
    let duration = video
        .as_ref()
        .and_then(|v| v.duration.clone())
        .or_else(|| media.as_ref()?.duration_secs.map(|v| format!("{v:.1}")));

    rsx! {
        aside { class: "info",
            h2 { "{image.display_name()}" }
            if !image.caption.is_empty() {
                p { class: "caption", "{image.caption}" }
            }
            dl {
                Row { label: "File", value: image.file_name.clone() }
                if let Some(taken) = &summary.taken {
                    Row { label: taken_label, value: short_date(taken) }
                }
                if let Some(uploaded) = &image.date_time_uploaded {
                    Row { label: "Uploaded", value: short_date(uploaded) }
                }
                if let Some((w, h)) = dimensions {
                    Row { label: "Dimensions", value: format!("{w} × {h}") }
                }
                if let Some(size) = size {
                    Row { label: "Size", value: human_size(size) }
                }
                if let Some(duration) = duration {
                    Row { label: "Duration", value: format!("{duration} s") }
                }
                if let Some(media) = &media {
                    for track in media.tracks.iter() {
                        Row {
                            label: match track.kind {
                                TrackKind::Video => "Video",
                                TrackKind::Audio => "Audio",
                                TrackKind::Other(_) => "Track",
                            },
                            value: track.summary(),
                        }
                    }
                    if media.track(TrackKind::Audio).is_none() {
                        Row { label: "Audio", value: "None".to_string() }
                    }
                }
                if let Some(camera) = &summary.camera {
                    Row { label: "Camera", value: camera.clone() }
                }
                if let Some(lens) = &summary.lens {
                    Row { label: "Lens", value: lens.clone() }
                }
                if let Some(exposure) = &summary.exposure {
                    Row { label: "Exposure", value: exposure.clone() }
                }
                if let Some((lat, lon)) = summary.location {
                    dt { "Location" }
                    dd {
                        a {
                            href: "https://www.openstreetmap.org/?mlat={lat}&mlon={lon}#map=15/{lat}/{lon}",
                            target: "_blank",
                            "{lat:.5}, {lon:.5}"
                        }
                    }
                }
                Row { label: "Format", value: image.format.clone() }
                Row { label: "Image key", value: image.image_key.clone() }
                if let Some(md5) = &image.archived_md5 {
                    Row { label: "MD5", value: md5.clone() }
                }
                if image.hidden {
                    Row { label: "Hidden", value: "Yes".to_string() }
                }
            }
            if let Some(err) = file_error {
                p { class: "error small", "Couldn't read the file: {err}" }
            }
            if !image.keyword_array.is_empty() {
                h3 { "Keywords" }
                div { class: "chips",
                    for keyword in image.keyword_array.iter() {
                        Link {
                            class: "chip",
                            to: Route::SearchView { query: keyword.clone() },
                            "{keyword}"
                        }
                    }
                }
            }
            h3 { "In albums" }
            ul { class: "albums",
                for key in archive.albums_containing(&image_key).iter() {
                    if let Some((_, node)) = archive.album_node(key) {
                        li {
                            Link {
                                to: Route::AlbumView { album_key: key.clone() },
                                "{node.display_name()}"
                            }
                        }
                    }
                }
            }
            match &*smugmug_fields.read() {
                None => rsx! {
                    if platform::HAS_FULL_METADATA {
                        button { class: "secondary", onclick: load_all, "Show all metadata" }
                    }
                },
                Some(Err(err)) => rsx! { p { class: "error", "{err}" } },
                Some(Ok(smugmug)) => {
                    let file_fields = file.as_ref().map_or(&[][..], |f| f.fields.as_slice());
                    let merged = merge_fields(smugmug, file_fields);
                    rsx! {
                        h3 { "Metadata" }
                        p { class: "muted small legend",
                            span { class: "source smugmug", "S" } " SmugMug  "
                            span { class: "source file", "F" } " file  "
                            span { class: "source both", "S+F" } " both agree"
                        }
                        if merged.is_empty() {
                            p { class: "muted small", "Neither SmugMug nor the file has any metadata." }
                        }
                        dl { class: "exif",
                            for field in merged {
                                MetadataRow { field }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Row(label: &'static str, value: String) -> Element {
    rsx! {
        dt { "{label}" }
        dd { "{value}" }
    }
}

#[component]
fn MetadataRow(field: MergedField) -> Element {
    let (class, tag) = match field.source {
        Source::SmugMug => ("source smugmug", "S"),
        Source::File => ("source file", "F"),
        Source::Both => ("source both", "S+F"),
    };
    rsx! {
        dt { "{field.name}" }
        dd {
            span { class, "{tag}" }
            "{field.value}"
            if let Some(file_value) = &field.file_value {
                div { class: "file-value",
                    span { class: "source file", "F" }
                    "{file_value}"
                }
            }
        }
    }
}

/// Turns the stored metadata JSON object into key/value pairs, dropping empty values
fn flatten_json(json: &str) -> Vec<(String, String)> {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str(json) else {
        return vec![("metadata".to_string(), json.to_string())];
    };
    map.into_iter()
        .filter(|(k, _)| k != "Uri")
        .map(|(k, v)| {
            let v = match v {
                serde_json::Value::String(s) => s,
                other => other.to_string(),
            };
            (k, v)
        })
        .filter(|(_, v)| !v.is_empty())
        .collect()
}
