//! The viewer's own settings, kept in the OS config directory (never inside the archive)

use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
struct Settings {
    #[serde(default)]
    last_archive: Option<String>,
    /// Keys this version doesn't know about, kept when the file is rewritten
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

pub fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("dev", "c13s", "smugmug_syncer_viewer")
}

fn settings_file() -> Option<PathBuf> {
    Some(project_dirs()?.config_dir().join("settings.json"))
}

fn read() -> Settings {
    settings_file()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default()
}

pub fn last_location() -> Option<String> {
    read().last_archive
}

pub fn remember_location(location: &str) {
    let Some(path) = settings_file() else { return };
    let mut settings = read();
    settings.last_archive = Some(location.to_string());
    let result = std::fs::create_dir_all(path.parent().unwrap_or(&path))
        .and_then(|_| std::fs::write(&path, serde_json::to_vec_pretty(&settings)?));
    if let Err(err) = result {
        eprintln!("couldn't save settings to {}: {err}", path.display());
    }
}
