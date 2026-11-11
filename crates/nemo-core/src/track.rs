//! Track, artwork and identifier types.

use std::path::PathBuf;
use std::time::Duration;

use crate::audio::Origin;

/// Stable identifier of a track inside `NemoMusic`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrackId(pub String);

/// A YouTube video id: exactly 11 characters from `A-Z a-z 0-9 - _`.
///
/// Validated at construction because ids reach a subprocess command line and URLs. An id may
/// start with `-`, so callers must pass it after `--` or inside a full URL, never as a bare
/// argument.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoId(String);

/// The text is not a valid YouTube video id. The offending text is not echoed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid YouTube video id")]
pub struct InvalidVideoId;

impl VideoId {
    /// Parse and validate a video id.
    ///
    /// # Errors
    /// Returns [`InvalidVideoId`] unless the text is 11 ASCII letters, digits, `-` or `_`.
    pub fn parse(text: &str) -> Result<Self, InvalidVideoId> {
        let valid = text.len() == 11
            && text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(InvalidVideoId)
        }
    }

    /// The id as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One size of a cover image on a remote server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnail {
    /// Image address.
    pub url: String,
    /// Width in pixels. `0` means unknown.
    pub width: u32,
    /// Height in pixels. `0` means unknown.
    pub height: u32,
}

/// Where a track's cover art comes from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Artwork {
    /// No cover is known.
    #[default]
    None,
    /// Several sizes of one remote image.
    Remote(Vec<Thumbnail>),
    /// An image file, or an audio file whose tags hold the cover.
    LocalFile(PathBuf),
}

/// Choose the thumbnail for a slot of `target_px` pixels without upscaling (`design.md`,
/// Album art): the smallest one that is at least `target_px` wide, else the largest available.
/// Thumbnails of unknown width (`0`) are used only when no width is known at all.
#[must_use]
pub fn pick_thumbnail(thumbnails: &[Thumbnail], target_px: u32) -> Option<&Thumbnail> {
    thumbnails
        .iter()
        .filter(|t| t.width >= target_px && t.width > 0)
        .min_by_key(|t| t.width)
        .or_else(|| thumbnails.iter().max_by_key(|t| t.width))
}

/// A playable track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    /// Stable identifier.
    pub id: TrackId,
    /// Track title as the source gives it.
    pub title: String,
    /// Artist names, primary artist first.
    pub artists: Vec<String>,
    /// Album title, if any.
    pub album: Option<String>,
    /// Duration, if known.
    pub duration: Option<Duration>,
    /// Where the audio comes from.
    pub origin: Origin,
    /// YouTube video id for YouTube Music tracks.
    pub video_id: Option<VideoId>,
    /// File path for local tracks.
    pub local_path: Option<PathBuf>,
    /// Cover art.
    pub artwork: Artwork,
}

#[cfg(test)]
mod tests {
    use super::{Thumbnail, VideoId, pick_thumbnail};

    #[test]
    fn accepts_a_real_shaped_id() {
        assert_eq!(
            VideoId::parse("dQw4w9WgXcQ")
                .map(|v| v.as_str().to_owned())
                .ok()
                .as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert!(VideoId::parse("-_-_-_-_-_-").is_ok());
    }

    #[test]
    fn rejects_wrong_length_and_bad_characters() {
        for bad in [
            "",
            "short",
            "dQw4w9WgXcQx",
            "dQw4w9WgXc ",
            "dQw4w9WgXc;",
            "dQw4w9WgXc\n",
            "dQw4w9WgXcé",
        ] {
            assert!(VideoId::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn error_does_not_echo_the_input() {
        let err = VideoId::parse("secret-token-here").unwrap_err();
        assert!(!err.to_string().contains("secret"));
    }

    fn thumb(width: u32) -> Thumbnail {
        Thumbnail {
            url: format!("https://example.invalid/{width}"),
            width,
            height: width,
        }
    }

    #[test]
    fn thumbnail_is_the_smallest_that_is_big_enough() {
        let all = [thumb(60), thumb(226), thumb(544), thumb(120)];
        assert_eq!(pick_thumbnail(&all, 100).map(|t| t.width), Some(120));
        assert_eq!(pick_thumbnail(&all, 226).map(|t| t.width), Some(226));
        assert_eq!(pick_thumbnail(&all, 300).map(|t| t.width), Some(544));
    }

    #[test]
    fn thumbnail_falls_back_to_the_largest_instead_of_upscaling_a_tiny_one() {
        let all = [thumb(60), thumb(120)];
        assert_eq!(pick_thumbnail(&all, 640).map(|t| t.width), Some(120));
        assert!(pick_thumbnail(&[], 100).is_none());
    }

    #[test]
    fn unknown_widths_are_used_only_as_a_last_resort() {
        let all = [thumb(0), thumb(200)];
        assert_eq!(pick_thumbnail(&all, 100).map(|t| t.width), Some(200));
        assert_eq!(pick_thumbnail(&[thumb(0)], 100).map(|t| t.width), Some(0));
    }
}
