//! Search results: matching folders/albums, then matching photos and videos

use dioxus::prelude::*;

use super::{
    browse::{ImageGrid, NodeCards},
    use_archive,
};

const MAX_IMAGE_RESULTS: usize = 2000;
const MAX_NODE_RESULTS: usize = 60;

#[component]
pub fn SearchView(query: String) -> Element {
    let archive = use_archive();
    let nodes = archive.search_nodes(&query, MAX_NODE_RESULTS);
    let results = archive.search(&query, MAX_IMAGE_RESULTS);
    let (images, albums): (Vec<_>, Vec<_>) =
        results.into_iter().map(|v| (v.image, v.album_key)).unzip();
    let capped = images.len() >= MAX_IMAGE_RESULTS;

    rsx! {
        section { class: "page",
            h1 { "Search: “{query}”" }
            if !nodes.is_empty() {
                h2 { "Folders & albums" }
                NodeCards { indices: nodes }
            }
            h2 {
                "Photos & videos ({images.len()}"
                if capped { "+" }
                ")"
            }
            if images.is_empty() {
                p { class: "empty", "No matching file names, titles, captions or keywords." }
            }
            ImageGrid { key: "{query}", images, albums }
        }
    }
}
