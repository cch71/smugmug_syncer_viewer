# smugmug_syncer_viewer

A [Dioxus](https://dioxuslabs.com/) app for browsing an archive created by
[`smugmug_syncer`](../smugmug_syncer). It runs as a macOS desktop app today, and the same code
builds as a web app / PWA (see [docs/PWA.md](docs/PWA.md)).

The archive is only ever read. The viewer's own data (the last archive opened, and a thumbnail
cache) lives in the OS config and cache directories, never inside the archive.

## Features

- Folder and album tree in a sidebar, mirroring the account's SmugMug layout, with image
  counts.
- Folder pages with cover thumbnails, and album pages with a thumbnail grid.
- A photo and video viewer: arrow keys for previous/next, `Esc` to close, `i` to toggle the
  info panel.
- An info panel showing file name, dates, dimensions, size, camera, GPS (linked to
  OpenStreetMap), keywords, MD5, the albums the image is in, and on request every EXIF/IPTC
  field SmugMug kept.
- Search across file names, titles, captions, keywords and album names.
- Fast thumbnails on desktop: JPEGs are decoded at reduced scale, EXIF orientation is applied,
  and the results are cached on disk.

## Requirements

- Rust 1.88+ (edition 2024)
- The Dioxus CLI, matching the `dioxus` version in `Cargo.toml`:

  ```bash
  cargo install dioxus-cli@0.7.10 --locked
  ```

- Web builds also need the wasm target: `rustup target add wasm32-unknown-unknown`

## Running the desktop app

```bash
dx serve
```

Or, without the CLI:

```bash
cargo run --release -- ~/wip/SmugMugBackups/CaliHamilton
```

The viewer picks the archive to open from, in order:

1. the first command-line argument
2. the `SMUGMUG_ARCHIVE` environment variable
3. the archive opened last time

If none of these is set, it shows a screen where you can type a path or pick the folder.
**Open…** in the toolbar switches to another archive.

To build a macOS `.app` bundle:

```bash
dx bundle --release
```

## What it reads

The archive layout comes from the syncer's `PathFinder` (`smugmug_syncer/src/path_finder.rs`).
The viewer mirrors those paths in [src/archive/paths.rs](src/archive/paths.rs).

| File | Format | Used for |
| --- | --- | --- |
| `.smugmug_db/node_and_album_tree.db` | zstd JSON (`tree_ds` tree of SmugMug nodes and albums) | Folder/album tree |
| `.smugmug_db/album_image_map.db` | zstd JSON, album key → images | Album contents, search |
| `.smugmug_db/props.json` | JSON | Last sync times |
| `.smugmug_db/video_info/*.parquet` | parquet | Video file extension, size and duration |
| `.smugmug_db/image_metadata/*.parquet` | parquet | Date taken, camera, dimensions, GPS, full EXIF JSON |
| `artifacts/<ImageKey>` | image as uploaded (no extension) | Photos, and video poster frames |
| `artifacts/<ImageKey>.<ext>` | video | Videos |

Folders the syncer doesn't manage, such as `recovered/`, are ignored.

## Project layout

```
src/
  main.rs               entry point
  archive/              platform neutral archive model and index (also builds for wasm)
    model.rs            serde types for the syncer's files
    paths.rs            archive relative paths (mirror of the syncer's PathFinder)
    mod.rs              Archive: tree, album → images, search
  platform/             everything that differs between desktop and web
    desktop/            disk access, parquet reading, /media asset handler, thumbnails, settings
    web.rs              HTTP fetching for the web/PWA build
  ui/                   Dioxus components and router (platform neutral)
assets/                 CSS (bundled through asset!())
public/                 PWA manifest, service worker and the app icons (copied as-is into
                        web builds; also the desktop bundle icon)
docs/icon.svg           source the icon PNGs are rendered from
```

## Development

```bash
cargo clippy --release                                    # desktop
cargo clippy --no-default-features --features web --target wasm32-unknown-unknown
cargo test --release
SMUGMUG_ARCHIVE=~/wip/SmugMugBackups/CaliHamilton cargo test --release -- --nocapture
```

The last command also loads a real archive and builds thumbnails for a sample of its artifacts,
printing the timings.
