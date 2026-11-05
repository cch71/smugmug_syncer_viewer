//! In-memory index of a `smugmug_syncer` archive.
//!
//! Everything here is platform neutral: it is built from the raw bytes of the archive's files,
//! which [`crate::platform`] reads from disk (desktop) or over HTTP (web/PWA).

// Reading files' own EXIF/codecs is desktop only for now (see docs/PWA.md), so parts of these
// are unused in the web build
pub mod db_version;
#[cfg_attr(not(feature = "desktop"), allow(dead_code))]
pub mod file_info;
pub mod model;
#[cfg_attr(not(feature = "desktop"), allow(dead_code))]
pub mod mp4;
pub mod paths;

use std::{collections::HashMap, io::Read, sync::Arc};

use anyhow::{Context, Result};

use model::{Album, Image, ImageDetails, Node, Props, TreeFile, VideoInfo};

/// A folder or album in the SmugMug node tree
#[derive(Debug, Clone, PartialEq)]
pub struct FolderNode {
    pub node: Node,
    pub album: Option<Album>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// Number of images in this album, or in every album below this folder
    pub total_images: usize,
}

impl FolderNode {
    pub fn display_name(&self) -> &str {
        match (&self.album, self.node.name.is_empty()) {
            (_, false) => &self.node.name,
            (Some(album), true) if !album.name.is_empty() => &album.name,
            _ => "Home",
        }
    }
}

/// An image together with where it lives in the tree
#[derive(Debug, Clone, PartialEq)]
pub struct ImageRef {
    pub album_key: String,
    pub image: Arc<Image>,
}

#[derive(Debug, PartialEq)]
pub struct Archive {
    /// Where the archive was opened from (folder path or base URL)
    pub location: String,
    pub nodes: Vec<FolderNode>,
    pub root: usize,
    node_by_id: HashMap<String, usize>,
    node_by_album: HashMap<String, usize>,
    album_images: HashMap<String, Vec<Arc<Image>>>,
    /// Image key -> album keys containing it
    image_albums: HashMap<String, Vec<String>>,
    pub videos: HashMap<String, VideoInfo>,
    pub props: Props,
    /// Set when the archive was written with a newer, still compatible, database version
    /// (see [`db_version::check`]), for the UI to show
    pub db_notice: Option<String>,
}

impl Archive {
    /// Builds the index from the contents of `node_and_album_tree.db`, `album_image_map.db`
    /// (both zstd compressed JSON) and `props.json`.
    pub fn from_parts(
        location: String,
        tree_db: &[u8],
        image_map_db: Option<&[u8]>,
        props_json: Option<&[u8]>,
        videos: HashMap<String, VideoInfo>,
    ) -> Result<Self> {
        // Checked first, so an archive this viewer can't read fails with a clear message
        // rather than a parse error
        let props: Props = props_json
            .and_then(|v| serde_json::from_slice(v).ok())
            .unwrap_or_default();
        let db_notice = db_version::check(props.db_version.as_deref())?;

        let tree: TreeFile = serde_json::from_slice(&zstd_decode(tree_db)?)
            .context("parsing node_and_album_tree.db")?;
        let album_images: HashMap<String, Vec<Arc<Image>>> = match image_map_db {
            Some(data) => {
                let map: HashMap<String, Vec<Image>> = serde_json::from_slice(&zstd_decode(data)?)
                    .context("parsing album_image_map.db")?;
                map.into_iter()
                    .map(|(k, v)| (k, v.into_iter().map(Arc::new).collect()))
                    .collect()
            }
            None => HashMap::new(),
        };

        let mut archive = Self {
            location,
            nodes: Vec::with_capacity(tree.nodes.len()),
            root: 0,
            node_by_id: HashMap::new(),
            node_by_album: HashMap::new(),
            album_images,
            image_albums: HashMap::new(),
            videos,
            props,
            db_notice,
        };
        archive.build_tree(tree)?;
        archive.index_images();
        Ok(archive)
    }

    fn build_tree(&mut self, tree: TreeFile) -> Result<()> {
        let mut parents = Vec::with_capacity(tree.nodes.len());
        for entry in tree.nodes {
            let idx = self.nodes.len();
            self.node_by_id.insert(entry.node_id.node_id.clone(), idx);
            if let Some(album) = &entry.value {
                self.node_by_album.insert(album.album_key.clone(), idx);
            }
            parents.push(entry.parent.map(|v| v.node_id));
            self.nodes.push(FolderNode {
                node: entry.node_id,
                album: entry.value,
                parent: None,
                children: Vec::new(),
                total_images: 0,
            });
        }

        let mut root = None;
        for (idx, parent_id) in parents.into_iter().enumerate() {
            match parent_id.and_then(|id| self.node_by_id.get(&id).copied()) {
                Some(parent) => {
                    self.nodes[idx].parent = Some(parent);
                    self.nodes[parent].children.push(idx);
                }
                None => {
                    root.get_or_insert(idx);
                }
            }
        }
        self.root = root.context("node tree is empty")?;

        // Image counts, children before parents.  Entries are written breadth first, so walking
        // backwards visits every child before its parent.
        for idx in (0..self.nodes.len()).rev() {
            let own = self.nodes[idx]
                .album
                .as_ref()
                .map(|a| {
                    self.album_images
                        .get(&a.album_key)
                        .map_or(a.image_count as usize, Vec::len)
                })
                .unwrap_or(0);
            let children: usize = self.nodes[idx]
                .children
                .iter()
                .map(|c| self.nodes[*c].total_images)
                .sum();
            self.nodes[idx].total_images = own + children;
        }
        Ok(())
    }

    fn index_images(&mut self) {
        for (album_key, images) in &self.album_images {
            for image in images {
                self.image_albums
                    .entry(image.image_key.clone())
                    .or_default()
                    .push(album_key.clone());
            }
        }
    }

    pub fn node(&self, node_id: &str) -> Option<(usize, &FolderNode)> {
        let idx = *self.node_by_id.get(node_id)?;
        Some((idx, &self.nodes[idx]))
    }

    pub fn album_node(&self, album_key: &str) -> Option<(usize, &FolderNode)> {
        let idx = *self.node_by_album.get(album_key)?;
        Some((idx, &self.nodes[idx]))
    }

    pub fn album_images(&self, album_key: &str) -> &[Arc<Image>] {
        self.album_images.get(album_key).map_or(&[], Vec::as_slice)
    }

    pub fn albums_containing(&self, image_key: &str) -> &[String] {
        self.image_albums.get(image_key).map_or(&[], Vec::as_slice)
    }

    /// Nodes from the root down to (and including) `idx`
    pub fn ancestry(&self, mut idx: usize) -> Vec<usize> {
        let mut path = vec![idx];
        while let Some(parent) = self.nodes[idx].parent {
            path.push(parent);
            idx = parent;
        }
        path.reverse();
        path
    }

    /// First image of the album, or of the first album below the folder, for use as a cover
    pub fn cover_image(&self, idx: usize) -> Option<&Arc<Image>> {
        let node = &self.nodes[idx];
        if let Some(album) = &node.album {
            return self
                .album_images(&album.album_key)
                .iter()
                .find(|v| !v.hidden)
                .or_else(|| self.album_images(&album.album_key).first());
        }
        node.children.iter().find_map(|c| self.cover_image(*c))
    }

    pub fn album_count(&self) -> usize {
        self.node_by_album.len()
    }

    pub fn image_count(&self) -> usize {
        self.image_albums.len()
    }

    pub fn video_count(&self) -> usize {
        self.album_images
            .values()
            .flatten()
            .filter(|v| v.is_video)
            .map(|v| v.image_key.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

    /// Case-insensitive search over file names, titles, captions and keywords.  Every word of
    /// the query has to match.  Results are de-duplicated by image key.
    pub fn search(&self, query: &str, limit: usize) -> Vec<ImageRef> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        if words.is_empty() {
            return Vec::new();
        }

        let mut seen = std::collections::HashSet::new();
        let mut results = Vec::new();
        // Walk albums in tree order so results are stable
        for node in &self.nodes {
            let Some(album) = &node.album else { continue };
            let album_name = node.display_name().to_lowercase();
            for image in self.album_images(&album.album_key) {
                let haystack = format!(
                    "{} {} {} {} {}",
                    image.file_name,
                    image.title,
                    image.caption,
                    image.keyword_array.join(" "),
                    album_name
                )
                .to_lowercase();
                if words.iter().all(|w| haystack.contains(w.as_str()))
                    && seen.insert(image.image_key.as_str())
                {
                    results.push(ImageRef {
                        album_key: album.album_key.clone(),
                        image: image.clone(),
                    });
                    if results.len() >= limit {
                        return results;
                    }
                }
            }
        }
        results
    }

    /// Albums or folders whose name matches every word of the query
    pub fn search_nodes(&self, query: &str, limit: usize) -> Vec<usize> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        if words.is_empty() {
            return Vec::new();
        }
        (0..self.nodes.len())
            .filter(|i| *i != self.root)
            .filter(|i| {
                let name = self.nodes[*i].display_name().to_lowercase();
                words.iter().all(|w| name.contains(w.as_str()))
            })
            .take(limit)
            .collect()
    }
}

/// Image details loaded after the archive opens (they come from many parquet files)
pub type DetailsMap = HashMap<String, ImageDetails>;

fn zstd_decode(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder =
        ruzstd::decoding::StreamingDecoder::new(data).context("reading zstd header")?;
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).context("decompressing")?;
    Ok(out)
}

/// Formats a byte count for display
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Trims an RFC 3339 timestamp to `YYYY-MM-DD HH:MM` for display
pub fn short_date(ts: &str) -> String {
    let ts = ts.replace('T', " ");
    ts.chars().take(16).collect()
}
