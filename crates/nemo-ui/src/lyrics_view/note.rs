//! The intro and gap note (lyrics-polish criteria 1 to 6). Before the first sung line, and in
//! a long instrumental break between two lines, a music note sits where the next line will
//! appear and fills up with the playback position. It is full at the moment that line starts,
//! then fades out with `motion.fast` while the line becomes active as usual.
//!
//! "The line starts" means the moment it becomes the active line, on the scroll clock
//! ([`super::clock::Clocks`]: position, offset, sync offset and a 0.5 s lead). Every time below is
//! on that clock, so the note is full in the same frame the line lights up. Pure: no timer, the
//! numbers come from the position updates only.

use nemo_core::lyrics::{Granularity, LyricLine, LyricsDoc};

use super::clock::Clocks;
use super::{LineRole, row_height_d};
use crate::theme::motion::FAST_MS;

/// A break between two sung lines gets the note when it lasts at least this long, ms. Shorter
/// pauses are breaths and short fills: a note there would come and go with every line. Five
/// seconds is more than one typical sung line (2 to 4 s), so the note only marks real
/// instrumental passages, and it leaves time to see the fill move.
pub const GAP_MIN_MS: i64 = 5_000;
/// The intro gets the note when the first line starts at least this late, ms. A shorter intro
/// would only flash the note.
pub const INTRO_MIN_MS: i64 = 1_000;
/// The note at one playback position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GapNote {
    /// Index of the line the note waits for; the note sits right above it.
    pub before: usize,
    /// How full the note is, 0 to 1: time since the gap started over the gap length. Exactly
    /// 1 from the moment the line starts.
    pub fill: f32,
    /// How present the note is, 0 to 1: it fades in at the gap start and out at the line start,
    /// `motion.fast` each way. Always 1 under reduced motion, and the note goes away at once.
    pub presence: f32,
    /// `true` while the line has not started yet; `false` while the note fades out.
    pub waiting: bool,
}

/// What the lyrics view centres on: the active line, or the waiting note.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Focus {
    /// The line drawn as active. `None` while the note waits: every line is then drawn as an
    /// "other" line and the note takes the anchor.
    pub active: Option<usize>,
    /// The intro or gap note, while it waits or fades out.
    pub note: Option<GapNote>,
}

/// Height in px of the note row at full presence: one active-line row, so the line the note
/// waits for takes its place when it starts.
#[must_use]
pub fn row_height(dense: bool) -> f32 {
    row_height_d(LineRole::Active, dense)
}

fn ms(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

/// A line with words to sing (blank LRC lines mark breaks).
fn sung(line: &LyricLine) -> bool {
    !line.text.trim().is_empty()
}

fn start(line: &LyricLine) -> Option<i64> {
    line.start_ms.map(ms)
}

/// When a line stops being sung: its own end, or the end of its last timed word.
fn end(line: &LyricLine) -> Option<i64> {
    line.end_ms
        .or_else(|| line.words.last().and_then(|w| w.end_ms))
        .map(ms)
}

/// Start of the break before sung line `n`, when the break is long enough for the note. The
/// intro starts at 0. A break between lines needs a known end: the end time of the last sung
/// line, or else a blank line after it. Without either, singing and silence look the same,
/// so no note is shown.
fn gap_start(doc: &LyricsDoc, n: usize) -> Option<i64> {
    let s = start(doc.lines.get(n)?)?;
    let Some(p) = doc.lines.get(..n)?.iter().rposition(sung) else {
        return (s >= INTRO_MIN_MS).then_some(0);
    };
    let g = doc
        .lines
        .get(p)
        .and_then(end)
        .or_else(|| doc.lines.get(p + 1..n)?.iter().find_map(start))?;
    (s.saturating_sub(g) >= GAP_MIN_MS).then_some(g)
}

/// Fraction `num / den`, clamped to 0..=1.
fn ratio(num: i64, den: i64) -> f32 {
    if den <= 0 {
        return 1.0;
    }
    // Times within a song are far below 2^24 ms of difference that matter here.
    #[allow(clippy::cast_precision_loss)]
    let r = num as f32 / den as f32;
    r.clamp(0.0, 1.0)
}

/// The note at `position_ms` with the user's `offset_ms`, or `None` when none shows: no long
/// enough gap, unsynced or instrumental lyrics, or the next line started more than
/// `motion.fast` ago. `reduced` (reduced motion) keeps the fill but drops the fades.
#[must_use]
pub fn gap_note(
    doc: &LyricsDoc,
    position_ms: i64,
    offset_ms: i64,
    reduced: bool,
) -> Option<GapNote> {
    if doc.granularity == Granularity::Unsynced || doc.instrumental {
        return None;
    }
    let u = Clocks::new(doc.granularity, position_ms, offset_ms).scroll_ms;
    let fade = if reduced { 0 } else { i64::from(FAST_MS) };
    // The next sung line: not started yet, or started less than `motion.fast` ago.
    let n = doc
        .lines
        .iter()
        .position(|l| sung(l) && start(l).is_some_and(|s| s.saturating_add(fade) > u))?;
    let s = start(doc.lines.get(n)?)?;
    let g = gap_start(doc, n)?;
    if u < g {
        return None;
    }
    if u < s {
        let presence = if reduced { 1.0 } else { ratio(u - g, fade) };
        return Some(GapNote {
            before: n,
            fill: ratio(u - g, s - g),
            presence,
            waiting: true,
        });
    }
    Some(GapNote {
        before: n,
        fill: 1.0,
        presence: 1.0 - ratio(u - s, fade),
        waiting: false,
    })
}

/// The [`Focus`] at `position_ms`: the note, and the active line unless the note waits.
#[must_use]
pub fn focus(doc: &LyricsDoc, position_ms: i64, offset_ms: i64, reduced: bool) -> Focus {
    let note = gap_note(doc, position_ms, offset_ms, reduced);
    let active = if note.is_some_and(|n| n.waiting) {
        None
    } else {
        super::active_line(doc, position_ms, offset_ms)
    };
    Focus { active, note }
}

#[cfg(test)]
mod tests;
