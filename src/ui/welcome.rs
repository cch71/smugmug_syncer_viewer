//! Shown until an archive is open: choose an archive, and see loading progress or errors

use dioxus::prelude::*;

use super::{AppState, LoadState};
use crate::platform;

#[component]
pub fn Welcome() -> Element {
    let state = use_context::<AppState>();
    let mut location = use_signal(|| match &*state.archive.peek() {
        LoadState::Loading(v) => v.clone(),
        _ => platform::initial_location().unwrap_or_default(),
    });

    let status = state.archive.read().clone();
    let loading = matches!(status, LoadState::Loading(_));
    let placeholder = if platform::HAS_FOLDER_PICKER {
        "/path/to/archive (the folder holding .smugmug_db)"
    } else {
        "URL the archive is served from, e.g. https://example.com/archive"
    };

    rsx! {
        div { class: "welcome",
            div { class: "welcome-card",
                h1 { "SmugMug Archive Viewer" }
                p { class: "muted",
                    "Browse the folders, albums, photos and videos backed up by "
                    code { "smugmug_syncer" }
                    ". The archive is only read, never changed."
                }
                form {
                    class: "open-form",
                    onsubmit: move |e| {
                        e.prevent_default();
                        state.open(location());
                    },
                    input {
                        r#type: "text",
                        value: "{location}",
                        placeholder,
                        disabled: loading,
                        oninput: move |e| location.set(e.value()),
                    }
                    button { r#type: "submit", disabled: loading, "Open" }
                    if platform::HAS_FOLDER_PICKER {
                        button {
                            r#type: "button",
                            class: "secondary",
                            disabled: loading,
                            onclick: move |_| async move {
                                if let Some(picked) = platform::pick_location().await {
                                    location.set(picked.clone());
                                    state.open(picked);
                                }
                            },
                            "Browse…"
                        }
                    }
                }
                match status {
                    LoadState::Loading(path) => rsx! {
                        p { class: "status", span { class: "spinner" } "Loading {path}…" }
                    },
                    LoadState::Failed(err) => rsx! {
                        p { class: "status error", "Couldn't open the archive: {err}" }
                    },
                    _ => rsx! {},
                }
            }
        }
    }
}
