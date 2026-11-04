//! Reads the EXIF embedded in artifact files

use std::{
    fs::File,
    io::{BufReader, Cursor},
    path::Path,
};

use anyhow::{Context, Result};
use exif::{Exif, Field, In, Tag, Value};

use crate::archive::file_info::{FileInfo, smugmug_name};

/// Longest value kept in the field list; longer ones are binary blobs or vendor data
const MAX_VALUE_LEN: usize = 200;

/// Reads the image's EXIF.  A file without EXIF (or in a format that can't hold it) gives an
/// empty [`FileInfo`].
pub fn read_file_info(path: &Path) -> Result<FileInfo> {
    let file = File::open(path).with_context(|| format!("{} not found", path.display()))?;
    match exif::Reader::new().read_from_container(&mut BufReader::new(file)) {
        Ok(exif) => Ok(file_info(&exif)),
        Err(exif::Error::Io(err)) => Err(err.into()),
        Err(_) => Ok(FileInfo::default()),
    }
}

/// EXIF orientation (1-8) of an image held in memory, 1 when it has none
pub fn orientation(data: &[u8]) -> u32 {
    exif::Reader::new()
        .read_from_container(&mut Cursor::new(data))
        .ok()
        .and_then(|exif| {
            exif.get_field(Tag::Orientation, In::PRIMARY)
                .and_then(|v| v.value.get_uint(0))
        })
        .unwrap_or(1)
}

fn file_info(exif: &Exif) -> FileInfo {
    let get = |tag| exif.get_field(tag, In::PRIMARY);
    let text = |tag| get(tag).map(|f| display(exif, f)).filter(|v| !v.is_empty());

    let mut fields = Vec::new();
    for field in exif.fields().filter(|f| f.ifd_num == In::PRIMARY) {
        if matches!(field.tag, Tag::MakerNote) || is_blob(&field.value) {
            continue;
        }
        let tag_name = field.tag.to_string();
        let value = match field.tag {
            Tag::GPSLatitude | Tag::GPSLongitude => match gps_coordinate(exif, field.tag) {
                Some(v) => format!("{v:.6}"),
                None => continue,
            },
            _ => display(exif, field),
        };
        if value.is_empty() || value.len() > MAX_VALUE_LEN {
            continue;
        }
        let name = smugmug_name(&tag_name).map_or(tag_name, str::to_string);
        fields.push((name, value));
    }

    let exposure: Vec<String> = [
        text(Tag::FNumber),
        text(Tag::ExposureTime),
        text(Tag::PhotographicSensitivity).map(|v| format!("ISO {v}")),
        text(Tag::FocalLength),
    ]
    .into_iter()
    .flatten()
    .collect();

    let dimension = |a, b| {
        get(a)
            .or_else(|| get(b))
            .and_then(|f| f.value.get_uint(0))
            .map(u64::from)
    };

    FileInfo {
        fields,
        date_time_original: text(Tag::DateTimeOriginal)
            .or_else(|| text(Tag::DateTime))
            .map(|v| exif_date_to_iso(&v)),
        make: text(Tag::Make),
        model: text(Tag::Model),
        lens: text(Tag::LensModel),
        exposure: (!exposure.is_empty()).then(|| exposure.join(" · ")),
        width: dimension(Tag::PixelXDimension, Tag::ImageWidth),
        height: dimension(Tag::PixelYDimension, Tag::ImageLength),
        latitude: gps_coordinate(exif, Tag::GPSLatitude),
        longitude: gps_coordinate(exif, Tag::GPSLongitude),
        media: None,
    }
}

/// Field value as text: strings unquoted, everything else with its unit
fn display(exif: &Exif, field: &Field) -> String {
    match &field.value {
        Value::Ascii(parts) => parts
            .iter()
            .map(|v| {
                String::from_utf8_lossy(v)
                    .trim_matches(char::from(0))
                    .trim()
                    .to_string()
            })
            .filter(|v| !v.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        _ => shorten_numbers(&field.display_value().with_unit(exif).to_string()),
    }
}

/// Rounds long decimals from rational values, e.g. `332.79414542020777 meters` to
/// `332.794 meters`
fn shorten_numbers(text: &str) -> String {
    text.split(' ')
        .map(|word| match word.split_once('.') {
            Some((_, decimals)) if decimals.len() > 3 => match word.parse::<f64>() {
                Ok(v) => format!("{v:.3}").trim_end_matches('0').to_string(),
                Err(_) => word.to_string(),
            },
            _ => word.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_blob(value: &Value) -> bool {
    matches!(value, Value::Undefined(data, _) if data.len() > 64)
}

/// Signed decimal degrees from a GPS latitude/longitude and its N/S/E/W reference
fn gps_coordinate(exif: &Exif, tag: Tag) -> Option<f64> {
    let ref_tag = if tag == Tag::GPSLatitude {
        Tag::GPSLatitudeRef
    } else {
        Tag::GPSLongitudeRef
    };
    let Value::Rational(parts) = &exif.get_field(tag, In::PRIMARY)?.value else {
        return None;
    };
    let degrees = parts
        .iter()
        .zip([1.0, 60.0, 3600.0])
        .map(|(v, div)| v.to_f64() / div)
        .sum::<f64>();
    if !degrees.is_finite() || degrees == 0.0 {
        return None;
    }
    let negative = exif
        .get_field(ref_tag, In::PRIMARY)
        .and_then(|f| match &f.value {
            Value::Ascii(v) => v.first().and_then(|v| v.first().copied()),
            _ => None,
        })
        .is_some_and(|c| c == b'S' || c == b'W');
    Some(if negative { -degrees } else { degrees })
}

/// `2024:04:03 17:07:47` -> `2024-04-03T17:07:47`
fn exif_date_to_iso(value: &str) -> String {
    match value.split_once(' ') {
        Some((date, time)) => format!("{}T{time}", date.replace(':', "-")),
        None => value.replace(':', "-"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(
            shorten_numbers("332.79414542020777 meters"),
            "332.794 meters"
        );
        assert_eq!(shorten_numbers("1.5 EV"), "1.5 EV");
        assert_eq!(
            shorten_numbers("3.9900000095374253-6.6 mm"),
            "3.9900000095374253-6.6 mm"
        );
    }

    #[test]
    fn dates() {
        assert_eq!(
            exif_date_to_iso("2024:04:03 17:07:47"),
            "2024-04-03T17:07:47"
        );
    }
}
