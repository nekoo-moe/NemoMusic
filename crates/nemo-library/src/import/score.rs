//! Scoring a catalog result against an imported song (playlist-import design).
//!
//! Score = 0.5 x title + 0.3 x artist + 0.2 x duration, in whole percent. Title and artist
//! similarity is the share of normalized words in common (words in common divided by the
//! larger word count). Duration is 1.0 within 3 s, falls to 0 at 15 s, and is 0.5 when
//! either side is unknown. Confidence: high at 85 or more with the durations within 5 s (or
//! unknown), medium at 65, low at 45, none below. The thresholds are starting values, to be
//! tuned with the owner's sample playlists (requirements, Open Question 1). Integer math only.

use std::collections::HashSet;
use std::time::Duration;

use nemo_core::import::{Confidence, ImportedSong, MatchCandidate};
use nemo_core::track::Track;

use super::normalize::tokens;

/// Lowest score for [`Confidence::High`] (also needs [`HIGH_DURATION`]).
pub const HIGH: u8 = 85;
/// Lowest score for [`Confidence::Medium`].
pub const MEDIUM: u8 = 65;
/// Lowest score for [`Confidence::Low`].
pub const LOW: u8 = 45;
/// Largest duration difference a high-confidence match may have.
pub const HIGH_DURATION: Duration = Duration::from_secs(5);
/// Durations this close score full marks.
const FULL_DURATION_MS: u128 = 3_000;
/// Durations this far apart score zero.
const ZERO_DURATION_MS: u128 = 15_000;

/// Share of words in common, in percent: words in both divided by the larger word count.
/// Two empty texts score 0.
#[must_use]
pub fn similarity(a: &str, b: &str) -> u32 {
    let a: HashSet<String> = tokens(a).into_iter().collect();
    let b: HashSet<String> = tokens(b).into_iter().collect();
    let larger = a.len().max(b.len());
    if larger == 0 {
        return 0;
    }
    let common = a.intersection(&b).count();
    u32::try_from(common * 100 / larger).unwrap_or(0)
}

fn difference(a: Option<Duration>, b: Option<Duration>) -> Option<Duration> {
    Some(a?.abs_diff(b?))
}

/// Duration agreement in percent: 100 within 3 s, 0 at 15 s or more, 50 when unknown.
#[must_use]
pub fn duration_score(a: Option<Duration>, b: Option<Duration>) -> u32 {
    let Some(diff) = difference(a, b).map(|d| d.as_millis()) else {
        return 50;
    };
    if diff <= FULL_DURATION_MS {
        100
    } else if diff >= ZERO_DURATION_MS {
        0
    } else {
        let left = ZERO_DURATION_MS - diff;
        u32::try_from(left * 100 / (ZERO_DURATION_MS - FULL_DURATION_MS)).unwrap_or(0)
    }
}

/// The score of `track` for `song`, 0 to 100.
#[must_use]
pub fn score(song: &ImportedSong, track: &Track) -> u8 {
    let title = similarity(&song.title, &track.title);
    let artist = similarity(&song.artists.join(" "), &track.artists.join(" "));
    let duration = duration_score(song.duration, track.duration);
    u8::try_from((50 * title + 30 * artist + 20 * duration) / 100).unwrap_or(100)
}

/// The confidence of a score, given the two durations.
#[must_use]
pub fn confidence(score: u8, song: Option<Duration>, track: Option<Duration>) -> Confidence {
    let close = difference(song, track).is_none_or(|d| d <= HIGH_DURATION);
    if score >= HIGH && close {
        Confidence::High
    } else if score >= MEDIUM {
        Confidence::Medium
    } else if score >= LOW {
        Confidence::Low
    } else {
        Confidence::None
    }
}

/// `track` as a scored candidate for `song`.
#[must_use]
pub fn candidate(song: &ImportedSong, track: Track) -> MatchCandidate {
    let score = score(song, &track);
    let confidence = confidence(score, song.duration, track.duration);
    MatchCandidate {
        track,
        score,
        confidence,
    }
}

#[cfg(test)]
mod tests;
