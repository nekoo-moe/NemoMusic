//! The staggered scroll of the lyrics (`lyrics-rebuild` spec, after Better Lyrics). The list is
//! laid out at its new scroll position at once; every line then glides from where it was to
//! where it is, each with its own duration, so lines below the active one arrive a little after
//! it and the list settles in a soft cascade. Running glides add up, so a new line change in the
//! middle of one starts from where the lines are now. Pure: time is passed in.

use crate::motion::cubic_bezier;

/// Duration of the line that is the active one, ms.
pub const BASE_MS: f32 = 750.0;
/// Lines above the active one add `ln(i + 1)` times this, ms.
pub const ABOVE_LN_MS: f32 = 10.0;
/// Lines below the active one add `ln(i + 1)` times this, ms.
pub const BELOW_LN_MS: f32 = 80.0;
/// Lines below the active one add this much per line of distance, ms.
pub const BELOW_STEP_MS: f32 = 20.0;
/// Lines further than this from the active one move like the one at this distance (they are
/// out of view anyway).
pub const MAX_DISTANCE: usize = 8;
/// Most glides that run at once; the oldest is dropped (it is nearly done by then).
const MAX_RUNS: usize = 12;
/// A shift smaller than this is not worth a glide, px.
const MIN_DELTA: f32 = 0.5;

/// `cubic-bezier(0.86, 0, 0.2, 1)`: a strong ease in and out.
#[must_use]
pub fn ease(t: f32) -> f32 {
    cubic_bezier(0.86, 0.0, 0.2, 1.0, t)
}

/// How long line `line` glides when line `active` is the active one, ms. The distance counts at
/// most [`MAX_DISTANCE`] lines.
#[must_use]
pub fn duration_ms(line: usize, active: usize) -> f32 {
    let dist = |d: usize| {
        // At most MAX_DISTANCE: exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let d = d.min(MAX_DISTANCE) as f32;
        d
    };
    if line < active {
        let i = dist(active - line);
        BASE_MS + (i + 1.0).ln() * ABOVE_LN_MS
    } else {
        let i = dist(line - active);
        BASE_MS + (i + 1.0).ln() * BELOW_LN_MS + i * BELOW_STEP_MS
    }
}

/// One running glide.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Run {
    /// Shift of the scroll, px: where the lines were minus where they are (the signed scroll
    /// delta). A line starts `delta` px away from its place and ends at 0.
    delta: f32,
    /// The active line when the glide started: the distance of a line from it sets its duration.
    active: usize,
    /// Time since it started, ms.
    elapsed_ms: f32,
}

impl Run {
    /// Progress of line `line`, 0 to 1.
    fn left(&self, line: usize) -> f32 {
        let t = self.elapsed_ms / duration_ms(line, self.active);
        1.0 - ease(t.clamp(0.0, 1.0))
    }

    fn done(&self) -> bool {
        self.elapsed_ms >= duration_ms(self.active + MAX_DISTANCE, self.active)
    }
}

/// The scroll position of the list and the glides of its lines.
#[derive(Debug, Clone, PartialEq)]
pub struct Cascade {
    /// Where the list rests, px from its top: the position the lines are laid out at.
    pub value: f32,
    /// Same as `value`: the list is always laid out at its new place and the glides carry the
    /// lines there.
    pub target: f32,
    runs: Vec<Run>,
}

impl Cascade {
    /// A settled scroll at `value`.
    #[must_use]
    pub fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            runs: Vec::with_capacity(MAX_RUNS),
        }
    }

    /// Jump to `value` with no glide (a seek, a manual scroll, a new document, reduced motion).
    pub fn snap(&mut self, value: f32) {
        self.value = value;
        self.target = value;
        self.runs.clear();
    }

    /// Move the rest position to `to`; every line glides from its old place, its duration set by
    /// its distance from line `active`. Moves under [`MIN_DELTA`] px are taken at once.
    pub fn retarget(&mut self, to: f32, active: usize) {
        let delta = to - self.value;
        self.value = to;
        self.target = to;
        if delta.abs() < MIN_DELTA {
            return;
        }
        if self.runs.len() >= MAX_RUNS {
            self.runs.remove(0);
        }
        self.runs.push(Run {
            delta,
            active,
            elapsed_ms: 0.0,
        });
    }

    /// Advance the glides by `dt_s` seconds.
    pub fn step(&mut self, dt_s: f32) {
        let passed_ms = dt_s.max(0.0) * 1000.0;
        for run in &mut self.runs {
            run.elapsed_ms += passed_ms;
        }
        self.runs.retain(|r| !r.done());
    }

    /// `true` when no line is gliding.
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.runs.is_empty()
    }

    /// Extra distance of line `line` from its place in the layout, px: all running glides added.
    /// Positive means below its place.
    #[must_use]
    pub fn offset(&self, line: usize) -> f32 {
        self.runs.iter().map(|r| r.delta * r.left(line)).sum()
    }

    /// The most any line can be away from its place now, px: how far outside the viewport a line
    /// may start a glide.
    #[must_use]
    pub fn reach(&self) -> f32 {
        self.runs.iter().map(|r| r.delta.abs()).sum()
    }
}

#[cfg(test)]
mod tests;
