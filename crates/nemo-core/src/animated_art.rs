//! The contract of an animated-cover source: a short silent clip shown in place of the still
//! cover on Now Playing.
//!
//! No I/O here. `nemo-spotify` implements [`AnimatedArtSource`] for Spotify Canvas; the UI asks
//! each enabled source through it and never knows a source by name. There is no Apple Music
//! source (now-playing-styles criterion 22, Later).

use std::path::PathBuf;

use crate::BoxFuture;

/// Names one animated-art source, for example `"spotify-canvas"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArtSourceId(&'static str);

impl ArtSourceId {
    /// A source id from a fixed name.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// The id as text.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// The song an animated cover is looked up for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtQuery {
    /// The song title.
    pub title: String,
    /// Artist names, primary artist first.
    pub artists: Vec<String>,
    /// The album title, if known.
    pub album: Option<String>,
    /// The song length in ms, if known.
    pub duration_ms: Option<u64>,
}

/// An animated cover downloaded into the app's private cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimatedArt {
    /// The source it came from.
    pub source: ArtSourceId,
    /// The clip file in the private cache.
    pub clip: PathBuf,
    /// Clip width in pixels.
    pub width: u32,
    /// Clip height in pixels.
    pub height: u32,
    /// The credit shown with it, for example "Animated cover: Spotify".
    pub label: &'static str,
}

/// The answer of a lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtLookup {
    /// The song has an animated cover.
    Found(AnimatedArt),
    /// The song has none (kept as a miss, so it is not asked again soon).
    NoneAvailable,
}

/// What the settings entry of a source says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArtSourceInfo {
    /// The entry title.
    pub label: &'static str,
    /// What leaves the computer, in plain text.
    pub sends: &'static str,
    /// The risk, in plain text.
    pub risk: &'static str,
}

/// An animated-cover lookup failed; the still cover stays.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnimatedArtError {
    /// The source needs a sign-in that is missing.
    #[error("signed out")]
    SignedOut,
    /// The network cannot be reached.
    #[error("offline")]
    Offline,
    /// The source refused the request with this HTTP status.
    #[error("refused with status {0}")]
    Refused(u16),
    /// The clip's video format cannot be played.
    #[error("video not supported")]
    Unsupported,
    /// The clip is over the size cap.
    #[error("clip too large")]
    TooLarge,
    /// The clip cache failed.
    #[error("cache error: {0}")]
    Store(String),
}

/// A source of animated covers.
pub trait AnimatedArtSource: Send + Sync {
    /// The source id.
    fn id(&self) -> ArtSourceId;

    /// The exact hosts the source contacts, for the settings entry and the README egress list.
    fn hosts(&self) -> &'static [&'static str];

    /// What the settings entry says.
    fn info(&self) -> ArtSourceInfo;

    /// Find and download the animated cover of a song.
    ///
    /// # Errors
    /// An [`AnimatedArtError`] when the source needs a sign-in, cannot be reached, refuses, or
    /// the clip is unsupported or too large.
    fn find<'a>(
        &'a self,
        query: &'a ArtQuery,
    ) -> BoxFuture<'a, Result<ArtLookup, AnimatedArtError>>;

    /// Delete every cached clip and miss of this source.
    fn clear_cache(&self);
}

#[cfg(test)]
mod tests {
    use super::{AnimatedArtError, ArtSourceId};

    #[test]
    fn source_ids_compare_by_name() {
        const CANVAS: ArtSourceId = ArtSourceId::new("spotify-canvas");
        assert_eq!(CANVAS.as_str(), "spotify-canvas");
        assert_eq!(CANVAS, ArtSourceId::new("spotify-canvas"));
        assert_ne!(CANVAS, ArtSourceId::new("other"));
    }

    #[test]
    fn errors_are_one_short_line() {
        assert_eq!(AnimatedArtError::SignedOut.to_string(), "signed out");
        assert_eq!(
            AnimatedArtError::Refused(403).to_string(),
            "refused with status 403"
        );
        assert_eq!(
            AnimatedArtError::Unsupported.to_string(),
            "video not supported"
        );
    }
}
