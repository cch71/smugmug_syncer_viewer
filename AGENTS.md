# AGENTS.md

Guidance for coding agents working on `smugmug_syncer_viewer`, a Dioxus app that browses
archives created by `smugmug_syncer`. See [README.md](README.md) for usage and
[docs/PWA.md](docs/PWA.md) for the web build.

## Rules

### The archive and the syncer are read only

- Never write into an archive directory, including caches, indexes and sidecar files. Viewer
  state goes in the OS config/cache directories (`platform/desktop/settings.rs`).
- Treat `../smugmug_syncer` as a reference. Don't modify it unless asked to. If the syncer's
  on-disk format changes, update `src/archive/paths.rs` and `src/archive/model.rs` to match its
  `PathFinder` and types.
- Archive files may come from older or newer syncer versions, so give every serde field a
  default and ignore unknown keys.

### Keep the UI and the archive index platform neutral

- `src/archive/` and `src/ui/` must build for both `desktop` and `web`
  (`wasm32-unknown-unknown`). Put file system, threads, native dialogs and anything else
  platform specific in `src/platform/`, behind the same function names in `desktop/` and
  `web.rs`.
- Build media URLs with `platform::thumb_url`, `image_url` and `video_url`. Never write
  `/media/...` or archive paths in UI code.
- Desktop-only crates (tokio, parquet, image, rfd, directories) must stay optional, enabled
  by the `desktop` feature.

### Reuse shared code

- Archive-relative paths come only from `src/archive/paths.rs`.
- Read parquet only through `platform/desktop/parquet_rows.rs`.
- Read EXIF from files only through `platform/desktop/exif.rs`, and video containers only
  through `src/archive/mp4.rs`.
- Merge SmugMug's metadata with a file's through `src/archive/file_info.rs`. When a newly read
  EXIF tag means the same as a SmugMug metadata key, add it to `smugmug_name` so the two show
  as one row.

### Keep one copy of each image file

Don't keep the same image (icons, logos, placeholders) in more than one place, such as in both
`assets/` and `public/`. Put it in one place and point every user at it.

- App icons live only in `public/icons/`. The web favicon, `manifest.webmanifest` and
  `Dioxus.toml`'s `[bundle] icon` all reference them there.
- `public/` is for files that need fixed URLs (manifest, service worker, icons).
  `assets/` is for files loaded through `asset!()`, which get hashed names.
- Render generated sizes from one source (`docs/icon.svg`) and replace the old files; don't add
  variants alongside them.

## Checks before finishing

- `cargo clippy --release` and
  `cargo clippy --no-default-features --features web --target wasm32-unknown-unknown` add no
  new warnings.
- `cargo test --release`. When an archive is available, also run it with `SMUGMUG_ARCHIVE` set.
- `dx build --platform web --release` still succeeds.
