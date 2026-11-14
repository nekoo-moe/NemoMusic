//! Lyrics extras that travel beside a [`LyricsDoc`]: per-line facts
//! such as the singer, background vocals and the provider's romanization, and the user's own
//! lyrics.
//!
//! `LyricsDoc` and `LyricLine` are built as struct literals in many crates, so this data is
//! kept apart from them and indexed by line number (lyrics-plus design).

use crate::lyrics::{Granularity, LyricWord, LyricsDoc, ProviderId};

/// The provider id of lyrics the user wrote or synced. They win over every other provider.
pub const OWN_LYRICS: ProviderId = ProviderId::new("Your lyrics");

/// Side data of one line, same index as `LyricsDoc.lines`.
///
/// The default says nothing: no known singer, main vocals, no romanization.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineMeta {
    /// Which singer sings the line, numbered from 0 in the order the source declares them.
    /// `None` when the source names no singer.
    pub singer: Option<u8>,
    /// The line is background vocals.
    pub background: bool,
    /// The provider's own romanization of the line, if it ships one.
    pub roman: Option<String>,
}

/// One line of own lyrics being written or synced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DraftLine {
    /// The line text.
    pub text: String,
    /// When the line starts, in ms from the start of the song. `None` until it is synced.
    pub start_ms: Option<u64>,
    /// Word timings, in order; empty for a line-synced line.
    pub words: Vec<LyricWord>,
}

/// Own lyrics before they are saved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OwnLyricsDraft {
    /// The lines, in order.
    pub lines: Vec<DraftLine>,
}

/// Saving or deleting own lyrics failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OwnLyricsError {
    /// A line has no start time. Times are never spread evenly; the user syncs every line.
    #[error("line {line} has no start time")]
    MissingStart {
        /// The first line without a start, counted from 1.
        line: usize,
    },
    /// The draft has no lines.
    #[error("no lyrics")]
    Empty,
    /// The words of a line are not in time order.
    #[error("words of line {line} are out of order")]
    WordOrder {
        /// The line, counted from 1.
        line: usize,
    },
    /// The local store failed.
    #[error("store: {0}")]
    Store(String),
}

/// Lyrics the user wrote or synced for a track, kept on this computer.
pub trait OwnLyrics: Send + Sync {
    /// The own lyrics of a track, with provider [`OWN_LYRICS`], if any.
    fn get(&self, track_key: &str) -> Option<LyricsDoc>;

    /// Save a draft. Returns the granularity stored: `Word` when every line has words,
    /// otherwise `Line`.
    ///
    /// # Errors
    /// [`OwnLyricsError::Empty`], [`OwnLyricsError::MissingStart`] or
    /// [`OwnLyricsError::WordOrder`] when the draft is not complete, and
    /// [`OwnLyricsError::Store`] when it cannot be stored.
    fn save(&self, track_key: &str, draft: &OwnLyricsDraft) -> Result<Granularity, OwnLyricsError>;

    /// Keep a copy of lyrics from a provider as the track's own lyrics, to edit or keep.
    ///
    /// # Errors
    /// [`OwnLyricsError::Store`] when it cannot be stored.
    fn save_copy(&self, track_key: &str, doc: &LyricsDoc) -> Result<(), OwnLyricsError>;

    /// Delete the own lyrics of a track.
    ///
    /// # Errors
    /// [`OwnLyricsError::Store`] when they cannot be deleted.
    fn delete(&self, track_key: &str) -> Result<(), OwnLyricsError>;
}

#[cfg(test)]
mod tests {
    use super::{LineMeta, OWN_LYRICS, OwnLyricsDraft, OwnLyricsError};
    use crate::BoxFuture;
    use crate::lyrics::{LyricsQuery, ProviderId};
    use crate::services::{LyricsLookup, LyricsService};

    #[test]
    fn default_line_meta_says_nothing() {
        let meta = LineMeta::default();
        assert_eq!(meta.singer, None);
        assert!(!meta.background);
        assert_eq!(meta.roman, None);
    }

    struct Bare;

    impl LyricsService for Bare {
        fn lookup<'a>(&'a self, _query: &'a LyricsQuery) -> BoxFuture<'a, LyricsLookup> {
            Box::pin(async {
                LyricsLookup {
                    doc: None,
                    tried: Vec::new(),
                }
            })
        }
        fn offset_ms(&self, _track_key: &str) -> i64 {
            0
        }
        fn set_offset_ms(&self, _track_key: &str, _offset_ms: i64) {}
    }

    #[test]
    fn line_meta_defaults_to_empty() {
        assert!(Bare.line_meta("key", ProviderId::new("LRCLIB")).is_empty());
    }

    #[test]
    fn own_lyrics_errors_name_the_line() {
        assert_eq!(
            OwnLyricsError::MissingStart { line: 3 }.to_string(),
            "line 3 has no start time"
        );
        assert_eq!(
            OwnLyricsError::WordOrder { line: 2 }.to_string(),
            "words of line 2 are out of order"
        );
        assert_eq!(OwnLyricsError::Empty.to_string(), "no lyrics");
        assert_eq!(OWN_LYRICS, ProviderId::new("Your lyrics"));
        assert!(OwnLyricsDraft::default().lines.is_empty());
    }
}
