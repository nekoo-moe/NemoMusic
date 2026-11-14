//! Lyrics extras that travel beside a [`LyricsDoc`](crate::lyrics::LyricsDoc): per-line facts
//! such as the singer, background vocals and the provider's romanization.
//!
//! `LyricsDoc` and `LyricLine` are built as struct literals in many crates, so this data is
//! kept apart from them and indexed by line number (lyrics-plus design).

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

#[cfg(test)]
mod tests {
    use super::LineMeta;
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
}
