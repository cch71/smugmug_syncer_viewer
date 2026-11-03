//! Layout around every page: header with search, folder tree sidebar, and the routed page

use dioxus::prelude::*;

use super::{AppState, Route, use_archive};

#[component]
pub fn Shell() -> Element {
    let state = use_context::<AppState>();
    let archive = use_archive();
    let route = use_route::<Route>();
    let mut query = use_signal(|| match &route {
        Route::SearchView { query } => query.clone(),
        _ => String::new(),
    });
    let mut sidebar_open = use_signal(|| true);

    // Tree nodes to expand so the current page is visible in the sidebar
    let active = match &route {
        Route::FolderView { node_id } => archive.node(node_id).map(|(i, _)| i),
        Route::AlbumView { album_key } | Route::PhotoView { album_key, .. } => {
            archive.album_node(album_key).map(|(i, _)| i)
        }
        _ => None,
    };
    let active_path = active.map(|i| archive.ancestry(i)).unwrap_or_default();

    rsx! {
        div { class: if sidebar_open() { "app" } else { "app sidebar-hidden" },
            header { class: "topbar",
                button {
                    class: "icon-button",
                    title: "Toggle folders",
                    onclick: move |_| sidebar_open.toggle(),
                    "☰"
                }
                Link { class: "brand", to: Route::HomeView {}, "SmugMug Archive" }
                form {
                    class: "search",
                    onsubmit: move |e| {
                        e.prevent_default();
                        navigator().push(Route::SearchView { query: query() });
                    },
                    input {
                        r#type: "search",
                        placeholder: "Search file names, titles, captions, keywords…",
                        value: "{query}",
                        oninput: move |e| query.set(e.value()),
                    }
                }
                span { class: "location", title: "{archive.location}", "{archive.location}" }
                button {
                    class: "secondary",
                    title: "Open a different archive",
                    onclick: move |_| state.close(),
                    "Open…"
                }
            }
            nav { class: "sidebar",
                ul { class: "tree",
                    for child in archive.nodes[archive.root].children.iter().copied() {
                        TreeItem {
                            key: "{child}",
                            idx: child,
                            active: active,
                            active_path: active_path.clone(),
                        }
                    }
                }
            }
            main { class: "content", Outlet::<Route> {} }
        }
    }
}

#[component]
fn TreeItem(idx: usize, active: Option<usize>, active_path: Vec<usize>) -> Element {
    let archive = use_archive();
    let node = &archive.nodes[idx];
    // None until the user toggles it; until then follow the current page
    let mut toggled = use_signal(|| None::<bool>);
    let has_children = !node.children.is_empty();
    let expanded = toggled().unwrap_or(active_path.contains(&idx));

    let class = match (Some(idx) == active, node.album.is_some()) {
        (true, true) => "tree-row album active",
        (true, false) => "tree-row folder active",
        (false, true) => "tree-row album",
        (false, false) => "tree-row folder",
    };

    rsx! {
        li {
            div { class,
                if has_children {
                    button {
                        class: "twisty",
                        onclick: move |_| toggled.set(Some(!expanded)),
                        if expanded { "▾" } else { "▸" }
                    }
                } else {
                    span { class: "twisty" }
                }
                Link { to: Route::for_node(&archive, idx),
                    span { class: "tree-name", "{node.display_name()}" }
                    span { class: "count", "{node.total_images}" }
                }
            }
            if has_children && expanded {
                ul {
                    for child in node.children.iter().copied() {
                        TreeItem {
                            key: "{child}",
                            idx: child,
                            active,
                            active_path: active_path.clone(),
                        }
                    }
                }
            }
        }
    }
}
