//! Home, folder and album pages, and the cards and thumbnail grid they share

use std::sync::Arc;

use dioxus::prelude::*;

use super::{Route, use_archive};
use crate::{
    archive::{human_size, model::Image, short_date},
    platform,
};

/// Thumbnails rendered per "page" of a grid; more are added with "Show more"
const GRID_PAGE: usize = 240;

#[component]
pub fn HomeView() -> Element {
    let archive = use_archive();
    let props = &archive.props;
    let folders = archive.nodes.len() - archive.album_count();

    rsx! {
        section { class: "page",
            h1 { "Archive" }
            if let Some(notice) = &archive.db_notice {
                p { class: "error small", "{notice}" }
            }
            div { class: "stats",
                Stat { label: "Folders", value: folders.to_string() }
                Stat { label: "Albums", value: archive.album_count().to_string() }
                Stat { label: "Photos & videos", value: archive.image_count().to_string() }
                Stat { label: "Videos", value: archive.video_count().to_string() }
            }
            p { class: "muted small",
                if let Some(ts) = &props.album_node_last_sync_ts {
                    "Folders synced {short_date(ts)} UTC. "
                }
                if let Some(ts) = &props.image_map_last_sync_ts {
                    "Image list synced {short_date(ts)} UTC. "
                }
                if let Some(version) = &props.db_version {
                    "Database version {version}."
                }
            }
            NodeCards { indices: archive.nodes[archive.root].children.clone() }
        }
    }
}

#[component]
fn Stat(label: &'static str, value: String) -> Element {
    rsx! {
        div { class: "stat",
            div { class: "stat-value", "{value}" }
            div { class: "stat-label", "{label}" }
        }
    }
}

#[component]
pub fn FolderView(node_id: String) -> Element {
    let archive = use_archive();
    let Some((idx, node)) = archive.node(&node_id) else {
        return rsx! { Missing { what: "folder" } };
    };

    rsx! {
        section { class: "page",
            Breadcrumbs { idx }
            h1 { "{node.display_name()}" }
            p { class: "muted small",
                "{node.children.len()} items · {node.total_images} photos & videos"
            }
            NodeCards { indices: node.children.clone() }
        }
    }
}

#[component]
pub fn AlbumView(album_key: String) -> Element {
    let archive = use_archive();
    let Some((idx, node)) = archive.album_node(&album_key) else {
        return rsx! { Missing { what: "album" } };
    };
    let album = node.album.clone().unwrap_or_default();
    let images = archive.album_images(&album_key).to_vec();

    rsx! {
        section { class: "page",
            Breadcrumbs { idx }
            div { class: "album-header",
                h1 { "{node.display_name()}" }
                p { class: "muted small",
                    "{images.len()} photos & videos"
                    if let Some(size) = album.original_sizes {
                        " · {human_size(size)}"
                    }
                    if let Some(date) = &album.date {
                        " · created {short_date(date)}"
                    }
                    if let Some(date) = &album.images_last_updated {
                        " · updated {short_date(date)}"
                    }
                    if !album.privacy.is_empty() {
                        " · {album.privacy}"
                    }
                }
                if !album.description.is_empty() {
                    p { class: "description", "{album.description}" }
                }
            }
            if images.is_empty() {
                p { class: "empty", "This album has no images in the archive." }
            }
            ImageGrid { key: "{album_key}", album_key: album_key.clone(), images }
        }
    }
}

/// Grid of folder/album cards with cover thumbnails
#[component]
pub fn NodeCards(indices: Vec<usize>) -> Element {
    let archive = use_archive();
    rsx! {
        div { class: "cards",
            for idx in indices {
                {
                    let node = &archive.nodes[idx];
                    let cover = archive.cover_image(idx).map(|v| platform::thumb_url(&v.image_key));
                    let kind = if node.album.is_some() { "Album" } else { "Folder" };
                    rsx! {
                        Link { key: "{idx}", class: "card", to: Route::for_node(&archive, idx),
                            div { class: "card-cover",
                                if let Some(src) = cover {
                                    img { src, loading: "lazy", decoding: "async", alt: "" }
                                } else {
                                    span { class: "card-icon", if node.album.is_some() { "▦" } else { "▤" } }
                                }
                            }
                            div { class: "card-body",
                                div { class: "card-title", "{node.display_name()}" }
                                div { class: "card-meta", "{kind} · {node.total_images}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Thumbnail grid that renders [`GRID_PAGE`] images at a time.  Pass `album_key` when all
/// images come from one album; otherwise each image is linked through `albums`.
#[component]
pub fn ImageGrid(
    images: Vec<Arc<Image>>,
    #[props(default)] album_key: String,
    #[props(default)] albums: Vec<String>,
) -> Element {
    let mut shown = use_signal(|| GRID_PAGE);
    let total = images.len();

    rsx! {
        div { class: "grid",
            for (i, image) in images.iter().take(shown()).enumerate() {
                Thumb {
                    key: "{image.image_key}-{i}",
                    image: image.clone(),
                    album_key: albums.get(i).cloned().unwrap_or_else(|| album_key.clone()),
                }
            }
        }
        if shown() < total {
            div { class: "more",
                button { onclick: move |_| shown += GRID_PAGE, "Show more ({total - shown()} left)" }
                button { class: "secondary", onclick: move |_| shown.set(total), "Show all" }
            }
        }
    }
}

#[component]
fn Thumb(image: Arc<Image>, album_key: String) -> Element {
    let src = platform::thumb_url(&image.image_key);
    let name = image.display_name().to_string();
    rsx! {
        Link {
            class: "tile",
            title: "{name}",
            to: Route::PhotoView { album_key, image_key: image.image_key.clone() },
            img { src, loading: "lazy", decoding: "async", alt: "{name}" }
            if image.is_video {
                span { class: "badge", "▶" }
            }
        }
    }
}

#[component]
pub fn Breadcrumbs(idx: usize) -> Element {
    let archive = use_archive();
    let path = archive.ancestry(idx);
    let last = path.len() - 1;
    rsx! {
        nav { class: "crumbs",
            for (i, node) in path.into_iter().enumerate() {
                if i > 0 {
                    span { class: "sep", "/" }
                }
                if i == last {
                    span { "{archive.nodes[node].display_name()}" }
                } else {
                    Link { to: Route::for_node(&archive, node), "{archive.nodes[node].display_name()}" }
                }
            }
        }
    }
}

#[component]
pub fn Missing(what: &'static str) -> Element {
    rsx! {
        div { class: "empty",
            h2 { "That {what} isn't in this archive" }
            Link { to: Route::HomeView {}, "Back to the archive" }
        }
    }
}
