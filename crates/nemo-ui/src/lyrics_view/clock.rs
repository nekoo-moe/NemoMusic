//! The two lyric clocks (`lyrics-rebuild` spec, after Better Lyrics). Pure: time is passed in.
//!
//! - The **scroll clock** is audio time plus [`SCROLL_LEAD_MS`]. A line is *active* once it has
//!   started on this clock, so the list scrolls to it half a second before it is sung.
//! - The **audio clock** is the playback position plus the track offset plus the sync offset of
//!   the lyrics ([`sync_offset_ms`]). A line is *animating* from [`PREPARE_MS`] before its start
//!   until its last word has finished, so animation can be prepared and nothing is cut short.
//!
//! A line is *lit* from the moment it is active until it stops animating: it stays bright while
//! its last word is still being sung even though the scroll clock has moved on. Two lines may be
//! lit at once (backing vocals, overlaps).

use nemo_core::lyrics::{Granularity, LyricLine, LyricsDoc};

use crate::motion::cubic_bezier;

/// Line-synced lyrics are shown this much earlier, ms.
pub const LINE_SYNC_OFFSET_MS: i64 = 115;
/// Word- and syllable-synced lyrics are shown this much earlier, ms.
pub const WORD_SYNC_OFFSET_MS: i64 = 150;
/// The scroll clock runs this far ahead of the audio clock, ms.
pub const SCROLL_LEAD_MS: i64 = 500;
/// A line starts animating this long before it is sung, ms.
pub const PREPARE_MS: i64 = 2000;
/// Length of a line's light-up and dim-down, ms.
pub const LIT_FADE_MS: f32 = 166.0;
/// A line lasts at least this long on the audio clock, ms.
const MIN_LIT_MS: i64 = 300;
/// A line without any end time is lit this long, ms.
const FALLBACK_LIT_MS: i64 = 4000;
/// The last word without an end time lasts this long, ms.
const LAST_WORD_MS: i64 = 600;
/// Most lines looked at behind the newest started line when searching for lit lines.
const LOOK_BACK: usize = 8;

/// The sync offset of a lyrics level, ms.
#[must_use]
pub const fn sync_offset_ms(g: Granularity) -> i64 {
    match g {
        Granularity::Syllable | Granularity::Word => WORD_SYNC_OFFSET_MS,
        Granularity::Line => LINE_SYNC_OFFSET_MS,
        Granularity::Unsynced => 0,
    }
}

/// Both clocks at one playback position, ms of song time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clocks {
    /// Audio clock: position + track offset + sync offset.
    pub audio_ms: i64,
    /// Scroll clock: audio clock + [`SCROLL_LEAD_MS`].
    pub scroll_ms: i64,
}

impl Clocks {
    /// The clocks for lyrics of level `g` at `position_ms` with the user's `offset_ms`.
    #[must_use]
    pub fn new(g: Granularity, position_ms: i64, offset_ms: i64) -> Self {
        let audio_ms = position_ms
            .saturating_add(offset_ms)
            .saturating_add(sync_offset_ms(g));
        Self {
            audio_ms,
            scroll_ms: audio_ms.saturating_add(SCROLL_LEAD_MS),
        }
    }
}

fn ms(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX / 4)
}

fn start_of(line: &LyricLine) -> Option<i64> {
    line.start_ms.map(ms)
}

/// When one line starts and stops, on the audio clock, ms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineWindow {
    /// The line's own start.
    pub start_ms: i64,
    /// When its last word has finished (or, for a line without word timing, when the next line
    /// takes over).
    pub end_ms: i64,
}

fn ease(x: f32) -> f32 {
    cubic_bezier(0.25, 0.1, 0.25, 1.0, x)
}

impl LineWindow {
    /// The line is *animating*: from [`PREPARE_MS`] before its start until it has finished.
    #[must_use]
    pub fn animating(&self, c: &Clocks) -> bool {
        c.audio_ms >= self.start_ms.saturating_sub(PREPARE_MS) && c.audio_ms < self.end_ms
    }

    /// The line is *lit*: active on the scroll clock and not yet finished on the audio clock.
    #[must_use]
    pub fn lit(&self, c: &Clocks) -> bool {
        c.scroll_ms >= self.start_ms && c.audio_ms < self.end_ms
    }

    /// How lit the line looks, 0 to 1: it rises over [`LIT_FADE_MS`] when it becomes active and
    /// falls over the same time after it has finished. A pure function of the clocks, so a seek
    /// needs no state.
    #[must_use]
    pub fn lit_progress(&self, c: &Clocks) -> f32 {
        let open = self.start_ms.saturating_sub(SCROLL_LEAD_MS);
        let rise = ease(frac(c.audio_ms.saturating_sub(open)));
        let fall = 1.0 - ease(frac(c.audio_ms.saturating_sub(self.end_ms)));
        rise.min(fall)
    }

    /// The line still draws a change (lit, or dimming down): frames are needed.
    #[must_use]
    pub fn busy(&self, c: &Clocks) -> bool {
        let p = self.lit_progress(c);
        p > 0.0 && (p < 1.0 || self.lit(c))
    }
}

/// `delta_ms` as a share of [`LIT_FADE_MS`], 0 to 1.
fn frac(delta_ms: i64) -> f32 {
    // A few hundred ms: exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let d = delta_ms.clamp(0, 1000) as f32;
    (d / LIT_FADE_MS).clamp(0.0, 1.0)
}

/// The windows of every line of one document, built once when the lyrics arrive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Timeline {
    lines: Vec<Option<LineWindow>>,
}

impl Timeline {
    /// The windows of `doc`. Unsynced documents have none.
    #[must_use]
    pub fn new(doc: &LyricsDoc) -> Self {
        if doc.granularity == Granularity::Unsynced {
            return Self::default();
        }
        let starts: Vec<Option<i64>> = doc.lines.iter().map(start_of).collect();
        let lines = doc
            .lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let start = starts.get(i).copied().flatten()?;
                let next = starts.get(i + 1..)?.iter().find_map(|s| *s);
                Some(window(l, start, next))
            })
            .collect();
        Self { lines }
    }

    /// The window of line `i`, when it has a start.
    #[must_use]
    pub fn window(&self, i: usize) -> Option<LineWindow> {
        self.lines.get(i).copied().flatten()
    }

    /// Index of the active line on the scroll clock: the last line that has started.
    #[must_use]
    pub fn active(&self, c: &Clocks) -> Option<usize> {
        self.started(c).checked_sub(1)
    }

    /// Number of leading lines that have started on the scroll clock.
    fn started(&self, c: &Clocks) -> usize {
        self.lines
            .iter()
            .take_while(|w| w.is_some_and(|w| w.start_ms <= c.scroll_ms))
            .count()
    }

    /// Indices of the lit lines, newest first, without allocating.
    pub fn lit<'a>(&'a self, c: &'a Clocks) -> impl Iterator<Item = usize> + 'a {
        let hi = self.started(c);
        (hi.saturating_sub(LOOK_BACK)..hi)
            .rev()
            .filter(|i| self.window(*i).is_some_and(|w| w.lit(c)))
    }

    /// `true` while any line is lit or still dimming down.
    #[must_use]
    pub fn busy(&self, c: &Clocks) -> bool {
        let hi = self.started(c);
        (hi.saturating_sub(LOOK_BACK)..hi).any(|i| self.window(i).is_some_and(|w| w.busy(c)))
    }
}

fn window(line: &LyricLine, start: i64, next: Option<i64>) -> LineWindow {
    let end = if let Some(last) = line.words.last() {
        // Lit until the last word is done: its own end, else the line's, else a short tail.
        last.end_ms
            .or(line.end_ms)
            .map_or(ms(last.start_ms) + LAST_WORD_MS, ms)
    } else {
        let handover = next.map(|n| n - SCROLL_LEAD_MS);
        match (line.end_ms.map(ms), handover) {
            (Some(e), Some(h)) => e.min(h),
            (Some(e), None) => e,
            (None, Some(h)) => h,
            (None, None) => start + FALLBACK_LIT_MS,
        }
    };
    LineWindow {
        start_ms: start,
        end_ms: end.max(start + MIN_LIT_MS),
    }
}

#[cfg(test)]
mod tests;
