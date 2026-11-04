# Building and serving the web app / PWA

The UI and the archive index are platform neutral. Only `src/platform/` differs:

| | Desktop (`--features desktop`, default) | Web (`--features web`) |
| --- | --- | --- |
| Archive location | Folder path | Base URL (`?archive=` query parameter, default `archive`, relative to the app) |
| Reading `.db` / `props.json` | `std::fs` | `fetch` |
| Media URLs | `/media/<gen>/...`, served by an asset handler | `<base>/artifacts/<key>`, served by the web server |
| Thumbnails | Generated and cached on disk | Full image, scaled by CSS |
| Parquet (video info, EXIF) | Read with the `parquet` crate | Not read yet (see below) |
| File's own EXIF, video codecs | Read from the artifact | Not read yet (see below) |

## Build

```bash
dx build --platform web --release
```

The output is written to `target/dx/smugmug_syncer_viewer/release/web/public/`. It contains
`index.html`, the wasm bundle, `manifest.webmanifest`, `sw.js` and the icons.

## Serve with an archive

Put the archive next to the app (a symlink works), or anywhere else and pass its URL:

```
webroot/
  index.html, assets/, sw.js, manifest.webmanifest, icons/   ← build output
  archive -> ~/wip/SmugMugBackups/CaliHamilton               ← the archive, read only
```

Notes for the web server:

- **Dotfiles:** `.smugmug_db/` must be served. Many servers hide dot-directories by default
  (for example nginx `location ~ /\.` rules, or Netlify).
- **SPA fallback:** routes such as `/album/<key>` are client-side, so unknown paths should
  return `index.html`. Without this, reloading on a deep link gives a 404.
- **Content types:** image artifacts have no extension. Browsers sniff images, but sending
  `Content-Type` based on content helps. Videos need byte-range support.
- **CORS:** if the archive is on another origin, it must send `Access-Control-Allow-Origin`.
- **Privacy:** anyone who can reach the URL can see the whole archive, private albums
  included. Put it behind authentication if it isn't on a trusted network.

For a quick local test (it has no SPA fallback, so reload from `/`):

```bash
cd webroot && python3 -m http.server 8765 --bind 127.0.0.1
```

## PWA behaviour

- `public/manifest.webmanifest` makes the app installable ("Add to Home Screen" or Install).
- `public/sw.js` caches the app shell network-first, and caches media under `/artifacts/`
  cache-first, so photos you've already viewed work offline. `.smugmug_db` is always fetched
  fresh. Bump `SHELL_CACHE` / `MEDIA_CACHE` to invalidate.
- Service workers need HTTPS, or `localhost` / `127.0.0.1`.

## Remaining gaps compared with desktop

1. **Parquet metadata.** `video_info` and `image_metadata` are zstd-compressed parquet. The
   `parquet` crate's zstd codec uses the C `zstd` library, which doesn't build for
   `wasm32-unknown-unknown` without a wasm-capable clang. Options:
   - build `zstd-sys` with a wasm-capable clang (`brew install llvm`, then
     `CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang`) and enable the desktop
     loader code for wasm
   - have the server pre-export the summary columns as JSON
   - use `parquet-wasm` / `hyparquet` from JS

   Until one of these is done, videos load from `<key>.mp4` (the extension the syncer stores),
   and the info panel omits the camera, date taken and GPS.
2. **Thumbnails.** The grid loads full-size images. For large albums, pre-generate thumbnails
   on the server (for example with the desktop thumbnail code as a CLI) and point `thumb_url`
   in `src/platform/web.rs` at them.
3. **The artifact's own EXIF and video codecs.** `load_file_info` returns nothing on the web.
   `src/archive/mp4.rs` has no dependencies and builds for wasm. It needs `Read + Seek`,
   which a reader backed by HTTP range requests can provide. `kamadak-exif` is pure Rust too,
   and only needs the first 64 KB or so of a JPEG.
4. **Local folders without a server.** Chromium's File System Access API
   (`showDirectoryPicker`) could read an archive folder directly. That would be a third
   implementation of the `platform` functions.
