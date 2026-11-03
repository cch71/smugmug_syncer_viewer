//! Dioxus UI.  Platform neutral: anything platform specific goes through [`crate::platform`].

mod browse;
mod photo;
mod search;
mod shell;
mod welcome;

use std::sync::Arc;

use dioxus::prelude::*;

use crate::{
    archive::{Archive, DetailsMap},
    platform,
};
use browse::{AlbumView, FolderView, HomeView};
use photo::PhotoView;
use search::SearchView;
use shell::Shell;
use welcome::Welcome;

const MAIN_CSS: Asset = asset!("/assets/main.css");

#[derive(Routable, Clone, PartialEq, Debug)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Shell)]
        #[route("/")]
        HomeView {},
        #[route("/folder/:node_id")]
        FolderView { node_id: String },
        #[route("/album/:album_key")]
        AlbumView { album_key: String },
        #[route("/album/:album_key/photo/:image_key")]
        PhotoView { album_key: String, image_key: String },
        #[route("/search?:query")]
        SearchView { query: String },
    #[end_layout]
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

impl Route {
    /// Route showing the given tree node
    pub fn for_node(archive: &Archive, idx: usize) -> Route {
        let node = &archive.nodes[idx];
        match &node.album {
            _ if idx == archive.root => Route::HomeView {},
            Some(album) => Route::AlbumView {
                album_key: album.album_key.clone(),
            },
            None => Route::FolderView {
                node_id: node.node.node_id.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadState {
    Idle,
    Loading(String),
    Ready(Arc<Archive>),
    Failed(String),
}

/// Shared application state, provided as context by [`App`]
#[derive(Clone, Copy)]
pub struct AppState {
    pub archive: Signal<LoadState>,
    /// EXIF summary per image key, loaded in the background after the archive opens
    pub details: Signal<Option<Arc<DetailsMap>>>,
}

impl AppState {
    /// Opens the archive at `location` (folder path on desktop, base URL on the web)
    pub fn open(mut self, location: String) {
        let location = location.trim().to_string();
        if location.is_empty() {
            return;
        }
        self.archive.set(LoadState::Loading(location.clone()));
        self.details.set(None);
        spawn(async move {
            match platform::load_archive(location.clone()).await {
                Ok(archive) => {
                    platform::remember_location(&location);
                    let archive_location = archive.location.clone();
                    self.archive.set(LoadState::Ready(Arc::new(archive)));
                    match platform::load_details(archive_location).await {
                        Ok(details) => self.details.set(Some(Arc::new(details))),
                        Err(err) => eprintln!("couldn't load image metadata: {err:#}"),
                    }
                }
                Err(err) => self.archive.set(LoadState::Failed(format!("{err:#}"))),
            }
        });
    }

    pub fn close(mut self) {
        self.archive.set(LoadState::Idle);
        self.details.set(None);
    }
}

/// The open archive.  Only call from components rendered while an archive is loaded.
pub fn use_archive() -> Arc<Archive> {
    match &*use_context::<AppState>().archive.read() {
        LoadState::Ready(archive) => archive.clone(),
        _ => panic!("use_archive called without an open archive"),
    }
}

#[component]
pub fn App() -> Element {
    platform::use_media_handler();
    let state = use_context_provider(|| AppState {
        archive: Signal::new(LoadState::Idle),
        details: Signal::new(None),
    });
    use_hook(|| {
        if let Some(location) = platform::initial_location() {
            state.open(location);
        }
    });

    let ready = matches!(*state.archive.read(), LoadState::Ready(_));
    rsx! {
        document::Title { "SmugMug Archive Viewer" }
        document::Stylesheet { href: MAIN_CSS }
        PwaHead {}
        if ready {
            Router::<Route> {}
        } else {
            Welcome {}
        }
    }
}

/// Favicon, web app manifest and service worker registration.  Web only: the files come from
/// `public/`, which the desktop webview doesn't serve, and desktop shows the bundle icon instead.
#[component]
fn PwaHead() -> Element {
    if cfg!(feature = "desktop") {
        return rsx! {};
    }
    rsx! {
        document::Link { rel: "icon", href: "icons/icon-192.png" }
        document::Link { rel: "manifest", href: "manifest.webmanifest" }
        document::Meta { name: "theme-color", content: "#1f2430" }
        document::Script {
            "if ('serviceWorker' in navigator) {{ navigator.serviceWorker.register('sw.js'); }}"
        }
    }
}

#[component]
fn NotFound(segments: Vec<String>) -> Element {
    rsx! {
        div { class: "empty",
            h2 { "Nothing here" }
            p { "/{segments.join(\"/\")}" }
            Link { to: Route::HomeView {}, "Back to the archive" }
        }
    }
}
