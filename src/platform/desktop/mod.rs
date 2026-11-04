//! Desktop back end: reads the archive straight from disk and serves its media to the webview
//! through the `/media/...` asset handler.

mod exif;
mod media;
mod parquet_rows;
mod settings;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use dioxus::prelude::*;

use crate::archive::{
    Archive, DetailsMap,
    file_info::{FileInfo, media_fields},
    model::{ImageDetails, VideoInfo},
    mp4, paths,
};

pub use media::{image_url, thumb_url, use_media_handler, video_url};
pub use settings::remember_location;

pub fn launch(app: fn() -> Element) {
    use dioxus::desktop::{Config, LogicalSize, WindowBuilder};

    let window = WindowBuilder::new()
        .with_title("SmugMug Archive Viewer")
        .with_inner_size(LogicalSize::new(1400.0, 900.0))
        .with_min_inner_size(LogicalSize::new(640.0, 480.0));
    dioxus::LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(window))
        .launch(app);
}

/// Archive to open at start up: the first command line argument, `SMUGMUG_ARCHIVE`, or the
/// archive opened last time
pub fn initial_location() -> Option<String> {
    std::env::args()
        .nth(1)
        .or_else(|| std::env::var("SMUGMUG_ARCHIVE").ok())
        .or_else(settings::last_location)
}

/// Shows a folder picker
pub async fn pick_location() -> Option<String> {
    rfd::AsyncFileDialog::new()
        .set_title("Choose a smugmug_syncer archive folder")
        .pick_folder()
        .await
        .map(|v| v.path().to_string_lossy().to_string())
}

/// Reads the archive's tree, image map, props and video info
pub async fn load_archive(location: String) -> Result<Archive> {
    run_blocking(move || {
        let root = expand_home(&location);
        let tree_path = root.join(paths::NODE_AND_ALBUM_DATA_FILE);
        if !tree_path.exists() {
            bail!(
                "{} isn't a smugmug_syncer archive (no {})",
                root.display(),
                paths::NODE_AND_ALBUM_DATA_FILE
            );
        }
        let tree = std::fs::read(&tree_path).with_context(|| tree_path.display().to_string())?;
        let image_map = read_if_exists(&root.join(paths::ALBUM_IMAGE_MAP_FILE))?;
        let props = read_if_exists(&root.join(paths::PROPS_FILE))?;
        let videos = load_video_info(&root.join(paths::VIDEO_INFO_DIR))?;

        let archive = Archive::from_parts(
            root.to_string_lossy().to_string(),
            &tree,
            image_map.as_deref(),
            props.as_deref(),
            videos,
        )?;
        media::set_archive_root(&root);
        Ok(archive)
    })
    .await
}

/// Reads the summary columns of every `image_metadata` parquet file
pub async fn load_details(location: String) -> Result<DetailsMap> {
    run_blocking(move || {
        let dir = expand_home(&location).join(paths::IMAGE_METADATA_DIR);
        let mut details = HashMap::new();
        parquet_rows::for_each_row(
            &dir,
            &[
                "image_key",
                "date_time_original",
                "original_width",
                "original_height",
                "original_size",
                "make",
                "model",
                "latitude",
                "longitude",
            ],
            |file, row| {
                let Some(key) = row.str("image_key") else {
                    return;
                };
                details.insert(
                    key.to_string(),
                    ImageDetails {
                        date_time_original: row.string("date_time_original"),
                        original_width: row.u64("original_width"),
                        original_height: row.u64("original_height"),
                        original_size: row.u64("original_size"),
                        make: row.string("make"),
                        model: row.string("model"),
                        latitude: row.f64("latitude"),
                        longitude: row.f64("longitude"),
                        source_file: Some(file.to_string()),
                    },
                );
            },
        )?;
        Ok(details)
    })
    .await
}

/// Reads the full EXIF/IPTC JSON SmugMug holds for one image.  `source_file` (from
/// [`ImageDetails`]) limits the scan to the parquet file holding the image.
pub async fn load_metadata_json(
    location: String,
    image_key: String,
    source_file: Option<String>,
) -> Result<Option<String>> {
    run_blocking(move || {
        let dir = expand_home(&location).join(paths::IMAGE_METADATA_DIR);
        let mut found = None;
        let filter = |name: &str| source_file.as_deref().is_none_or(|v| v == name);
        parquet_rows::for_each_row_in(&dir, filter, &["image_key", "metadata_json"], |_, row| {
            if found.is_none() && row.str("image_key") == Some(image_key.as_str()) {
                found = row.string("metadata_json");
            }
        })?;
        Ok(found)
    })
    .await
}

/// Reads what the artifact file itself records: EXIF for images, or for videos (when
/// `video_ext` is given) the container's tracks and codecs
pub async fn load_file_info(
    location: String,
    image_key: String,
    video_ext: Option<String>,
) -> Result<FileInfo> {
    run_blocking(move || {
        let root = expand_home(&location);
        let Some(ext) = video_ext else {
            return exif::read_file_info(&root.join(paths::image_artifact(&image_key)));
        };
        let path = root.join(paths::video_artifact(&image_key, &ext));
        let file =
            std::fs::File::open(&path).with_context(|| format!("{} not found", path.display()))?;
        let media = mp4::probe(&mut std::io::BufReader::new(file))
            .with_context(|| format!("reading {}", path.display()))?;
        Ok(FileInfo {
            fields: media_fields(&media),
            media: Some(media),
            ..Default::default()
        })
    })
    .await
}

fn load_video_info(dir: &Path) -> Result<HashMap<String, VideoInfo>> {
    let mut videos = HashMap::new();
    parquet_rows::for_each_row(
        dir,
        &["image_key", "ext", "width", "height", "duration"],
        |_, row| {
            if let (Some(key), Some(ext)) = (row.str("image_key"), row.str("ext")) {
                videos.insert(
                    key.to_string(),
                    VideoInfo {
                        ext: ext.to_string(),
                        width: row.u64("width"),
                        height: row.u64("height"),
                        duration: row.string("duration"),
                    },
                );
            }
        },
    )?;
    Ok(videos)
}

fn read_if_exists(path: &Path) -> Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(data) => Ok(Some(data)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(anyhow::Error::from(err).context(path.display().to_string())),
    }
}

/// Expands a leading `~/` so paths can be typed the way they're written in a shell
fn expand_home(location: &str) -> PathBuf {
    match (location.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(location),
    }
}

async fn run_blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(f).await?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Loads a real archive when `SMUGMUG_ARCHIVE` points at one, e.g.
    /// `SMUGMUG_ARCHIVE=~/wip/SmugMugBackups/CaliHamilton cargo test -- --nocapture`
    #[tokio::test(flavor = "multi_thread")]
    async fn loads_archive_from_env() {
        let Ok(location) = std::env::var("SMUGMUG_ARCHIVE") else {
            eprintln!("SMUGMUG_ARCHIVE not set; skipping");
            return;
        };
        let start = std::time::Instant::now();
        let archive = load_archive(location.clone()).await.unwrap();
        eprintln!(
            "loaded {} nodes, {} albums, {} images, {} videos ({} video infos) in {:?}",
            archive.nodes.len(),
            archive.album_count(),
            archive.image_count(),
            archive.video_count(),
            archive.videos.len(),
            start.elapsed()
        );
        assert!(archive.album_count() > 0);

        let start = std::time::Instant::now();
        let details = load_details(location.clone()).await.unwrap();
        eprintln!(
            "loaded {} image details in {:?}",
            details.len(),
            start.elapsed()
        );

        if let Some((key, d)) = details.iter().find(|(_, d)| d.make.is_some()) {
            let start = std::time::Instant::now();
            let json = load_metadata_json(location, key.clone(), d.source_file.clone())
                .await
                .unwrap();
            eprintln!("{key}: {d:?}\nmetadata json in {:?}:", start.elapsed());
            eprintln!("{}", json.unwrap_or_default());
        }
    }
}

#[cfg(test)]
mod file_info_tests {
    use super::*;
    use crate::archive::file_info::merge_fields;

    /// Probes every video in the archive and merges one image's EXIF with SmugMug's, when
    /// `SMUGMUG_ARCHIVE` is set
    #[tokio::test(flavor = "multi_thread")]
    async fn file_info_from_env_archive() {
        let Ok(location) = std::env::var("SMUGMUG_ARCHIVE") else {
            eprintln!("SMUGMUG_ARCHIVE not set; skipping");
            return;
        };
        let archive = load_archive(location.clone()).await.unwrap();

        let start = std::time::Instant::now();
        let mut codecs = std::collections::BTreeMap::<String, usize>::new();
        let mut failures = Vec::new();
        for (key, info) in &archive.videos {
            match load_file_info(location.clone(), key.clone(), Some(info.ext.clone())).await {
                Ok(file) => {
                    let media = file.media.unwrap();
                    let names: Vec<String> = media.tracks.iter().map(|t| t.summary()).collect();
                    let video = media
                        .tracks
                        .iter()
                        .find(|t| t.kind == crate::archive::file_info::TrackKind::Video);
                    let audio = media
                        .tracks
                        .iter()
                        .find(|t| t.kind == crate::archive::file_info::TrackKind::Audio);
                    *codecs
                        .entry(format!(
                            "{} + {}",
                            video.map_or("no video", |t| t.codec.as_str()),
                            audio.map_or("no audio", |t| t.codec.as_str())
                        ))
                        .or_default() += 1;
                    if key == "Vq8j9FG" || key == "npvpSC4" {
                        eprintln!("{key}: {:?} {names:?}", media.duration_secs);
                    }
                }
                Err(err) => failures.push(format!("{key}: {err:#}")),
            }
        }
        eprintln!(
            "probed {} videos in {:?}",
            archive.videos.len(),
            start.elapsed()
        );
        for (combo, count) in &codecs {
            eprintln!("  {count:4}  {combo}");
        }
        for failure in failures.iter().take(10) {
            eprintln!("  failed {failure}");
        }
        assert!(
            failures.len() * 20 < archive.videos.len().max(1),
            "too many failures"
        );

        let details = load_details(location.clone()).await.unwrap();
        let (key, d) = details.iter().find(|(_, d)| d.lens_hint()).unwrap();
        let file = load_file_info(location.clone(), key.clone(), None)
            .await
            .unwrap();
        let json = load_metadata_json(location, key.clone(), d.source_file.clone())
            .await
            .unwrap()
            .unwrap();
        let smugmug: Vec<(String, String)> = match serde_json::from_str(&json).unwrap() {
            serde_json::Value::Object(map) => map
                .into_iter()
                .map(|(k, v)| (k, v.as_str().map_or(v.to_string(), str::to_string)))
                .filter(|(_, v)| !v.is_empty())
                .collect(),
            _ => vec![],
        };
        eprintln!(
            "\n{key}: summary {:?}",
            crate::archive::file_info::summarize(Some(d), Some(&file))
        );
        for field in merge_fields(&smugmug, &file.fields) {
            eprintln!(
                "  {:?} {} = {}{}",
                field.source,
                field.name,
                field.value,
                field
                    .file_value
                    .map(|v| format!("   (file: {v})"))
                    .unwrap_or_default()
            );
        }
    }

    trait LensHint {
        fn lens_hint(&self) -> bool;
    }
    impl LensHint for ImageDetails {
        fn lens_hint(&self) -> bool {
            self.make.as_deref() == Some("Apple") && self.latitude.is_some()
        }
    }
}
