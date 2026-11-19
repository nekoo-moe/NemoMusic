//! The motion of the setup flow (brand-intro spec, task 3): what moves when the step changes.
//!
//! Pure. A [`Clock`] counts the milliseconds since the step changed; the views ask the curves
//! below for one moment, so tests and snapshots can show any of them. Nothing here lasts more
//! than [`TOTAL_MS`] (400 ms, `motion.slow`).

use super::SetupStep;
use crate::motion::{emphasized_decelerate, spring_response};

/// Distance the step's content slides in, in px.
pub const SLIDE_PX: f32 = 24.0;
/// Distance a row rises on top of that, in px.
pub const ROW_RISE_PX: f32 = 8.0;
/// Delay between one row and the next, in ms.
pub const STAGGER_MS: f32 = 40.0;
/// Rows after this one start together, so a long step still ends in time.
pub const LAST_STAGGERED_ROW: usize = 5;
/// The whole transition, in ms: the last staggered row ends here.
pub const TOTAL_MS: f32 = 400.0;
/// `motion.base` in ms.
const BASE: f32 = 200.0;
/// Scale the mark starts the Done settle from.
const SETTLE_FROM: f32 = 0.88;
/// Stiffness and damping ratio of the Done settle (one small overshoot).
const SETTLE_STIFFNESS: f32 = 380.0;
const SETTLE_DAMPING: f32 = 0.6;

/// Eased 0 to 1 progress of something that starts at `at` ms and lasts [`BASE`].
fn eased(t_ms: f32, at: f32) -> f32 {
    emphasized_decelerate(((t_ms - at) / BASE).clamp(0.0, 1.0))
}

/// Progress of the step's content as a block: it slides [`SLIDE_PX`] and fades in.
#[must_use]
pub fn block(t_ms: f32) -> f32 {
    eased(t_ms, 0.0)
}

/// Progress of row `index` of the step: it rises [`ROW_RISE_PX`] and fades in, [`STAGGER_MS`]
/// after the row before it.
#[must_use]
pub fn row(t_ms: f32, index: usize) -> f32 {
    // At most `LAST_STAGGERED_ROW`: exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let start = STAGGER_MS * index.min(LAST_STAGGERED_ROW) as f32;
    eased(t_ms, start)
}

/// Progress of the stepper: a marker that changes state morphs from one to the other.
#[must_use]
pub fn morph(t_ms: f32) -> f32 {
    eased(t_ms, 0.0)
}

/// Scale of the mark on the Done step: a small settle with one overshoot, once.
#[must_use]
pub fn settle(t_ms: f32) -> f32 {
    let secs = (t_ms / 1000.0).max(0.0);
    SETTLE_FROM + (1.0 - SETTLE_FROM) * spring_response(secs, SETTLE_STIFFNESS, SETTLE_DAMPING)
}

/// The milliseconds since the step changed, and the step it changed from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clock {
    ms: f32,
    from: Option<SetupStep>,
}

impl Clock {
    /// A clock that has run out: nothing moves.
    #[must_use]
    pub const fn settled() -> Self {
        Self {
            ms: TOTAL_MS,
            from: None,
        }
    }

    /// The step changed from `from` (`None` when the flow just opened).
    pub fn restart(&mut self, from: Option<SetupStep>) {
        self.ms = 0.0;
        self.from = from;
    }

    /// Jump to the end (reduced motion, or the flow closed).
    pub fn finish(&mut self) {
        self.ms = TOTAL_MS;
    }

    /// Move the clock by `dt_ms`.
    pub fn advance(&mut self, dt_ms: f32) {
        self.ms = (self.ms + dt_ms.max(0.0)).min(TOTAL_MS);
    }

    /// Milliseconds since the step changed, at most [`TOTAL_MS`].
    #[must_use]
    pub fn ms(&self) -> f32 {
        self.ms
    }

    /// The step the flow came from, if it moved from one.
    #[must_use]
    pub fn from(&self) -> Option<SetupStep> {
        self.from
    }

    /// `true` once nothing moves (no frames are needed).
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.ms >= TOTAL_MS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::motion::{BASE_MS, SLOW_MS};

    #[test]
    fn the_constants_follow_the_motion_tokens() {
        let ms = |v: u32| f32::from(u16::try_from(v).unwrap_or(u16::MAX));
        assert!((BASE - ms(BASE_MS)).abs() < f32::EPSILON);
        assert!((TOTAL_MS - ms(SLOW_MS)).abs() < f32::EPSILON);
    }

    #[test]
    fn the_block_runs_one_motion_base_and_the_stagger_ends_in_400_ms() {
        assert!(block(0.0).abs() < 1e-4);
        assert!((block(BASE) - 1.0).abs() < 1e-3);
        let last = row(TOTAL_MS, 99);
        assert!(
            (last - 1.0).abs() < 1e-3,
            "the last row ends in time: {last}"
        );
        assert!(row(TOTAL_MS - 1.0, LAST_STAGGERED_ROW) < 1.0);
    }

    #[test]
    fn rows_start_forty_milliseconds_apart() {
        assert!(row(39.0, 1).abs() < 1e-4, "row 1 waits");
        assert!(row(41.0, 1) > 0.0);
        assert!(row(100.0, 0) > row(100.0, 1));
        assert!(row(100.0, 1) > row(100.0, 2));
    }

    #[test]
    fn the_done_settle_overshoots_once_and_rests_at_one_inside_400_ms() {
        assert!((settle(0.0) - SETTLE_FROM).abs() < 1e-4);
        let peak = (0..=400_u16)
            .map(|t| settle(f32::from(t)))
            .fold(0.0, f32::max);
        assert!((1.005..1.05).contains(&peak), "a small overshoot: {peak}");
        assert!((settle(TOTAL_MS) - 1.0).abs() < 0.01);
    }

    #[test]
    fn the_clock_stops_at_400_ms_and_a_restart_replays_it() {
        let mut c = Clock::settled();
        assert!(c.is_settled());
        c.restart(Some(SetupStep::Welcome));
        assert!(!c.is_settled());
        assert_eq!(c.from(), Some(SetupStep::Welcome));
        c.advance(399.0);
        assert!(!c.is_settled());
        c.advance(50.0);
        assert!(c.is_settled());
        assert!((c.ms() - TOTAL_MS).abs() < f32::EPSILON);
        c.restart(None);
        c.finish();
        assert!(c.is_settled());
    }
}
