//! Archive-relative locations of the files `smugmug_syncer` writes.  Mirrors the syncer's
//! `PathFinder` (`smugmug_syncer/src/path_finder.rs`); keep the two in step.

pub const NODE_AND_ALBUM_DATA_FILE: &str = ".smugmug_db/node_and_album_tree.db";
pub const ALBUM_IMAGE_MAP_FILE: &str = ".smugmug_db/album_image_map.db";
pub const PROPS_FILE: &str = ".smugmug_db/props.json";
// The parquet directories are only read on desktop for now (see docs/PWA.md)
#[cfg_attr(not(feature = "desktop"), allow(dead_code))]
pub const VIDEO_INFO_DIR: &str = ".smugmug_db/video_info";
#[cfg_attr(not(feature = "desktop"), allow(dead_code))]
pub const IMAGE_METADATA_DIR: &str = ".smugmug_db/image_metadata";
pub const ARTIFACTS_DIR: &str = "artifacts";

/// Image artifacts are stored under their image key with no extension.  For videos this file
/// is the poster frame.
pub fn image_artifact(image_key: &str) -> String {
    format!("{ARTIFACTS_DIR}/{image_key}")
}

/// Videos are stored as `<image key>.<ext>` (ext from `video_info`)
pub fn video_artifact(image_key: &str, ext: &str) -> String {
    format!("{ARTIFACTS_DIR}/{image_key}.{ext}")
}
