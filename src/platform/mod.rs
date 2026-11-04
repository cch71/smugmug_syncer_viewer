//! Everything that differs between the desktop app and the web/PWA build.
//!
//! Both back ends expose the same functions, so the UI never needs a `cfg`:
//!
//! - `launch`: start Dioxus with platform specific configuration
//! - `initial_location` / `remember_location` / `pick_location`: choose the archive
//! - `load_archive`, `load_details`, `load_metadata_json`: read the archive's metadata
//! - `load_file_info`: read the EXIF or video codecs recorded in an artifact file itself
//! - `thumb_url`, `image_url`, `video_url`: URLs the webview loads media from
//! - `use_media_handler`: hook that serves media (desktop only; a no-op on the web)

#[cfg(feature = "desktop")]
mod desktop;
#[cfg(feature = "desktop")]
pub use desktop::*;

#[cfg(all(feature = "web", not(feature = "desktop")))]
mod web;
#[cfg(all(feature = "web", not(feature = "desktop")))]
pub use web::*;

#[cfg(not(any(feature = "desktop", feature = "web")))]
compile_error!("enable either the `desktop` or `web` feature");

/// True if the platform can open a native folder picker
pub const HAS_FOLDER_PICKER: bool = cfg!(feature = "desktop");

/// True if the platform reads the parquet metadata and the artifacts' own EXIF/codecs
pub const HAS_FULL_METADATA: bool = cfg!(feature = "desktop");
