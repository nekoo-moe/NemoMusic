//! Match scoring (ADR 0002): how well does a provider's track match the one we asked for.
//!
//! Points: title 40, primary artist 30, album 10, duration 20.
//! - Album is ignored when either side lacks it: the maximum shrinks to 90 and the total is
//!   rescaled to 0..=100.
//! - Duration: full points within 1 s, linear down to 0 at 3 s. When both durations are known and
//!   differ by more than 3 s the candidate is rejected. If either is unknown the duration earns
//!   0 points (no rescale), so an unknown duration can never help a match.
//! - Accept at total >= 70 and title points > 0.

use nemo_core::lyrics::{LyricsDoc, LyricsQuery, MatchedMeta};

use crate::normalize::normalize;

/// Lowest accepted total score.
pub const ACCEPT_THRESHOLD: u8 = 70;
/// Largest duration difference (ms) that is not rejected outright.
pub const MAX_DURATION_DIFF_MS: u64 = 3000;
/// Duration difference (ms) up to which the duration earns full points.
pub const FULL_DURATION_DIFF_MS: u64 = 1000;
/// How far past the track duration (ms) a synced doc's last timestamp may be.
pub const WRONG_MATCH_SLACK_MS: u64 = 5000;

const TITLE_MAX: u32 = 40;
const ARTIST_MAX: u32 = 30;
const ALBUM_MAX: u32 = 10;
const DURATION_MAX: u32 = 20;

/// Score of one candidate, with the parts that make it up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchScore {
    /// Total, 0 to 100.
    pub total: u8,
    /// Title points, 0 to 40.
    pub title: u8,
    /// Primary artist points, 0 to 30.
    pub artist: u8,
    /// Album points, 0 to 10 (0 when ignored).
    pub album: u8,
    /// Duration points, 0 to 20.
    pub duration: u8,
}

impl MatchScore {
    /// True if `total >= 70` and the title earned points.
    #[must_use]
    pub const fn is_acceptable(&self) -> bool {
        self.total >= ACCEPT_THRESHOLD && self.title > 0
    }
}

/// Similarity of two strings in permille (0 to 1000).
///
/// 1000 if equal after normalization, otherwise the Dice coefficient of the distinct word sets.
/// 0 when either side normalizes to nothing.
fn similarity(a: &str, b: &str) -> u32 {
    let (na, nb) = (normalize(a), normalize(b));
    if na.is_empty() || nb.is_empty() {
        return 0;
    }
    if na == nb {
        return 1000;
    }
    let mut ta: Vec<&str> = na.split(' ').collect();
    let mut tb: Vec<&str> = nb.split(' ').collect();
    ta.sort_unstable();
    ta.dedup();
    tb.sort_unstable();
    tb.dedup();
    let common = ta.iter().filter(|t| tb.binary_search(t).is_ok()).count();
    let total = ta.len() + tb.len();
    // Word counts of a title are tiny, so these casts cannot truncate in practice.
    u32::try_from(2000 * common / total)
        .unwrap_or(1000)
        .min(1000)
}

/// `max * sim / 1000`, rounded.
fn points(max: u32, sim: u32) -> u32 {
    (max * sim + 500) / 1000
}

fn to_u8(v: u32) -> u8 {
    u8::try_from(v).unwrap_or(u8::MAX)
}

/// Score `candidate` against `query`.
///
/// Returns `None` when both durations are known and differ by more than 3 s (hard reject).
/// Otherwise returns the score; check [`MatchScore::is_acceptable`] for the 70 / title rule.
#[must_use]
pub fn score_match(query: &LyricsQuery, candidate: &MatchedMeta) -> Option<MatchScore> {
    let duration = match (query.duration_ms, candidate.duration_ms) {
        (Some(q), Some(c)) => {
            let diff = q.abs_diff(c);
            if diff > MAX_DURATION_DIFF_MS {
                return None;
            }
            if diff <= FULL_DURATION_DIFF_MS {
                DURATION_MAX
            } else {
                let left = u32::try_from(MAX_DURATION_DIFF_MS - diff).unwrap_or(0);
                (DURATION_MAX * left + 1000) / 2000
            }
        }
        _ => 0,
    };

    let title = points(TITLE_MAX, similarity(&query.title, &candidate.title));

    // Primary artist of the query against the best of the candidate's artists.
    let artist_sim = query.artists.first().map_or(0, |q| {
        candidate
            .artists
            .iter()
            .map(|c| similarity(q, c))
            .max()
            .unwrap_or(0)
    });
    let artist = points(ARTIST_MAX, artist_sim);

    let (album, album_max) = match (&query.album, &candidate.album) {
        (Some(q), Some(c)) if !normalize(q).is_empty() && !normalize(c).is_empty() => {
            (points(ALBUM_MAX, similarity(q, c)), ALBUM_MAX)
        }
        _ => (0, 0),
    };

    let available = TITLE_MAX + ARTIST_MAX + album_max + DURATION_MAX;
    let earned = title + artist + album + duration;
    let total = ((earned * 100 + available / 2) / available).min(100);

    Some(MatchScore {
        total: to_u8(total),
        title: to_u8(title),
        artist: to_u8(artist),
        album: to_u8(album),
        duration: to_u8(duration),
    })
}

/// Latest timestamp in a doc (line and word starts and ends), if any.
fn last_timestamp(doc: &LyricsDoc) -> Option<u64> {
    doc.lines
        .iter()
        .flat_map(|l| {
            let words = l.words.iter().flat_map(|w| [Some(w.start_ms), w.end_ms]);
            [l.start_ms, l.end_ms].into_iter().chain(words)
        })
        .flatten()
        .max()
}

/// True if a synced doc is a wrong match: its last timestamp is more than 5 s past the track
/// duration. Unsynced docs, docs without timestamps and unknown durations are never rejected.
#[must_use]
pub fn reject_wrong_match(doc: &LyricsDoc, track_duration_ms: Option<u64>) -> bool {
    use nemo_core::lyrics::Granularity;
    if doc.granularity == Granularity::Unsynced {
        return false;
    }
    match (last_timestamp(doc), track_duration_ms) {
        (Some(last), Some(dur)) => last > dur.saturating_add(WRONG_MATCH_SLACK_MS),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemo_core::lyrics::{Granularity, LyricLine, ProviderId};

    fn query(dur: Option<u64>) -> LyricsQuery {
        LyricsQuery {
            title: "Test Song".into(),
            artists: vec!["Test Artist".into(), "Other".into()],
            album: Some("Test Album".into()),
            duration_ms: dur,
            ytm_video_id: None,
            local_path: None,
        }
    }

    fn cand(title: &str, artist: &str, album: Option<&str>, dur: Option<u64>) -> MatchedMeta {
        MatchedMeta {
            title: title.into(),
            artists: vec![artist.into()],
            album: album.map(Into::into),
            duration_ms: dur,
        }
    }

    fn exact(dur: Option<u64>) -> MatchedMeta {
        cand("Test Song", "Test Artist", Some("Test Album"), dur)
    }

    #[test]
    fn perfect_match_is_100() {
        let s = score_match(&query(Some(200_000)), &exact(Some(200_000))).expect("scored");
        assert_eq!(
            s,
            MatchScore {
                total: 100,
                title: 40,
                artist: 30,
                album: 10,
                duration: 20
            }
        );
        assert!(s.is_acceptable());
    }

    #[test]
    fn duration_boundaries() {
        // (difference in ms, expected duration points, rejected)
        let cases: &[(u64, u8, bool)] = &[
            (0, 20, false),
            (1000, 20, false),
            (1001, 20, false),
            (1500, 15, false),
            (2000, 10, false),
            (2900, 1, false),
            (3000, 0, false),
            (3001, 0, true),
            (10_000, 0, true),
        ];
        for &(diff, want, rejected) in cases {
            for sign in [1i64, -1] {
                let c_dur = 200_000i64 + sign * i64::try_from(diff).expect("small");
                let c = exact(Some(u64::try_from(c_dur).expect("positive")));
                let got = score_match(&query(Some(200_000)), &c);
                if rejected {
                    assert_eq!(got, None, "diff {diff} sign {sign}");
                } else {
                    assert_eq!(
                        got.expect("scored").duration,
                        want,
                        "diff {diff} sign {sign}"
                    );
                }
            }
        }
    }

    #[test]
    fn unknown_duration_earns_nothing_and_is_not_rejected() {
        let s = score_match(&query(None), &exact(Some(1))).expect("scored");
        assert_eq!(s.duration, 0);
        assert_eq!(s.total, 80);
        let s = score_match(&query(Some(1)), &exact(None)).expect("scored");
        assert_eq!(s.duration, 0);
    }

    #[test]
    fn threshold_69_vs_70() {
        // title 40 + album 10 + artist 0 = 50, plus duration points.
        let q = query(Some(200_000));
        // diff 1100 ms -> duration 19 -> total 69 -> rejected.
        let c = cand("Test Song", "Nobody", Some("Test Album"), Some(201_100));
        let s = score_match(&q, &c).expect("scored");
        assert_eq!((s.total, s.duration), (69, 19));
        assert!(!s.is_acceptable());
        // diff 1000 ms -> duration 20 -> total 70 -> accepted.
        let c = cand("Test Song", "Nobody", Some("Test Album"), Some(201_000));
        let s = score_match(&q, &c).expect("scored");
        assert_eq!(s.total, 70);
        assert!(s.is_acceptable());
    }

    #[test]
    fn threshold_on_struct() {
        let mk = |total, title| MatchScore {
            total,
            title,
            artist: 0,
            album: 0,
            duration: 0,
        };
        assert!(!mk(69, 40).is_acceptable());
        assert!(mk(70, 40).is_acceptable());
        assert!(!mk(100, 0).is_acceptable());
    }

    #[test]
    fn title_must_score() {
        // Artist + album + duration = 60 only; and even at a higher total, title 0 rejects.
        let c = cand(
            "Completely Different",
            "Test Artist",
            Some("Test Album"),
            Some(200_000),
        );
        let s = score_match(&query(Some(200_000)), &c).expect("scored");
        assert_eq!((s.title, s.total), (0, 60));
        assert!(!s.is_acceptable());
    }

    #[test]
    fn missing_album_is_rescaled() {
        let mut q = query(Some(200_000));
        q.album = None;
        let s = score_match(&q, &exact(Some(200_000))).expect("scored");
        assert_eq!((s.total, s.album), (100, 0));
        let c = cand("Test Song", "Test Artist", None, Some(200_000));
        let s = score_match(&query(Some(200_000)), &c).expect("scored");
        assert_eq!(s.total, 100);
    }

    #[test]
    fn noise_in_titles_does_not_hurt() {
        let c = cand(
            "Test Song (feat. X) - Remastered 2011",
            "TEST ARTIST",
            Some("test album"),
            Some(200_000),
        );
        let s = score_match(&query(Some(200_000)), &c).expect("scored");
        assert_eq!(s.total, 100);
    }

    #[test]
    fn partial_title_overlap_gets_partial_points() {
        let c = cand(
            "Test Song Reprise",
            "Test Artist",
            Some("Test Album"),
            Some(200_000),
        );
        let s = score_match(&query(Some(200_000)), &c).expect("scored");
        // Dice: 2 * 2 / (2 + 3) = 0.8 -> 32 points.
        assert_eq!(s.title, 32);
    }

    #[test]
    fn empty_artist_scores_zero() {
        let mut q = query(Some(1));
        q.artists.clear();
        let s = score_match(&q, &exact(Some(1))).expect("scored");
        assert_eq!(s.artist, 0);
    }

    fn doc(g: Granularity, starts: &[Option<u64>]) -> LyricsDoc {
        LyricsDoc {
            lines: starts
                .iter()
                .map(|&s| LyricLine {
                    start_ms: s,
                    end_ms: None,
                    text: "la".into(),
                    words: vec![],
                })
                .collect(),
            granularity: g,
            provider: ProviderId::new("test"),
            matched: None,
            instrumental: false,
        }
    }

    #[test]
    fn wrong_match_boundary() {
        let d = |last| doc(Granularity::Line, &[Some(1000), Some(last)]);
        assert!(!reject_wrong_match(&d(205_000), Some(200_000)));
        assert!(reject_wrong_match(&d(205_001), Some(200_000)));
        assert!(!reject_wrong_match(&d(999_999), None));
        assert!(!reject_wrong_match(
            &doc(Granularity::Unsynced, &[None, None]),
            Some(1)
        ));
        assert!(!reject_wrong_match(&doc(Granularity::Line, &[]), Some(1)));
        assert!(!reject_wrong_match(&d(u64::MAX), Some(u64::MAX)));
    }

    #[test]
    fn wrong_match_looks_at_word_and_end_times() {
        let mut d = doc(Granularity::Word, &[Some(1000)]);
        d.lines[0].end_ms = Some(300_000);
        assert!(reject_wrong_match(&d, Some(200_000)));
        d.lines[0].end_ms = None;
        d.lines[0].words.push(nemo_core::lyrics::LyricWord {
            start_ms: 100,
            end_ms: Some(400_000),
            text: "la".into(),
        });
        assert!(reject_wrong_match(&d, Some(200_000)));
    }
}
