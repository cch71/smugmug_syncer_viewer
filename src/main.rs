//! Viewer for archives created by `smugmug_syncer`.
//!
//! Runs as a desktop app today (`dx serve` / `cargo run`) and is structured to also build as a
//! web app / PWA (`dx serve --platform web`): see `docs/PWA.md`.

mod archive;
mod platform;
mod ui;

fn main() {
    platform::launch(ui::App);
}
