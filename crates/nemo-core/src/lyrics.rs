//! Lyrics types and the provider trait (ADR 0002).
//!
//! Providers live in `nemo-lyrics` (open sources) and `nemo-ytm` (YouTube Music timed lyrics).
//! Lyrics are copyrighted text: show them and cache them locally, never export them.

use std::path::PathBuf;
use std::time::Duration;

use crate::BoxFuture;
use crate::track::{Track, VideoId};

/// Stable name of a provider, shown in the UI ("Lyrics: LRCLIB").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderId(&'static str);

impl ProviderId {
    /// Create an id from a static name.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// The display name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// How precisely lyrics are timed. A higher value is better, so `max()` picks the best.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Granularity {
    /// Plain text. No timing. Never fake timing for it.
    Unsynced,
    /// One timestamp per line.
    Line,
    /// One timestamp per word.
    Word,
}

/// What to look up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsQuery {
    /// Track title.
    pub title: String,
    /// Artist names, primary first.
    pub artists: Vec<String>,
    /// Album title, if known.
    pub album: Option<String>,
    /// Track duration in milliseconds, if known. Used to reject wrong matches.
    pub duration_ms: Option<u64>,
    /// YouTube video id, if the track comes from YouTube Music.
    pub ytm_video_id: Option<VideoId>,
    /// File path, if the track is a local file (for sidecar `.lrc` and embedded tags).
    pub local_path: Option<PathBuf>,
}

impl LyricsQuery {
    /// Build a query from a track.
    #[must_use]
    pub fn from_track(track: &Track) -> Self {
        Self {
            title: track.title.clone(),
            artists: track.artists.clone(),
            album: track.album.clone(),
            duration_ms: track
                .duration
                .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
            ytm_video_id: track.video_id.clone(),
            local_path: track.local_path.clone(),
        }
    }
}

/// One timed word inside a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricWord {
    /// Start time in milliseconds.
    pub start_ms: u64,
    /// End time in milliseconds, if the source gives it.
    pub end_ms: Option<u64>,
    /// The word text.
    pub text: String,
}

/// One line of lyrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricLine {
    /// Start time in milliseconds. `None` for unsynced lyrics.
    pub start_ms: Option<u64>,
    /// End time in milliseconds, if the source gives it.
    pub end_ms: Option<u64>,
    /// The line text.
    pub text: String,
    /// Timed words. Empty unless the source has word timing.
    pub words: Vec<LyricWord>,
}

/// The track a provider says it matched. Shown when the match is weak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedMeta {
    /// Title the provider returned.
    pub title: String,
    /// Artists the provider returned.
    pub artists: Vec<String>,
    /// Album the provider returned.
    pub album: Option<String>,
    /// Duration the provider returned, in milliseconds.
    pub duration_ms: Option<u64>,
}

/// Lyrics for one track from one provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsDoc {
    /// The lines, in time order for synced lyrics.
    pub lines: Vec<LyricLine>,
    /// Timing precision.
    pub granularity: Granularity,
    /// Which provider produced this.
    pub provider: ProviderId,
    /// What the provider matched, if it says.
    pub matched: Option<MatchedMeta>,
    /// The track has no vocals. The UI shows "Instrumental", not an empty panel.
    pub instrumental: bool,
}

/// A lyrics lookup failed. A missing track is not an error; it is `Ok(None)`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LyricsError {
    /// The provider asked us to slow down.
    #[error("rate limited by provider")]
    RateLimited {
        /// How long to wait, if the provider says.
        retry_after: Option<Duration>,
    },
    /// The network request failed.
    #[error("network error: {0}")]
    Network(String),
    /// The provider's answer could not be parsed.
    #[error("could not parse provider answer: {0}")]
    Parse(String),
    /// The provider is switched off or unreachable.
    #[error("provider unavailable: {0}")]
    Unavailable(String),
}

/// A source of lyrics.
pub trait LyricsProvider: Send + Sync {
    /// Stable name, shown in the UI.
    fn id(&self) -> ProviderId;

    /// Hosts this provider contacts. Each must be on the README egress list.
    fn hosts(&self) -> &'static [&'static str];

    /// Look up lyrics. `Ok(None)` means the provider has none for this track.
    fn fetch<'a>(
        &'a self,
        query: &'a LyricsQuery,
    ) -> BoxFuture<'a, Result<Option<LyricsDoc>, LyricsError>>;
}

#[cfg(test)]
mod tests {
    use super::Granularity;

    #[test]
    fn word_is_better_than_line_is_better_than_unsynced() {
        assert!(Granularity::Word > Granularity::Line);
        assert!(Granularity::Line > Granularity::Unsynced);
        assert_eq!(
            [Granularity::Line, Granularity::Word, Granularity::Unsynced]
                .into_iter()
                .max(),
            Some(Granularity::Word)
        );
    }
}
