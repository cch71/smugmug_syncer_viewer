//! What the artifact file itself says about a photo or video (its embedded EXIF, or its
//! container's tracks), and how that is merged with the metadata SmugMug holds.
//!
//! SmugMug's metadata (`image_metadata` parquet, see the syncer's
//! `smugmug_image_metadata.rs`) survives even when the downloaded file had its EXIF stripped,
//! while the file can carry tags SmugMug doesn't report.  The info panel shows both: SmugMug's
//! value first, the file's as a fallback, and both when they disagree.

use super::model::ImageDetails;

/// Information read from the artifact file
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileInfo {
    /// Every EXIF tag in the file as (name, value).  Tags SmugMug also reports use SmugMug's
    /// metadata name (see [`smugmug_name`]) so the two can be merged.
    pub fields: Vec<(String, String)>,
    pub date_time_original: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
    /// Aperture, shutter speed, ISO and focal length on one line
    pub exposure: Option<String>,
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    /// Tracks and codecs, for videos
    pub media: Option<MediaStreams>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaStreams {
    /// Major brand from `ftyp`, e.g. `isom`, `mp42`, `qt`
    pub brand: Option<String>,
    pub duration_secs: Option<f64>,
    /// Creation time from `mvhd`, UTC
    pub created: Option<String>,
    pub tracks: Vec<TrackInfo>,
}

impl MediaStreams {
    pub fn track(&self, kind: TrackKind) -> Option<&TrackInfo> {
        self.tracks.iter().find(|v| v.kind == kind)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackInfo {
    pub kind: TrackKind,
    /// Readable codec name, e.g. `H.264 / AVC (High, level 4.0)` or `AAC-LC`
    pub codec: String,
    /// Sample entry four character code, e.g. `avc1`, `mp4a`
    pub fourcc: String,
    /// Resolution, frame rate, channels, sample rate…
    pub details: Vec<String>,
}

impl TrackInfo {
    /// Codec followed by its details, e.g. `AAC-LC · stereo · 48.0 kHz`
    pub fn summary(&self) -> String {
        std::iter::once(self.codec.as_str())
            .chain(self.details.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrackKind {
    Video,
    Audio,
    Other(String),
}

/// Where a merged value came from
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    SmugMug,
    File,
    /// Both report it and the values agree
    Both,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergedField {
    pub name: String,
    pub value: String,
    pub source: Source,
    /// The file's value, when it differs from SmugMug's
    pub file_value: Option<String>,
}

/// Merges SmugMug's metadata fields with the file's into one list sorted by name
pub fn merge_fields(smugmug: &[(String, String)], file: &[(String, String)]) -> Vec<MergedField> {
    let mut merged: Vec<MergedField> = smugmug
        .iter()
        .map(|(name, value)| MergedField {
            name: name.clone(),
            value: value.clone(),
            source: Source::SmugMug,
            file_value: None,
        })
        .collect();

    for (name, value) in file {
        match merged.iter_mut().find(|v| &v.name == name) {
            Some(existing) if existing.source == Source::File => {}
            Some(existing) if same_value(&existing.value, value) => existing.source = Source::Both,
            Some(existing) => existing.file_value = Some(value.clone()),
            None => merged.push(MergedField {
                name: name.clone(),
                value: value.clone(),
                source: Source::File,
                file_value: None,
            }),
        }
    }
    merged.sort_by(|a, b| a.name.cmp(&b.name));
    merged
}

/// Compares values written in different styles: `f/1.6` vs `1.6`, `2024:04:03 17:07:47` vs
/// `2024-04-03T17:07:47`, `4.2 mm` vs `4.2mm`
fn same_value(a: &str, b: &str) -> bool {
    fn normalize(v: &str) -> String {
        let v: String = v
            .trim_start_matches("f/")
            .chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect();
        // Timestamps: drop the ISO 8601 date/time separator
        if v.chars().all(|c| c.is_ascii_digit() || c == 't') {
            v.replace('t', "")
        } else {
            v
        }
    }
    let (a, b) = (normalize(a), normalize(b));
    a == b || (!a.is_empty() && !b.is_empty() && (a.starts_with(&b) || b.starts_with(&a)))
}

/// SmugMug's metadata name for an EXIF tag (as `kamadak-exif` names it), when it reports the
/// same thing.  Tags not listed keep their EXIF name.
pub fn smugmug_name(exif_tag: &str) -> Option<&'static str> {
    Some(match exif_tag {
        "Make" => "Make",
        "Model" => "Model",
        "LensModel" => "Lens",
        "LensSerialNumber" => "LensSerialNumber",
        "BodySerialNumber" => "SerialNumber",
        "FNumber" => "Aperture",
        "ExposureTime" => "Exposure",
        "PhotographicSensitivity" => "ISO",
        "FocalLength" => "FocalLength",
        "FocalLengthIn35mmFilm" => "FocalLength35mm",
        "ExposureBiasValue" => "ExposureCompensation",
        "ExposureProgram" => "ExposureProgram",
        "ExposureMode" => "ExposureMode",
        "MeteringMode" => "Metering",
        "Flash" => "Flash",
        "WhiteBalance" => "WhiteBalance",
        "LightSource" => "LightSource",
        "SceneCaptureType" => "SceneCaptureType",
        "SensingMethod" => "SensingMethod",
        "Contrast" => "Contrast",
        "Saturation" => "Saturation",
        "Sharpness" => "Sharpness",
        "GainControl" => "GainControl",
        "DigitalZoomRatio" => "DigitalZoomRatio",
        "SubjectDistance" => "SubjectDistance",
        "SubjectDistanceRange" => "SubjectRange",
        "ColorSpace" => "ColorSpace",
        "BrightnessValue" => "Brightness",
        "CompressedBitsPerPixel" => "CompressedBitsPerPixel",
        "Software" => "Software",
        "Artist" => "Author",
        "Copyright" => "Copyright",
        "ImageDescription" => "Caption",
        "UserComment" => "UserComment",
        "DateTimeOriginal" => "DateTimeCreated",
        "DateTimeDigitized" => "DateDigitized",
        "DateTime" => "DateTimeModified",
        "GPSLatitude" => "Latitude",
        "GPSLatitudeRef" => "LatitudeReference",
        "GPSLongitude" => "Longitude",
        "GPSLongitudeRef" => "LongitudeReference",
        "GPSAltitude" => "Altitude",
        _ => return None,
    })
}

/// Fields describing a video's streams, named like SmugMug's metadata (`VideoCodec`,
/// `AudioCodec`, `Duration`) so they merge with it
pub fn media_fields(media: &MediaStreams) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    if let Some(track) = media.track(TrackKind::Video) {
        fields.push(("VideoCodec".to_string(), track.codec.clone()));
    }
    if let Some(track) = media.track(TrackKind::Audio) {
        fields.push(("AudioCodec".to_string(), track.codec.clone()));
    }
    if let Some(duration) = media.duration_secs {
        fields.push(("Duration".to_string(), format!("{duration:.1}")));
    }
    if let Some(brand) = &media.brand {
        fields.push(("Container".to_string(), brand.clone()));
    }
    if let Some(created) = &media.created {
        fields.push(("DateTimeCreated".to_string(), created.clone()));
    }
    fields
}

/// The summary rows at the top of the info panel, SmugMug first and the file as fallback
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub taken: Option<String>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub exposure: Option<String>,
    pub dimensions: Option<(u64, u64)>,
    pub location: Option<(f64, f64)>,
}

pub fn summarize(details: Option<&ImageDetails>, file: Option<&FileInfo>) -> Summary {
    let from_details = |f: fn(&ImageDetails) -> Option<String>| details.and_then(f);
    let from_file = |f: fn(&FileInfo) -> Option<String>| file.and_then(f);

    let camera = |make: Option<&str>, model: Option<&str>| {
        let parts: Vec<&str> = [make, model].into_iter().flatten().collect();
        // Many models already start with the make, e.g. "Canon EOS Rebel SL3"
        match parts.as_slice() {
            [make, model] if model.starts_with(make) => Some(model.to_string()),
            [] => None,
            _ => Some(parts.join(" ")),
        }
    };

    Summary {
        taken: from_details(|d| d.date_time_original.clone())
            .or_else(|| from_file(|f| f.date_time_original.clone()))
            .or_else(|| file.and_then(|f| f.media.as_ref()?.created.clone())),
        camera: details
            .and_then(|d| camera(d.make.as_deref(), d.model.as_deref()))
            .or_else(|| file.and_then(|f| camera(f.make.as_deref(), f.model.as_deref()))),
        lens: from_file(|f| f.lens.clone()),
        exposure: from_file(|f| f.exposure.clone()),
        dimensions: details
            .and_then(|d| Some((d.original_width?, d.original_height?)))
            .or_else(|| file.and_then(|f| Some((f.width?, f.height?)))),
        location: details
            .and_then(|d| Some((d.latitude?, d.longitude?)))
            .or_else(|| file.and_then(|f| Some((f.latitude?, f.longitude?)))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn merges_sources() {
        let smugmug = pairs(&[
            ("Aperture", "1.6"),
            ("DateTimeCreated", "2024-04-03T17:07:47"),
            ("Make", "Apple"),
        ]);
        let file = pairs(&[
            ("Aperture", "f/1.6"),
            ("DateTimeCreated", "2024:04:03 17:07:47"),
            ("Make", "Samsung"),
            ("Orientation", "row 0 at top"),
        ]);
        let merged = merge_fields(&smugmug, &file);
        let get = |name: &str| merged.iter().find(|v| v.name == name).unwrap();
        assert_eq!(get("Aperture").source, Source::Both);
        assert_eq!(get("DateTimeCreated").source, Source::Both);
        assert_eq!(get("Make").source, Source::SmugMug);
        assert_eq!(get("Make").file_value.as_deref(), Some("Samsung"));
        assert_eq!(get("Orientation").source, Source::File);
    }

    #[test]
    fn summary_prefers_smugmug_and_falls_back_to_file() {
        let details = ImageDetails {
            make: Some("Canon".into()),
            model: Some("Canon EOS Rebel SL3".into()),
            ..Default::default()
        };
        let file = FileInfo {
            make: Some("Other".into()),
            width: Some(6000),
            height: Some(4000),
            latitude: Some(1.5),
            longitude: Some(2.5),
            ..Default::default()
        };
        let summary = summarize(Some(&details), Some(&file));
        assert_eq!(summary.camera.as_deref(), Some("Canon EOS Rebel SL3"));
        assert_eq!(summary.dimensions, Some((6000, 4000)));
        assert_eq!(summary.location, Some((1.5, 2.5)));
    }
}
