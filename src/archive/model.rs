//! Serde types for the files `smugmug_syncer` writes to `.smugmug_db`.
//!
//! The syncer serializes `smugmug::v2` types with SmugMug's PascalCase field names.  Only the
//! fields the viewer uses are declared here; everything defaults so files written by older or
//! newer syncer versions still load.

use serde::Deserialize;

/// `node_and_album_tree.db` (zstd JSON): a flattened `tree_ds` tree of SmugMug nodes, where
/// album nodes carry their album as the value and each entry repeats its parent node.
#[derive(Debug, Deserialize)]
pub struct TreeFile {
    #[serde(default)]
    pub nodes: Vec<TreeEntry>,
}

#[derive(Debug, Deserialize)]
pub struct TreeEntry {
    pub node_id: Node,
    #[serde(default)]
    pub value: Option<Album>,
    #[serde(default)]
    pub parent: Option<Node>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Node {
    #[serde(rename = "NodeID")]
    pub node_id: String,
    pub name: String,
    pub url_name: String,
    pub web_uri: String,
    pub privacy: String,
    #[serde(rename = "Type")]
    pub node_type: String,
    pub is_root: bool,
    pub date_added: Option<String>,
    pub date_modified: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Album {
    pub album_key: String,
    pub name: String,
    pub description: String,
    pub web_uri: String,
    pub privacy: String,
    pub image_count: u64,
    pub total_sizes: Option<u64>,
    pub original_sizes: Option<u64>,
    pub date: Option<String>,
    pub images_last_updated: Option<String>,
    pub last_updated: Option<String>,
}

/// One entry of `album_image_map.db` (zstd JSON): album key -> images in album order.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Image {
    pub image_key: String,
    pub file_name: String,
    pub title: String,
    pub caption: String,
    pub format: String,
    pub keyword_array: Vec<String>,
    #[serde(rename = "ArchivedMD5")]
    pub archived_md5: Option<String>,
    pub archived_size: Option<u64>,
    pub is_video: bool,
    pub hidden: bool,
    pub date_time_uploaded: Option<String>,
    pub last_updated: Option<String>,
    pub latitude: Option<String>,
    pub longitude: Option<String>,
}

impl Image {
    /// Title if it has one, otherwise the uploaded file name
    pub fn display_name(&self) -> &str {
        if self.title.trim().is_empty() {
            &self.file_name
        } else {
            &self.title
        }
    }
}

/// `props.json`: syncer-wide state.  Read only; unknown keys are ignored.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Props {
    pub album_node_last_sync_ts: Option<String>,
    pub image_map_last_sync_ts: Option<String>,
}

/// Columns of the `image_metadata` parquet files (EXIF/IPTC SmugMug holds for each artifact).
/// `metadata_json` is loaded on demand, see [`crate::archive::Archive`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImageDetails {
    pub date_time_original: Option<String>,
    pub original_width: Option<u64>,
    pub original_height: Option<u64>,
    pub original_size: Option<u64>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    /// Name of the parquet file the row came from, so `metadata_json` can be read from just it
    pub source_file: Option<String>,
}

/// Columns of the `video_info` parquet files (the video downloaded for a video artifact)
#[derive(Debug, Clone, PartialEq)]
pub struct VideoInfo {
    pub ext: String,
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub duration: Option<String>,
}
