# smugmug_syncer_viewer

[![CI](https://github.com/cch71/smugmug_syncer_viewer/actions/workflows/ci.yml/badge.svg)](https://github.com/cch71/smugmug_syncer_viewer/actions/workflows/ci.yml)
[![CodeQL](https://github.com/cch71/smugmug_syncer_viewer/actions/workflows/codeql.yml/badge.svg)](https://github.com/cch71/smugmug_syncer_viewer/actions/workflows/codeql.yml)
[![Security](https://github.com/cch71/smugmug_syncer_viewer/actions/workflows/security.yml/badge.svg)](https://github.com/cch71/smugmug_syncer_viewer/actions/workflows/security.yml)

A [Dioxus](https://dioxuslabs.com/) app for browsing an archive created by
[`smugmug_syncer`](../smugmug_syncer). It runs as a macOS desktop app today, and the same code
builds as a web app / PWA (see [docs/PWA.md](docs/PWA.md)).

The archive is only ever read. The viewer's own data (the last archive opened, and a thumbnail
cache) lives in the OS config and cache directories, never inside the archive.

**To install and use the app, see the [user manual](docs/USER_MANUAL.md).** Installers for
Windows, macOS and Linux are on the
[Releases page](https://github.com/cch71/smugmug_syncer_viewer/releases). The rest of this
README is for building from source.

## Features

- Folder and album tree in a sidebar, mirroring the account's SmugMug layout, with image
  counts.
- Folder pages with cover thumbnails, and album pages with a thumbnail grid.
- A photo and video viewer: arrow keys for previous/next, `Esc` to close, `i` to toggle the
  info panel.
- An info panel showing file name, dates, dimensions, size, camera, lens, exposure, GPS
  (linked to OpenStreetMap), keywords, MD5 and the albums the image is in. For videos it also
  shows the video and audio codecs (for example `H.264 / AVC (High, level 4.0) · 1920 × 1080 ·
  30.00 fps` and `AAC-LC · stereo · 48.0 kHz`).
- Merged metadata. Each value comes from SmugMug's stored metadata (`image_metadata`) or from
  the EXIF embedded in the downloaded file, falling back to the file when SmugMug has nothing.
  **Show all metadata** lists every field from both, tagged **S** (SmugMug), **F** (file) or
  **S+F** (both agree), and shows both values when they disagree.
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
| `artifacts/<ImageKey>` | image as uploaded (no extension) | Photos, their embedded EXIF, and video poster frames |
| `artifacts/<ImageKey>.<ext>` | video | Videos, and their codecs (read from the MP4/MOV `moov` box) |

The codecs shown are those of the file in the archive: SmugMug's MP4 rendition, not the
originally uploaded `.wmv`, `.mpg` or `.3gp`.

Folders the syncer doesn't manage, such as `recovered/`, are ignored.

## Project layout

```
src/
  main.rs               entry point
  archive/              platform neutral archive model and index (also builds for wasm)
    model.rs            serde types for the syncer's files
    file_info.rs        EXIF/codec info from artifact files, merged with SmugMug's metadata
    mp4.rs              dependency-free MP4/MOV track and codec probe
    paths.rs            archive relative paths (mirror of the syncer's PathFinder)
    mod.rs              Archive: tree, album → images, search
  platform/             everything that differs between desktop and web
    desktop/            disk access, parquet and EXIF reading, /media asset handler,
                        thumbnails, settings
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
cargo deny check                                          # advisories, licenses, sources
SMUGMUG_ARCHIVE=~/wip/SmugMugBackups/CaliHamilton cargo test --release -- --nocapture
```

The last command also loads a real archive, builds thumbnails for a sample of its artifacts,
probes every video's codecs, and prints one photo's merged metadata.

## Releasing

Releases are made with [cargo-release](https://github.com/crate-ci/cargo-release) (configured
in [release.toml](release.toml)). Nothing is published to crates.io.

1. Add the changes to the `## [Unreleased]` section of [CHANGELOG.md](CHANGELOG.md) and
   commit them.
2. Do a dry run, then the release. Use `patch`, `minor` or `major` to choose the version, or
   leave it out to release the version already in `Cargo.toml`:

   ```bash
   cargo release patch
   cargo release patch --execute
   ```

   This bumps the version and dates the changelog. It then commits, creates the tag `vX.Y.Z`,
   and pushes both.
3. The pushed tag starts [the release workflow](.github/workflows/release.yml). It builds:

   | Platform | Packages |
   | --- | --- |
   | Windows x86_64 | `.msi`, `-setup.exe` |
   | macOS Apple silicon and Intel | `.dmg`, `.app.zip` |
   | Linux x86_64 | `.deb`, `.AppImage` (the AppImage is best effort) |

   It then creates the GitHub release with notes from the changelog, attaches the packages
   and `SHA256SUMS.txt`, and adds GitHub's generated notes.

A release can also be started by creating a `vX.Y.Z` tag from GitHub's **Draft a new
release** page, or by re-running the workflow manually with an existing tag. The tag has to
match the `Cargo.toml` version, or the build stops.

The packages aren't code-signed, so Windows and macOS warn on first launch. The user manual
explains how to get past the warning.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution you intentionally submit for
inclusion in this work, as defined in the Apache-2.0 license, shall be dual licensed as
above, without any additional terms or conditions.
