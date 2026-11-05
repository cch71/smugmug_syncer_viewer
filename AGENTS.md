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
- The syncer's `DB_VERSION` (`db_version` in `props.json`) is checked by
  `src/archive/db_version.rs`. When updating the viewer for a newer syncer layout, raise
  `SUPPORTED_DB_VERSION` to match; for a new major version, keep reading the old one or
  explain in the error why it can't.

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

### CI, releases and dependencies

- Releases go through `cargo release` (`release.toml`) and the tag-triggered
  `.github/workflows/release.yml`. Keep `publish = false`: nothing goes to crates.io.
- Record user-visible changes under `## [Unreleased]` in `CHANGELOG.md`. The release notes
  are taken from it.
- Pin third-party GitHub Actions to a full commit SHA with the version in a comment. Keep
  workflow `permissions` minimal (read by default, write only on the job that needs it), and
  check out with `persist-credentials: false`.
- When changing the `dioxus` version, change `DIOXUS_CLI_VERSION` in the workflows to match.
- New dependencies must pass `cargo deny check`. Add a license to `deny.toml` only if it is
  permissive or file-level copyleft.
- Put setup that several workflows need in `.github/actions/setup-build` rather than
  copying steps between them.

## Checks before finishing

- `cargo clippy --release` and
  `cargo clippy --no-default-features --features web --target wasm32-unknown-unknown` add no
  new warnings.
- `cargo test --release`. When an archive is available, also run it with `SMUGMUG_ARCHIVE` set.
- `dx build --platform web --release` still succeeds.
- `cargo deny check` passes.
- After editing workflows: `actionlint` and `zizmor --offline .github` report no new
  findings.
