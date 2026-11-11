//! Read title, artists, album and duration from a file's tags with `lofty`.
//!
//! Tags are untrusted text from the file: they are trimmed and length-capped, never executed or
//! used as paths.

use std::path::Path;
use std::time::Duration;

use lofty::prelude::{Accessor, AudioFile, TaggedFileExt};

use nemo_core::track::Track;

/// Longest tag text kept, in characters.
pub const MAX_TAG_CHARS: usize = 512;

/// What the tags of one file say.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TagInfo {
    /// Track title.
    pub title: Option<String>,
    /// Artist names, split on `;` and `/`.
    pub artists: Vec<String>,
    /// Album title.
    pub album: Option<String>,
    /// Duration from the stream properties.
    pub duration: Option<Duration>,
}

/// Read the tags of one file. Returns `None` when the file cannot be parsed.
#[must_use]
pub fn read_tags(path: &Path) -> Option<TagInfo> {
    let file = lofty::read_from_path(path).ok()?;
    let duration = Some(file.properties().duration()).filter(|d| !d.is_zero());
    let tag = file.primary_tag().or_else(|| file.first_tag());
    let mut info = TagInfo {
        duration,
        ..TagInfo::default()
    };
    if let Some(tag) = tag {
        info.title = tag.title().and_then(|t| clean(&t));
        info.album = tag.album().and_then(|a| clean(&a));
        info.artists = tag.artist().map(|a| split_artists(&a)).unwrap_or_default();
    }
    Some(info)
}

/// Put tag facts into a track. Missing facts keep what the track already has.
pub fn apply_tags(track: &mut Track, info: TagInfo) {
    if let Some(title) = info.title {
        track.title = title;
    }
    if !info.artists.is_empty() {
        track.artists = info.artists;
    }
    if info.album.is_some() {
        track.album = info.album;
    }
    if info.duration.is_some() {
        track.duration = info.duration;
    }
}

/// Trim, drop control characters, cap the length. Empty text gives `None`.
#[must_use]
pub fn clean(text: &str) -> Option<String> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_TAG_CHARS)
        .collect();
    let trimmed = cleaned.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// Split an artist tag such as `A; B / C` into names.
#[must_use]
pub fn split_artists(text: &str) -> Vec<String> {
    text.split([';', '/']).filter_map(clean).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::track_from_path;

    #[test]
    fn clean_trims_caps_and_drops_control_characters() {
        assert_eq!(clean("  Song\u{0}\n "), Some("Song".to_owned()));
        assert_eq!(clean("   "), None);
        let long = "a".repeat(MAX_TAG_CHARS + 10);
        assert_eq!(clean(&long).map(|s| s.chars().count()), Some(MAX_TAG_CHARS));
    }

    #[test]
    fn artists_split_on_semicolon_and_slash() {
        assert_eq!(split_artists("A; B / C"), ["A", "B", "C"]);
        assert_eq!(split_artists(" ; "), Vec::<String>::new());
    }

    #[test]
    fn apply_keeps_existing_values_when_a_tag_is_missing() {
        let mut track = track_from_path("C:/music/01 Song.flac".into());
        apply_tags(
            &mut track,
            TagInfo {
                artists: vec!["Artist".into()],
                ..TagInfo::default()
            },
        );
        assert_eq!(track.title, "01 Song");
        assert_eq!(track.artists, ["Artist"]);
        assert!(track.album.is_none());
    }

    #[test]
    fn an_unparsable_file_gives_none() {
        let path = std::env::temp_dir().join(format!("nemo-tags-{}.flac", std::process::id()));
        std::fs::write(&path, b"not audio").expect("write");
        assert_eq!(read_tags(&path), None);
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn a_plain_wav_has_a_duration_and_no_tags() {
        let path = std::env::temp_dir().join(format!("nemo-tags-{}.wav", std::process::id()));
        std::fs::write(&path, wav_one_second()).expect("write");
        let info = read_tags(&path).expect("parsed");
        assert_eq!(info.title, None);
        assert_eq!(info.duration.map(|d| d.as_millis()), Some(1000));
        std::fs::remove_file(path).expect("cleanup");
    }

    /// One second of 8 kHz mono 16-bit silence.
    fn wav_one_second() -> Vec<u8> {
        let data_len: u32 = 8000 * 2;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data_len).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16_u32.to_le_bytes());
        v.extend_from_slice(&1_u16.to_le_bytes()); // PCM
        v.extend_from_slice(&1_u16.to_le_bytes()); // mono
        v.extend_from_slice(&8000_u32.to_le_bytes());
        v.extend_from_slice(&16000_u32.to_le_bytes());
        v.extend_from_slice(&2_u16.to_le_bytes());
        v.extend_from_slice(&16_u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_len.to_le_bytes());
        v.resize(v.len() + data_len as usize, 0);
        v
    }
}
