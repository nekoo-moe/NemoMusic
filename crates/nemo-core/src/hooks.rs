//! Player-event hooks shared by the services that watch playback (integrations, listening-stats).
//!
//! Pure, no I/O. [`PlayRule`] decides whether a play counts, the same way for Last.fm scrobbles
//! and for local listening stats, so both agree.

use std::time::Duration;

/// When a play counts: Last.fm's rule, also used for local listening stats.
///
/// A track longer than [`MIN_TRACK`](Self::MIN_TRACK) counts once it has been heard for half its
/// length or [`MAX_THRESHOLD`](Self::MAX_THRESHOLD), whichever comes first. A track of unknown
/// length counts at `MAX_THRESHOLD`. Paused time is not heard time; the caller measures `heard`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayRule;

impl PlayRule {
    /// 30 s: a track must be longer than this to count at all.
    pub const MIN_TRACK: Duration = Duration::from_secs(30);
    /// 4 min: a play never needs more heard time than this.
    pub const MAX_THRESHOLD: Duration = Duration::from_mins(4);

    /// Heard time after which a track of this length counts: `min(duration / 2, 4 min)`, or
    /// 4 min when the length is unknown. `None` when the track is 30 s or shorter, which never
    /// counts.
    #[must_use]
    pub fn threshold(duration: Option<Duration>) -> Option<Duration> {
        match duration {
            None => Some(Self::MAX_THRESHOLD),
            Some(d) if d <= Self::MIN_TRACK => None,
            Some(d) => Some((d / 2).min(Self::MAX_THRESHOLD)),
        }
    }

    /// Whether a play of a track of this length that was heard for `heard` counts.
    #[must_use]
    pub fn counts(duration: Option<Duration>, heard: Duration) -> bool {
        Self::threshold(duration).is_some_and(|t| heard >= t)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::PlayRule;

    const fn s(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    #[test]
    fn threshold_table() {
        let cases = [
            (Some(s(29)), None),
            (Some(s(30)), None),
            (Some(s(31)), Some(Duration::from_millis(15_500))),
            (Some(s(8 * 60)), Some(s(240))),
            (Some(s(10 * 60)), Some(s(240))),
            (None, Some(s(240))),
        ];
        for (duration, want) in cases {
            assert_eq!(PlayRule::threshold(duration), want, "{duration:?}");
        }
    }

    #[test]
    fn counts_table() {
        let cases = [
            // 29 s and 30 s tracks never count, even heard in full.
            (Some(s(29)), s(29), false),
            (Some(s(30)), s(30), false),
            // 31 s: half is 15.5 s.
            (Some(s(31)), Duration::from_millis(15_499), false),
            (Some(s(31)), Duration::from_millis(15_500), true),
            // 8 min: half is 4 min, which is also the cap.
            (Some(s(8 * 60)), s(239), false),
            (Some(s(8 * 60)), s(240), true),
            // 20 min: the 4 min cap comes first.
            (Some(s(20 * 60)), s(240), true),
            // Unknown length: 4 min.
            (None, s(239), false),
            (None, s(240), true),
        ];
        for (duration, heard, want) in cases {
            assert_eq!(
                PlayRule::counts(duration, heard),
                want,
                "{duration:?} heard {heard:?}"
            );
        }
    }
}
