//! Minimal ISO-BMFF (MP4 / QuickTime) reader that reports a video's tracks and codecs.
//!
//! Only the `moov` box is read: the media data is skipped with seeks, so probing a large video
//! costs a few small reads.  No dependencies, so it also builds for wasm.

use std::io::{Read, Seek, SeekFrom};

use anyhow::{Result, bail};

use super::file_info::{MediaStreams, TrackInfo, TrackKind};

/// Largest `moov` box read into memory
const MAX_MOOV: u64 = 64 * 1024 * 1024;
/// Seconds from 1904-01-01 (MP4 epoch) to 1970-01-01
const MP4_EPOCH_OFFSET: u64 = 2_082_844_800;

/// Reads the tracks of an MP4/MOV file
pub fn probe<R: Read + Seek>(reader: &mut R) -> Result<MediaStreams> {
    let len = reader.seek(SeekFrom::End(0))?;
    let mut pos = 0;
    let mut brand = None;
    while pos + 8 <= len {
        reader.seek(SeekFrom::Start(pos))?;
        let mut header = [0u8; 16];
        reader.read_exact(&mut header[..8])?;
        let mut size = u64::from(be_u32(&header, 0));
        let kind = [header[4], header[5], header[6], header[7]];
        let mut header_len = 8;
        if size == 1 {
            reader.read_exact(&mut header[8..16])?;
            size = be_u64(&header, 8);
            header_len = 16;
        } else if size == 0 {
            size = len - pos;
        }
        if size < header_len {
            bail!("corrupt box at {pos}");
        }

        match &kind {
            b"ftyp" => {
                let mut major = [0u8; 4];
                reader.read_exact(&mut major)?;
                brand = Some(fourcc(&major));
            }
            b"moov" => {
                let body_len = size - header_len;
                if body_len > MAX_MOOV {
                    bail!("moov box too large ({body_len} bytes)");
                }
                let mut body = vec![0u8; body_len as usize];
                reader.read_exact(&mut body)?;
                let mut streams = parse_moov(&body)?;
                streams.brand = brand;
                return Ok(streams);
            }
            _ => {}
        }
        pos += size;
    }
    bail!("no moov box (not an MP4/QuickTime file?)")
}

fn parse_moov(moov: &[u8]) -> Result<MediaStreams> {
    let mut streams = MediaStreams::default();
    for (kind, body) in boxes(moov) {
        match &kind {
            b"mvhd" => {
                if let Some((created, timescale, duration)) = parse_time_header(body) {
                    streams.created = created
                        .checked_sub(MP4_EPOCH_OFFSET)
                        .filter(|_| created > 0)
                        .map(unix_to_utc_string);
                    if timescale > 0 {
                        streams.duration_secs = Some(duration as f64 / timescale as f64);
                    }
                }
            }
            b"trak" => {
                if let Some(track) = parse_trak(body) {
                    streams.tracks.push(track);
                }
            }
            _ => {}
        }
    }
    // Video first, then audio, then anything else
    streams.tracks.sort_by_key(|t| match t.kind {
        TrackKind::Video => 0,
        TrackKind::Audio => 1,
        TrackKind::Other(_) => 2,
    });
    Ok(streams)
}

fn parse_trak(trak: &[u8]) -> Option<TrackInfo> {
    let mdia = child(trak, b"mdia")?;
    let handler = child(mdia, b"hdlr").and_then(|v| v.get(8..12))?;
    let (_, timescale, duration) = child(mdia, b"mdhd").and_then(parse_time_header)?;
    let stbl = child(child(mdia, b"minf")?, b"stbl")?;
    let stsd = child(stbl, b"stsd")?;
    // version/flags (4), entry count (4), then sample entries as boxes
    let (entry_kind, entry) = boxes(stsd.get(8..)?).next()?;
    let seconds = (timescale > 0).then(|| duration as f64 / timescale as f64);

    let track = match handler {
        b"vide" => {
            let mut details = Vec::new();
            let mut codec = video_codec_name(&entry_kind).to_string();
            // Visual sample entry: 8 bytes common header, 16 reserved, width, height at 24..28,
            // 78 bytes before the child boxes
            if let (Some(w), Some(h)) = (entry.get(24..26), entry.get(26..28)) {
                details.push(format!("{} × {}", be_u16(w, 0), be_u16(h, 0)));
            }
            if let Some(children) = entry.get(78..) {
                if let Some(profile) = child(children, b"avcC").and_then(avc_profile) {
                    codec = format!("{codec} ({profile})");
                } else if let Some(profile) = child(children, b"hvcC").and_then(hevc_profile) {
                    codec = format!("{codec} ({profile})");
                }
            }
            let samples = child(stbl, b"stts").map(sample_count).unwrap_or(0);
            if let Some(seconds) = seconds.filter(|v| *v > 0.0)
                && samples > 0
            {
                details.push(format!("{:.2} fps", samples as f64 / seconds));
            }
            TrackInfo {
                kind: TrackKind::Video,
                codec,
                fourcc: fourcc(&entry_kind),
                details,
            }
        }
        b"soun" => {
            let mut details = Vec::new();
            let mut codec = audio_codec_name(&entry_kind).to_string();
            // Audio sample entry: 8 bytes common header, version at 8, channels at 16,
            // sample rate (16.16 fixed) at 24
            let version = entry.get(8..10).map_or(0, |v| be_u16(v, 0));
            if let Some(channels) = entry.get(16..18).map(|v| be_u16(v, 0)) {
                details.push(channel_name(channels));
            }
            if let Some(rate) = entry.get(24..28).map(|v| be_u32(v, 0) >> 16)
                && rate > 0
            {
                details.push(format!("{:.1} kHz", rate as f64 / 1000.0));
            }
            // QuickTime sound description versions 1 and 2 add 16 and 36 bytes
            let children_at = 28 + [0, 16, 36].get(version as usize).copied().unwrap_or(0);
            if &entry_kind == b"mp4a"
                && let Some(children) = entry.get(children_at..)
            {
                let esds = child(children, b"esds")
                    .or_else(|| child(children, b"wave").and_then(|w| child(w, b"esds")));
                if let Some(name) = esds.and_then(mpeg4_audio_name) {
                    codec = name.to_string();
                }
            }
            TrackInfo {
                kind: TrackKind::Audio,
                codec,
                fourcc: fourcc(&entry_kind),
                details,
            }
        }
        other => TrackInfo {
            kind: TrackKind::Other(fourcc(other.try_into().ok()?)),
            codec: fourcc(&entry_kind),
            fourcc: fourcc(&entry_kind),
            details: Vec::new(),
        },
    };
    Some(track)
}

/// `(creation time, timescale, duration)` from an `mvhd` or `mdhd` box
fn parse_time_header(body: &[u8]) -> Option<(u64, u32, u64)> {
    match *body.first()? {
        1 => Some((
            be_u64(body.get(4..12)?, 0),
            be_u32(body.get(20..24)?, 0),
            be_u64(body.get(24..32)?, 0),
        )),
        _ => Some((
            u64::from(be_u32(body.get(4..8)?, 0)),
            be_u32(body.get(12..16)?, 0),
            u64::from(be_u32(body.get(16..20)?, 0)),
        )),
    }
}

/// Total samples listed in an `stts` (time to sample) box
fn sample_count(stts: &[u8]) -> u64 {
    let entries = stts.get(4..8).map_or(0, |v| be_u32(v, 0)) as usize;
    (0..entries)
        .filter_map(|i| stts.get(8 + i * 8..12 + i * 8))
        .map(|v| u64::from(be_u32(v, 0)))
        .sum()
}

fn avc_profile(avcc: &[u8]) -> Option<String> {
    let (profile, level) = (*avcc.get(1)?, *avcc.get(3)?);
    let name = match profile {
        66 => "Baseline",
        77 => "Main",
        88 => "Extended",
        100 => "High",
        110 => "High 10",
        122 => "High 4:2:2",
        244 => "High 4:4:4",
        _ => {
            return Some(format!(
                "profile {profile}, level {:.1}",
                f64::from(level) / 10.0
            ));
        }
    };
    Some(format!("{name}, level {:.1}", f64::from(level) / 10.0))
}

fn hevc_profile(hvcc: &[u8]) -> Option<String> {
    let profile = *hvcc.get(1)? & 0x1F;
    let level = *hvcc.get(12)?;
    let name = match profile {
        1 => "Main",
        2 => "Main 10",
        3 => "Main Still Picture",
        _ => {
            return Some(format!(
                "profile {profile}, level {:.1}",
                f64::from(level) / 30.0
            ));
        }
    };
    Some(format!("{name}, level {:.1}", f64::from(level) / 30.0))
}

/// Names the codec in an `esds` box from its object type (and AAC audio object type)
fn mpeg4_audio_name(esds: &[u8]) -> Option<&'static str> {
    let mut data = esds.get(4..)?; // version/flags
    // ES_Descriptor (0x03) -> DecoderConfigDescriptor (0x04) -> DecoderSpecificInfo (0x05)
    let (tag, es) = descriptor(&mut data)?;
    if tag != 0x03 {
        return None;
    }
    let flags = *es.get(2)?;
    let mut skip = 3;
    if flags & 0x80 != 0 {
        skip += 2;
    }
    if flags & 0x40 != 0 {
        skip += 1 + usize::from(*es.get(skip)?);
    }
    if flags & 0x20 != 0 {
        skip += 2;
    }
    let mut rest = es.get(skip..)?;
    let (tag, config) = descriptor(&mut rest)?;
    if tag != 0x04 {
        return None;
    }
    let object_type = *config.first()?;
    let audio_object_type = config
        .get(13..)
        .and_then(|mut v| descriptor(&mut v))
        .filter(|(tag, _)| *tag == 0x05)
        .and_then(|(_, info)| info.first().map(|b| b >> 3));

    Some(match (object_type, audio_object_type) {
        (0x40 | 0x66..=0x68, Some(1)) => "AAC Main",
        (0x40 | 0x66..=0x68, Some(2)) => "AAC-LC",
        (0x40 | 0x66..=0x68, Some(5)) => "HE-AAC",
        (0x40 | 0x66..=0x68, Some(29)) => "HE-AAC v2",
        (0x40 | 0x66..=0x68, Some(23)) => "AAC-LD",
        (0x40 | 0x66..=0x68, Some(39)) => "AAC-ELD",
        (0x40 | 0x66..=0x68, _) => "AAC",
        (0x69 | 0x6B, _) => "MP3",
        (0xA5, _) => "AC-3",
        (0xA6, _) => "E-AC-3",
        (0xAD, _) => "Opus",
        _ => "MPEG-4 audio",
    })
}

/// Reads one MPEG-4 descriptor (tag, variable length size, body), advancing `data`
fn descriptor<'a>(data: &mut &'a [u8]) -> Option<(u8, &'a [u8])> {
    let tag = *data.first()?;
    let mut len = 0usize;
    let mut i = 1;
    loop {
        let b = *data.get(i)?;
        len = (len << 7) | usize::from(b & 0x7F);
        i += 1;
        if b & 0x80 == 0 || i > 4 {
            break;
        }
    }
    let body = data.get(i..i + len)?;
    *data = &data[i + len..];
    Some((tag, body))
}

fn video_codec_name(kind: &[u8; 4]) -> &'static str {
    match kind {
        b"avc1" | b"avc3" => "H.264 / AVC",
        b"hvc1" | b"hev1" => "H.265 / HEVC",
        b"av01" => "AV1",
        b"vp08" => "VP8",
        b"vp09" => "VP9",
        b"mp4v" => "MPEG-4 Part 2",
        b"s263" | b"h263" => "H.263",
        b"jpeg" | b"mjpa" | b"mjpb" => "Motion JPEG",
        b"apcn" | b"apch" | b"apcs" | b"apco" | b"ap4h" => "Apple ProRes",
        b"dvc " | b"dvcp" => "DV",
        _ => "Unknown video codec",
    }
}

fn audio_codec_name(kind: &[u8; 4]) -> &'static str {
    match kind {
        b"mp4a" => "MPEG-4 audio",
        b"samr" => "AMR-NB",
        b"sawb" => "AMR-WB",
        b"ac-3" => "AC-3",
        b"ec-3" => "E-AC-3",
        b"Opus" => "Opus",
        b"alac" => "Apple Lossless",
        b"fLaC" => "FLAC",
        b".mp3" => "MP3",
        b"lpcm" | b"sowt" | b"twos" | b"raw " | b"in24" | b"in32" | b"fl32" => "PCM",
        b"ulaw" => "µ-law",
        b"alaw" => "A-law",
        b"ima4" => "IMA ADPCM",
        _ => "Unknown audio codec",
    }
}

fn channel_name(channels: u16) -> String {
    match channels {
        1 => "mono".to_string(),
        2 => "stereo".to_string(),
        6 => "5.1".to_string(),
        n => format!("{n} channels"),
    }
}

/// Iterates the child boxes in `data` as (type, body)
fn boxes(data: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    let mut pos = 0usize;
    std::iter::from_fn(move || {
        let header = data.get(pos..pos + 8)?;
        let mut size = be_u32(header, 0) as usize;
        let kind = [header[4], header[5], header[6], header[7]];
        let mut header_len = 8;
        if size == 1 {
            size = usize::try_from(be_u64(data.get(pos + 8..pos + 16)?, 0)).ok()?;
            header_len = 16;
        } else if size == 0 {
            size = data.len() - pos;
        }
        let body = data.get(pos + header_len..pos.checked_add(size)?)?;
        pos += size.max(header_len);
        Some((kind, body))
    })
}

fn child<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(data).find(|(k, _)| k == kind).map(|(_, body)| body)
}

fn fourcc(kind: &[u8; 4]) -> String {
    String::from_utf8_lossy(kind).trim().to_string()
}

fn be_u16(data: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([data[at], data[at + 1]])
}

fn be_u32(data: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(data[at..at + 4].try_into().unwrap())
}

fn be_u64(data: &[u8], at: usize) -> u64 {
    u64::from_be_bytes(data[at..at + 8].try_into().unwrap())
}

/// Formats Unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (days-to-civil from Howard Hinnant)
fn unix_to_utc_string(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
    }

    #[test]
    fn dates() {
        assert_eq!(unix_to_utc_string(0), "1970-01-01T00:00:00Z");
        assert_eq!(unix_to_utc_string(1_700_000_000), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn probes_h264_and_aac() {
        // mvhd v0: version/flags, created, modified, timescale 1000, duration 2000
        let mut mvhd = vec![0u8; 4];
        for v in [MP4_EPOCH_OFFSET as u32 + 1_700_000_000, 0, 1000, 2000] {
            mvhd.extend_from_slice(&v.to_be_bytes());
        }
        let mdhd = |timescale: u32, duration: u32| {
            let mut v = vec![0u8; 12];
            v.extend_from_slice(&timescale.to_be_bytes());
            v.extend_from_slice(&duration.to_be_bytes());
            bx(b"mdhd", &v)
        };
        let hdlr = |kind: &[u8; 4]| {
            let mut v = vec![0u8; 8];
            v.extend_from_slice(kind);
            v.extend_from_slice(&[0; 12]);
            bx(b"hdlr", &v)
        };
        let trak = |handler: &[u8; 4], timescale, duration, entry: Vec<u8>, stts: Vec<u8>| {
            let mut stsd = vec![0, 0, 0, 0, 0, 0, 0, 1];
            stsd.extend(entry);
            let stbl = bx(b"stbl", &[bx(b"stsd", &stsd), stts].concat());
            let minf = bx(b"minf", &stbl);
            let mdia = bx(
                b"mdia",
                &[mdhd(timescale, duration), hdlr(handler), minf].concat(),
            );
            bx(b"trak", &mdia)
        };

        // avc1: 8 byte header, 16 reserved, 1920x1080, rest of the 78 bytes, then avcC
        let mut avc1 = vec![0u8; 24];
        avc1.extend_from_slice(&1920u16.to_be_bytes());
        avc1.extend_from_slice(&1080u16.to_be_bytes());
        avc1.resize(78, 0);
        avc1.extend(bx(b"avcC", &[1, 100, 0, 40]));
        // 60 frames over 2 seconds
        let stts = bx(b"stts", &[0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 60, 0, 0, 0, 50]);
        let video = trak(b"vide", 1500, 3000, bx(b"avc1", &avc1), stts);

        // mp4a: version 0, 2 channels, 48 kHz, then esds with AAC-LC
        let mut mp4a = vec![0u8; 16];
        mp4a.extend_from_slice(&2u16.to_be_bytes());
        mp4a.extend_from_slice(&[0; 6]);
        mp4a.extend_from_slice(&(48_000u32 << 16).to_be_bytes());
        let esds = [
            &[0, 0, 0, 0][..],
            &[0x03, 22, 0, 1, 0],
            &[0x04, 17, 0x40, 0x15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            &[0x05, 2, 0x11, 0x90],
        ]
        .concat();
        mp4a.extend(bx(b"esds", &esds));
        let audio = trak(b"soun", 48_000, 96_000, bx(b"mp4a", &mp4a), vec![]);

        let file = [
            bx(b"ftyp", b"isom\0\0\0\0"),
            bx(b"mdat", &[0; 32]),
            bx(b"moov", &[bx(b"mvhd", &mvhd), video, audio].concat()),
        ]
        .concat();

        let streams = probe(&mut std::io::Cursor::new(file)).unwrap();
        assert_eq!(streams.brand.as_deref(), Some("isom"));
        assert_eq!(streams.duration_secs, Some(2.0));
        assert_eq!(streams.created.as_deref(), Some("2023-11-14T22:13:20Z"));
        assert_eq!(streams.tracks.len(), 2);
        assert_eq!(streams.tracks[0].kind, TrackKind::Video);
        assert_eq!(streams.tracks[0].codec, "H.264 / AVC (High, level 4.0)");
        assert_eq!(streams.tracks[0].details, ["1920 × 1080", "30.00 fps"]);
        assert_eq!(streams.tracks[1].kind, TrackKind::Audio);
        assert_eq!(streams.tracks[1].codec, "AAC-LC");
        assert_eq!(streams.tracks[1].details, ["stereo", "48.0 kHz"]);
    }
}
