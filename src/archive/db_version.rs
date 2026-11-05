//! Compatibility check for the archive's database layout version.
//!
//! `smugmug_syncer` records the semver version of its `.smugmug_db` layout as `db_version` in
//! `props.json` (its `DB_VERSION`).  Following semver, a different major version (or, while
//! the major version is 0, a different minor version) is a breaking change this viewer can't
//! read.  A newer minor version only adds data, which this viewer ignores.

use anyhow::{Result, bail};
use semver::Version;

/// The newest `.smugmug_db` layout this viewer was written against.  Raise it when the viewer
/// is updated for a newer syncer layout.
pub const SUPPORTED_DB_VERSION: &str = "1.0.0";

/// Version assumed for archives written before the syncer recorded `db_version`
pub const LEGACY_DB_VERSION: &str = "1.0.0";

/// Checks the archive's `db_version` against [`SUPPORTED_DB_VERSION`].  Returns an error for
/// an incompatible (or unparsable) version, and `Ok(Some(notice))` for a compatible archive
/// written with a newer minor version, whose extra data isn't shown.
pub fn check(stored: Option<&str>) -> Result<Option<String>> {
    let ours = Version::parse(SUPPORTED_DB_VERSION).expect("SUPPORTED_DB_VERSION is semver");
    let stored_str = stored.unwrap_or(LEGACY_DB_VERSION);
    let Ok(stored) = Version::parse(stored_str) else {
        bail!("The archive's database version '{stored_str}' isn't a valid version.");
    };

    let same_series = stored.major == ours.major && (ours.major != 0 || stored.minor == ours.minor);
    if !same_series {
        let supported = if ours.major == 0 {
            format!("0.{}.x", ours.minor)
        } else {
            format!("{}.x", ours.major)
        };
        if stored > ours {
            bail!(
                "The archive's database is version {stored}, written by a newer smugmug_syncer. \
                 This viewer reads version {supported}; install a newer viewer."
            );
        }
        bail!(
            "The archive's database is version {stored}, which this viewer no longer reads \
             (it reads version {supported}). Use an older viewer release, or sync the archive \
             again with a current smugmug_syncer."
        );
    }

    // Patch versions don't change the layout, so only a newer minor version can hold data
    // this viewer doesn't know about
    if (stored.major, stored.minor) > (ours.major, ours.minor) {
        return Ok(Some(format!(
            "This archive's database (version {stored}) is newer than this viewer supports \
             ({ours}), so some information may not be shown. Install a newer viewer to see it."
        )));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bump(major: u64, minor: u64, patch: u64) -> String {
        let v = Version::parse(SUPPORTED_DB_VERSION).unwrap();
        format!(
            "{}.{}.{}",
            v.major + major,
            v.minor + minor,
            v.patch + patch
        )
    }

    #[test]
    fn missing_version_is_the_legacy_layout() {
        assert_eq!(check(None).unwrap(), None);
    }

    #[test]
    fn same_or_newer_patch_is_supported_silently() {
        assert_eq!(check(Some(SUPPORTED_DB_VERSION)).unwrap(), None);
        assert_eq!(check(Some(&bump(0, 0, 1))).unwrap(), None);
    }

    #[test]
    fn newer_minor_opens_with_a_notice() {
        assert!(check(Some(&bump(0, 1, 0))).unwrap().is_some());
    }

    #[test]
    fn other_major_or_bad_version_is_rejected() {
        let err = check(Some(&bump(1, 0, 0))).unwrap_err().to_string();
        assert!(err.contains("newer smugmug_syncer"), "{err}");
        assert!(check(Some("0.9.0")).is_err());
        assert!(check(Some("one")).is_err());
    }
}
