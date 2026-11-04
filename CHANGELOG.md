# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). `cargo release` moves the Unreleased section under
the new version number.

## [Unreleased]

### Added

- Desktop viewer for `smugmug_syncer` archives: folder/album tree, album thumbnail grids,
  photo and video viewer with keyboard navigation, and search.
- Info panel that merges SmugMug's stored metadata with the EXIF in each file, and shows
  video and audio codecs for videos.
- Thumbnail cache in the OS cache directory, so the archive is never written to.
- Web/PWA build (`dx build --platform web`) that reads an archive over HTTP.

[Unreleased]: https://github.com/cch71/smugmug_syncer_viewer/compare/v0.1.0...HEAD
