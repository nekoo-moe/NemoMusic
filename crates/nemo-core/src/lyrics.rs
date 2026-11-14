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
    /// One timestamp per syllable: a word may be split into several timed pieces. The pieces
    /// are stored as [`LyricWord`]s whose `text` keeps its own whitespace, so pieces of one word
    /// have no space between them. The best level; pickers prefer it over [`Self::Word`].
    Syllable,
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

/// Credit a source asks apps to show next to its lyrics (for example Unison and the submitter).
/// The UI shows `label` (linked to `url` when present) and, when present, `by` (linked to
/// `by_url`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsCredit {
    /// Text such as "Lyrics from Unison".
    pub label: String,
    /// Page of the source for this lyric.
    pub url: Option<String>,
    /// Who submitted the lyric, if the source says.
    pub by: Option<String>,
    /// Profile page of the submitter.
    pub by_url: Option<String>,
    /// Extra detail for the picker, for example community confidence: low.
    pub note: Option<String>,
}

/// Static facts about a provider, for the settings screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderInfo {
    /// Whether the provider is on until the user turns it off. Grey-area sources are `false`.
    pub default_enabled: bool,
    /// One-line risk note shown next to an opt-in provider. `None` for open sources.
    pub risk_note: Option<&'static str>,
}

impl ProviderInfo {
    /// An open source that is on by default.
    pub const OPEN: Self = Self {
        default_enabled: true,
        risk_note: None,
    };

    /// A grey-area source: off until the user opts in, with a risk note.
    #[must_use]
    pub const fn opt_in(risk_note: &'static str) -> Self {
        Self {
            default_enabled: false,
            risk_note: Some(risk_note),
        }
    }
}

/// A lyrics document with the credit its source asks for.
pub type CreditedDoc = (LyricsDoc, Option<LyricsCredit>);

/// One provider's answer for a track, for the "rotate providers" picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsCandidate {
    /// Which provider produced it (also `doc.provider`).
    pub provider: ProviderId,
    /// Timing precision (also `doc.granularity`).
    pub granularity: Granularity,
    /// Match score, 0 to 100.
    pub score: u8,
    /// Short note shown with the provider name, for example "community confidence: low" or a
    /// risk note for an opt-in source. `None` when there is nothing to add.
    pub source_note: Option<String>,
    /// Credit the source asks for, if any.
    pub credit: Option<LyricsCredit>,
    /// The lyrics.
    pub doc: LyricsDoc,
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

    /// Like [`fetch`](Self::fetch), plus the credit the source asks for. The default has none.
    fn fetch_credited<'a>(
        &'a self,
        query: &'a LyricsQuery,
    ) -> BoxFuture<'a, Result<Option<CreditedDoc>, LyricsError>> {
        Box::pin(async move { Ok(self.fetch(query).await?.map(|doc| (doc, None))) })
    }

    /// Default on or off, and the risk note. The default is an open source that is on.
    fn info(&self) -> ProviderInfo {
        ProviderInfo::OPEN
    }

    /// The finest timing this provider can give for some tracks. The default is line timing.
    fn best_granularity(&self) -> Granularity {
        Granularity::Line
    }

    /// Longest the lyrics chain waits for this provider, including the wait for its host. The
    /// chain's global budget still caps it. The default is 4 s (ADR 0002); a provider that runs
    /// a slow helper declares more (YouTube captions: 20 s).
    fn timeout(&self) -> Duration {
        Duration::from_secs(4)
    }
}

#[cfg(test)]
mod tests {
    use super::Granularity;

    #[test]
    fn word_is_better_than_line_is_better_than_unsynced() {
        assert!(Granularity::Syllable > Granularity::Word);
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
