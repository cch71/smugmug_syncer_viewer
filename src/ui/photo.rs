//! Full size photo/video viewer with previous/next navigation and an info panel

use dioxus::prelude::*;

use super::{AppState, Route, browse::Missing, use_archive};
use crate::{
    archive::{human_size, short_date},
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

    let mut exif = use_signal(|| None::<Result<Vec<(String, String)>, String>>);
    let location = archive.location.clone();
    let source_file = details.as_ref().and_then(|v| v.source_file.clone());
    let key = image_key.clone();
    let load_exif = move |_| {
        let (location, key, source_file) = (location.clone(), key.clone(), source_file.clone());
        async move {
            let result = platform::load_metadata_json(location, key, source_file)
                .await
                .map_err(|e| format!("{e:#}"))
                .map(|json| json.map(|v| flatten_json(&v)).unwrap_or_default());
            exif.set(Some(result));
        }
    };

    let dimensions = details
        .as_ref()
        .and_then(|d| Some((d.original_width?, d.original_height?)))
        .or_else(|| video.as_ref().and_then(|v| Some((v.width?, v.height?))));
    let camera = details.as_ref().and_then(|d| {
        let parts: Vec<&str> = [d.make.as_deref(), d.model.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        (!parts.is_empty()).then(|| parts.join(" "))
    });
    let gps = details
        .as_ref()
        .and_then(|d| Some((d.latitude?, d.longitude?)));
    let size = details
        .as_ref()
        .and_then(|d| d.original_size)
        .or(image.archived_size);

    rsx! {
        aside { class: "info",
            h2 { "{image.display_name()}" }
            if !image.caption.is_empty() {
                p { class: "caption", "{image.caption}" }
            }
            dl {
                Row { label: "File", value: image.file_name.clone() }
                if let Some(taken) = details.as_ref().and_then(|d| d.date_time_original.clone()) {
                    Row { label: "Taken", value: short_date(&taken) }
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
                if let Some(video) = &video {
                    if let Some(duration) = &video.duration {
                        Row { label: "Duration", value: format!("{duration} s") }
                    }
                }
                if let Some(camera) = camera {
                    Row { label: "Camera", value: camera }
                }
                if let Some((lat, lon)) = gps {
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
            match &*exif.read() {
                None => rsx! {
                    if platform::HAS_FOLDER_PICKER {
                        button { class: "secondary", onclick: load_exif, "Show all metadata" }
                    }
                },
                Some(Err(err)) => rsx! { p { class: "error", "{err}" } },
                Some(Ok(fields)) if fields.is_empty() => rsx! {
                    p { class: "muted small", "SmugMug had no further metadata for this file." }
                },
                Some(Ok(fields)) => rsx! {
                    h3 { "Metadata" }
                    dl { class: "exif",
                        for (k, v) in fields.iter() {
                            dt { "{k}" }
                            dd { "{v}" }
                        }
                    }
                },
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

/// Turns the stored metadata JSON object into sorted key/value pairs for display
fn flatten_json(json: &str) -> Vec<(String, String)> {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str(json) else {
        return vec![("metadata".to_string(), json.to_string())];
    };
    let mut fields: Vec<_> = map
        .into_iter()
        .map(|(k, v)| {
            let v = match v {
                serde_json::Value::String(s) => s,
                other => other.to_string(),
            };
            (k, v)
        })
        .filter(|(_, v)| !v.is_empty())
        .collect();
    fields.sort();
    fields
}
