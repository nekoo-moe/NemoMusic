//! The play log store (a stub for now) and the play-count rule.

use std::time::Duration;

use nemo_core::hooks::PlayRule;

/// The play log could not be opened or written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// The play log is not available yet.
    #[error("the play log is not available")]
    Unavailable,
}

/// The play log. A stub: the SQLite file, the writer thread and the queries come in later
/// tasks.
#[derive(Debug, Default)]
pub struct StatsStore {
    _private: (),
}

/// Whether a play of a track of this length, heard for `heard`, counts in the stats.
///
/// This is `nemo_core::hooks::PlayRule::counts`, so the stats and the scrobbler agree.
#[must_use]
pub fn counts_as_play(duration: Option<Duration>, heard: Duration) -> bool {
    PlayRule::counts(duration, heard)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use nemo_core::hooks::PlayRule;

    use super::counts_as_play;

    const fn s(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    #[test]
    fn counts_as_play_follows_the_integrations_table() {
        // A track of 30 s or less never counts, however long it is heard.
        assert!(!counts_as_play(Some(s(29)), s(29)));
        assert!(!counts_as_play(Some(s(30)), s(30)));
        // 31 s counts at half its length (15.5 s).
        assert!(!counts_as_play(Some(s(31)), s(15)));
        assert!(counts_as_play(Some(s(31)), Duration::from_millis(15_500)));
        // 8 min counts at 4 min, not at 4 min minus 1 s.
        assert!(!counts_as_play(Some(s(480)), s(239)));
        assert!(counts_as_play(Some(s(480)), s(240)));
        // An unknown length counts at 240 s.
        assert!(!counts_as_play(None, s(239)));
        assert!(counts_as_play(None, s(240)));
    }

    #[test]
    fn counts_as_play_agrees_with_play_rule() {
        for duration in [
            None,
            Some(s(0)),
            Some(s(30)),
            Some(s(31)),
            Some(s(200)),
            Some(s(900)),
        ] {
            for heard in [0, 1, 15, 16, 100, 239, 240, 241, 1000] {
                assert_eq!(
                    counts_as_play(duration, s(heard)),
                    PlayRule::counts(duration, s(heard)),
                );
            }
        }
    }
}
