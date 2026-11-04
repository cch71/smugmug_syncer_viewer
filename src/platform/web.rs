//! Web / PWA back end: reads the archive over HTTP from wherever it is served.
//!
//! The archive location is a base URL, taken from the `?archive=` query parameter or defaulting
//! to `archive` (relative to the app).  See `docs/PWA.md` for how to serve an archive.

use std::{collections::HashMap, sync::Mutex};

use anyhow::{Result, anyhow, bail};
use dioxus::prelude::*;

use crate::archive::{Archive, DetailsMap, file_info::FileInfo, paths};

const DEFAULT_LOCATION: &str = "archive";

static BASE_URL: Mutex<String> = Mutex::new(String::new());

pub fn launch(app: fn() -> Element) {
    dioxus::launch(app);
}

pub fn initial_location() -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    Some(
        params
            .get("archive")
            .unwrap_or_else(|| DEFAULT_LOCATION.to_string()),
    )
}

pub fn remember_location(_location: &str) {}

pub async fn pick_location() -> Option<String> {
    None
}

pub async fn load_archive(location: String) -> Result<Archive> {
    let base = absolute_url(&location)?;
    let tree = fetch(&base, paths::NODE_AND_ALBUM_DATA_FILE)
        .await?
        .ok_or_else(|| anyhow!("no smugmug_syncer archive at {base}"))?;
    let image_map = fetch(&base, paths::ALBUM_IMAGE_MAP_FILE).await?;
    let props = fetch(&base, paths::PROPS_FILE).await?;

    // Parquet metadata (video info, EXIF) isn't read in the browser yet; videos fall back to
    // `<key>.mp4`, which is what the syncer stores for SmugMug's largest rendition.
    let archive = Archive::from_parts(
        base.clone(),
        &tree,
        image_map.as_deref(),
        props.as_deref(),
        HashMap::new(),
    )?;
    *BASE_URL.lock().unwrap() = base;
    Ok(archive)
}

pub async fn load_details(_location: String) -> Result<DetailsMap> {
    Ok(DetailsMap::new())
}

pub async fn load_metadata_json(
    _location: String,
    _image_key: String,
    _source_file: Option<String>,
) -> Result<Option<String>> {
    Ok(None)
}

/// Reading the artifact's own EXIF / codecs needs ranged fetches; not implemented on the web yet
pub async fn load_file_info(
    _location: String,
    _image_key: String,
    _video_ext: Option<String>,
) -> Result<FileInfo> {
    Ok(FileInfo::default())
}

/// Resolves the location against the page URL, so media URLs keep working when the router
/// changes the path.  The result has no trailing slash.
fn absolute_url(location: &str) -> Result<String> {
    let page = web_sys::window()
        .and_then(|w| w.location().href().ok())
        .ok_or_else(|| anyhow!("no page URL"))?;
    let dir = format!("{}/", location.trim_end_matches('/'));
    let url = web_sys::Url::new_with_base(&dir, &page)
        .map_err(|_| anyhow!("invalid archive URL: {location}"))?;
    Ok(url.href().trim_end_matches('/').to_string())
}

/// Fetches an archive file.  Returns `None` on 404.
async fn fetch(base: &str, relative: &str) -> Result<Option<Vec<u8>>> {
    let url = format!("{base}/{relative}");
    let response = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("{url}: {e}"))?;
    match response.status() {
        200 => Ok(Some(
            response.binary().await.map_err(|e| anyhow!("{url}: {e}"))?,
        )),
        404 => Ok(None),
        status => bail!("{url}: HTTP {status}"),
    }
}

fn media_url(relative: &str) -> String {
    format!("{}/{relative}", BASE_URL.lock().unwrap())
}

/// The browser has no thumbnail cache, so thumbnails are the full image scaled by CSS
pub fn thumb_url(image_key: &str) -> String {
    image_url(image_key)
}

pub fn image_url(image_key: &str) -> String {
    media_url(&paths::image_artifact(image_key))
}

pub fn video_url(image_key: &str, ext: &str) -> String {
    media_url(&paths::video_artifact(image_key, ext))
}

pub fn use_media_handler() {}
