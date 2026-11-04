//! Serves the archive's media to the webview at `/media/<generation>/...`:
//!
//! - `/media/<gen>/file/<archive relative path>`: the file as stored, with HTTP range support
//!   (WebKit needs ranges to play video)
//! - `/media/<gen>/thumb/<image key>`: a JPEG thumbnail, cached in the OS cache directory
//!
//! `<gen>` changes every time an archive is opened so the webview never shows media cached from
//! a different archive.  Requests are handled on a small worker pool so the UI thread never
//! blocks on disk or image decoding.

use std::{
    borrow::Cow,
    fs::File,
    io::{Cursor, Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, RwLock,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::SystemTime,
};

use anyhow::{Context, Result, anyhow};
use dioxus::desktop::{
    AssetRequest, RequestAsyncResponder, use_asset_handler,
    wry::http::{Response, StatusCode, header},
};
use image::{DynamicImage, GrayImage, RgbImage, codecs::jpeg::JpegEncoder};

use crate::archive::paths;

const HANDLER_NAME: &str = "media";
/// Longest edge of generated thumbnails, sized for HiDPI grid cells
const THUMB_SIZE: u32 = 480;
/// Largest chunk returned for an open ended range request (`bytes=N-`)
const MAX_RANGE_CHUNK: u64 = 8 * 1024 * 1024;

static GENERATION: AtomicU64 = AtomicU64::new(0);
static ARCHIVE: RwLock<Option<Arc<ArchiveRoot>>> = RwLock::new(None);

struct ArchiveRoot {
    generation: u64,
    root: PathBuf,
    thumb_dir: Option<PathBuf>,
}

/// Points the media handler at a newly opened archive
pub fn set_archive_root(root: &Path) {
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let thumb_dir = super::settings::project_dirs().map(|dirs| {
        dirs.cache_dir()
            .join("thumbs")
            .join(format!("{:016x}", fnv1a(root.to_string_lossy().as_bytes())))
    });
    *ARCHIVE.write().unwrap() = Some(Arc::new(ArchiveRoot {
        generation,
        root: root.to_path_buf(),
        thumb_dir,
    }));
}

fn current_generation() -> u64 {
    GENERATION.load(Ordering::SeqCst)
}

pub fn thumb_url(image_key: &str) -> String {
    format!("/{HANDLER_NAME}/{}/thumb/{image_key}", current_generation())
}

pub fn image_url(image_key: &str) -> String {
    file_url(&paths::image_artifact(image_key))
}

pub fn video_url(image_key: &str, ext: &str) -> String {
    file_url(&paths::video_artifact(image_key, ext))
}

fn file_url(relative: &str) -> String {
    format!("/{HANDLER_NAME}/{}/file/{relative}", current_generation())
}

/// Registers the `/media` asset handler.  Call once from the root component.
pub fn use_media_handler() {
    use_asset_handler(HANDLER_NAME, |request, responder| {
        submit(move || respond(&request, responder));
    });
}

fn respond(request: &AssetRequest, responder: RequestAsyncResponder) {
    let response = handle(request).unwrap_or_else(|err| {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Cow::Owned(err.to_string().into_bytes()))
            .unwrap()
    });
    responder.respond(response);
}

fn handle(request: &AssetRequest) -> Result<Response<Cow<'static, [u8]>>> {
    let archive = ARCHIVE.read().unwrap().clone().context("no archive open")?;

    // /media/<gen>/<kind>/<rest>
    let path = request.uri().path().trim_start_matches('/');
    let mut parts = path.splitn(4, '/');
    let (_, generation, kind, rest) = (parts.next(), parts.next(), parts.next(), parts.next());
    let rest = rest.context("bad media path")?;
    if generation.and_then(|v| v.parse::<u64>().ok()) != Some(archive.generation) {
        return Err(anyhow!("stale archive"));
    }

    match kind {
        Some("file") => serve_file(&archive.root.join(safe_relative(rest)?), request),
        Some("thumb") => serve_thumbnail(&archive, rest),
        _ => Err(anyhow!("unknown media kind")),
    }
}

/// Rejects paths that would escape the archive
fn safe_relative(path: &str) -> Result<&Path> {
    let path = Path::new(path);
    if path.components().all(|c| matches!(c, Component::Normal(_))) {
        Ok(path)
    } else {
        Err(anyhow!("invalid path"))
    }
}

fn serve_file(path: &Path, request: &AssetRequest) -> Result<Response<Cow<'static, [u8]>>> {
    let mut file = File::open(path).with_context(|| format!("{} not found", path.display()))?;
    let len = file.metadata()?.len();
    let mut head = [0u8; 16];
    let head_len = file.read(&mut head)?;
    let content_type = sniff_content_type(&head[..head_len], path);

    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| parse_range(v, len));

    let builder = Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "max-age=3600");

    match range {
        Some((start, end)) => {
            let mut data = vec![0u8; (end - start + 1) as usize];
            file.seek(SeekFrom::Start(start))?;
            file.read_exact(&mut data)?;
            Ok(builder
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"))
                .header(header::CONTENT_LENGTH, data.len())
                .body(Cow::Owned(data))?)
        }
        None => {
            let mut data = Vec::with_capacity(len as usize);
            file.seek(SeekFrom::Start(0))?;
            file.read_to_end(&mut data)?;
            Ok(builder
                .status(StatusCode::OK)
                .header(header::CONTENT_LENGTH, data.len())
                .body(Cow::Owned(data))?)
        }
    }
}

/// Parses a single `bytes=start-end` range, clamped to the file
fn parse_range(value: &str, len: u64) -> Option<(u64, u64)> {
    if len == 0 {
        return None;
    }
    let (start, end) = value
        .strip_prefix("bytes=")?
        .split(',')
        .next()?
        .split_once('-')?;
    let (start, end) = match (start.trim(), end.trim()) {
        ("", suffix) => (len.saturating_sub(suffix.parse().ok()?), len - 1),
        (start, "") => {
            let start: u64 = start.parse().ok()?;
            (start, (start + MAX_RANGE_CHUNK - 1).min(len - 1))
        }
        (start, end) => (start.parse().ok()?, end.parse::<u64>().ok()?.min(len - 1)),
    };
    (start <= end).then_some((start, end))
}

fn sniff_content_type(head: &[u8], path: &Path) -> &'static str {
    match head {
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [b'G', b'I', b'F', b'8', ..] => "image/gif",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "image/webp",
        [_, _, _, _, b'f', b't', b'y', b'p', b'q', b't', ..] => "video/quicktime",
        [_, _, _, _, b'f', b't', b'y', b'p', ..] => "video/mp4",
        _ => match path.extension().and_then(|v| v.to_str()) {
            Some("mov") => "video/quicktime",
            Some("mp4" | "m4v") => "video/mp4",
            _ => "application/octet-stream",
        },
    }
}

fn serve_thumbnail(archive: &ArchiveRoot, image_key: &str) -> Result<Response<Cow<'static, [u8]>>> {
    let source = archive
        .root
        .join(safe_relative(&paths::image_artifact(image_key))?);
    let data = cached_thumbnail(archive, image_key, &source)?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(header::CACHE_CONTROL, "max-age=86400")
        .header(header::CONTENT_LENGTH, data.len())
        .body(Cow::Owned(data))?)
}

/// Returns the cached thumbnail, (re)building it when it's missing or older than the source
fn cached_thumbnail(archive: &ArchiveRoot, image_key: &str, source: &Path) -> Result<Vec<u8>> {
    let source_modified = std::fs::metadata(source)
        .with_context(|| format!("{} not found", source.display()))?
        .modified()
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let cache_file = archive
        .thumb_dir
        .as_ref()
        .map(|dir| dir.join(format!("{image_key}_{THUMB_SIZE}.jpg")));
    if let Some(cache_file) = &cache_file
        && let Ok(meta) = std::fs::metadata(cache_file)
        && meta.modified().is_ok_and(|v| v >= source_modified)
        && let Ok(data) = std::fs::read(cache_file)
    {
        return Ok(data);
    }

    let data = make_thumbnail(&std::fs::read(source)?, THUMB_SIZE)?;
    if let Some(cache_file) = &cache_file {
        // Best effort: a failed cache write only costs a rebuild next time
        let tmp = cache_file.with_extension("tmp");
        let _ = std::fs::create_dir_all(cache_file.parent().unwrap())
            .and_then(|_| std::fs::write(&tmp, &data))
            .and_then(|_| std::fs::rename(&tmp, cache_file));
    }
    Ok(data)
}

/// Decodes, shrinks, orients and re-encodes an image as a JPEG thumbnail
fn make_thumbnail(data: &[u8], size: u32) -> Result<Vec<u8>> {
    let image = match data {
        [0xFF, 0xD8, ..] => {
            decode_jpeg_scaled(data, size).or_else(|_| image::load_from_memory(data))
        }
        _ => image::load_from_memory(data),
    }?;
    let image = apply_orientation(image.thumbnail(size, size), super::exif::orientation(data));

    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 80).encode_image(&image.to_rgb8())?;
    Ok(out)
}

/// Decodes a JPEG at a reduced scale (1/2 to 1/8), much faster than a full decode and resize
fn decode_jpeg_scaled(data: &[u8], size: u32) -> Result<DynamicImage> {
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(data));
    let target = size.min(u16::MAX as u32) as u16;
    let (width, height) = decoder.scale(target, target)?;
    let pixels = decoder.decode()?;
    let info = decoder.info().context("missing jpeg info")?;
    let (width, height) = (u32::from(width), u32::from(height));

    let image = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            RgbImage::from_raw(width, height, pixels).map(DynamicImage::ImageRgb8)
        }
        jpeg_decoder::PixelFormat::L8 => {
            GrayImage::from_raw(width, height, pixels).map(DynamicImage::ImageLuma8)
        }
        jpeg_decoder::PixelFormat::CMYK32 => {
            let rgb = pixels
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| {
                    let k = 255 - u32::from(p[3]);
                    [0, 1, 2].map(|i| ((255 - u32::from(p[i])) * k / 255) as u8)
                })
                .collect();
            RgbImage::from_raw(width, height, rgb).map(DynamicImage::ImageRgb8)
        }
        jpeg_decoder::PixelFormat::L16 => None,
    };
    image.context("unsupported jpeg pixel format")
}

fn apply_orientation(image: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        2 => image.fliph(),
        3 => image.rotate180(),
        4 => image.flipv(),
        5 => image.rotate90().fliph(),
        6 => image.rotate90(),
        7 => image.rotate270().fliph(),
        8 => image.rotate270(),
        _ => image,
    }
}

fn fnv1a(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf29ce484222325, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}

type Job = Box<dyn FnOnce() + Send>;

/// Runs a job on the media worker pool
fn submit(job: impl FnOnce() + Send + 'static) {
    static POOL: OnceLock<mpsc::Sender<Job>> = OnceLock::new();
    let sender = POOL.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        let workers = std::thread::available_parallelism().map_or(4, |v| v.get().clamp(2, 8));
        for i in 0..workers {
            let rx = rx.clone();
            std::thread::Builder::new()
                .name(format!("media-{i}"))
                .spawn(move || {
                    loop {
                        let job = rx.lock().unwrap().recv();
                        match job {
                            Ok(job) => job(),
                            Err(_) => break,
                        }
                    }
                })
                .expect("spawning media worker");
        }
        tx
    });
    let _ = sender.send(Box::new(job));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-1", 10), Some((0, 1)));
        assert_eq!(parse_range("bytes=5-", 10), Some((5, 9)));
        assert_eq!(parse_range("bytes=-3", 10), Some((7, 9)));
        assert_eq!(parse_range("bytes=4-100", 10), Some((4, 9)));
        assert_eq!(parse_range("bytes=9-2", 10), None);
        assert_eq!(parse_range("bytes=0-", 0), None);
        assert_eq!(
            parse_range("bytes=0-", 100 * 1024 * 1024),
            Some((0, MAX_RANGE_CHUNK - 1))
        );
    }

    /// Builds thumbnails for a sample of real artifacts when `SMUGMUG_ARCHIVE` is set
    #[test]
    fn thumbnails_from_env_archive() {
        let Ok(root) = std::env::var("SMUGMUG_ARCHIVE") else {
            eprintln!("SMUGMUG_ARCHIVE not set; skipping");
            return;
        };
        let dir = Path::new(&root).join(paths::ARTIFACTS_DIR);
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|v| v.ok().map(|v| v.path()))
            .take(40)
            .collect();
        files.sort();
        for path in files {
            let mut head = [0u8; 16];
            let n = File::open(&path).unwrap().read(&mut head).unwrap();
            let kind = sniff_content_type(&head[..n], &path);
            if !kind.starts_with("image/") {
                eprintln!("{}: {kind}", path.display());
                continue;
            }
            let start = std::time::Instant::now();
            let thumb = make_thumbnail(&std::fs::read(&path).unwrap(), THUMB_SIZE).unwrap();
            let decoded = image::load_from_memory(&thumb).unwrap();
            assert!(decoded.width().max(decoded.height()) <= THUMB_SIZE);
            eprintln!(
                "{}: {kind} -> {}x{} {} bytes in {:?}",
                path.file_name().unwrap().to_string_lossy(),
                decoded.width(),
                decoded.height(),
                thumb.len(),
                start.elapsed()
            );
        }
    }

    #[test]
    fn rejects_escaping_paths() {
        assert!(safe_relative("artifacts/abc").is_ok());
        assert!(safe_relative("../etc/passwd").is_err());
        assert!(safe_relative("/etc/passwd").is_err());
    }
}
