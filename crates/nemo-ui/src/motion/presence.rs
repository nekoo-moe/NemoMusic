//! [`Presence`]: the enter and exit of one surface (popover, menu, sheet, toast, tooltip, a
//! fading picture). A timed tween on the `motion.*` tokens of `design.md` section 8: it enters
//! with emphasized-decelerate and leaves with emphasized-accelerate, never longer than 400 ms.
//! Pure: time is passed in. Reduced motion jumps to the end (a 0 ms opacity swap).

use super::{emphasized_accelerate, emphasized_decelerate};
use crate::theme::motion::{BASE_MS, FAST_MS, SLOW_MS};

/// See the module docs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presence {
    /// Value when the current leg started.
    from: f32,
    /// Heading in (1) or out (0).
    on: bool,
    /// Progress of the current leg, 0 to 1.
    k: f32,
    enter_ms: f32,
    exit_ms: f32,
}

impl Presence {
    /// A hidden, settled surface that enters in `enter_ms` and leaves in `exit_ms` (both capped
    /// at `motion.slow`).
    #[must_use]
    pub fn new(enter_ms: u32, exit_ms: u32) -> Self {
        // Durations are at most 400, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let cap = |ms: u32| ms.clamp(1, SLOW_MS) as f32;
        Self {
            from: 0.0,
            on: false,
            k: 1.0,
            enter_ms: cap(enter_ms),
            exit_ms: cap(exit_ms),
        }
    }

    /// Popovers, menus, sheets and toasts: in with `motion.base`, out with `motion.fast`.
    #[must_use]
    pub fn popup() -> Self {
        Self::new(BASE_MS, FAST_MS)
    }

    /// Show or hide. A change mid-way starts from the value showing now (no jump). With
    /// `reduced` the value jumps to its end.
    pub fn set(&mut self, on: bool, reduced: bool) {
        if on != self.on {
            self.from = self.value();
            self.on = on;
            self.k = 0.0;
        }
        if reduced {
            self.finish();
        }
    }

    /// Start a fresh enter from 0 (a new toast replacing the old one).
    pub fn restart(&mut self, reduced: bool) {
        self.from = 0.0;
        self.on = true;
        self.k = if reduced { 1.0 } else { 0.0 };
    }

    /// Jump to the end of the current leg.
    pub fn finish(&mut self) {
        self.k = 1.0;
        self.from = self.target();
    }

    /// Advance by `dt_ms` milliseconds. A leg that starts part-way takes the matching share of
    /// its duration.
    pub fn advance(&mut self, dt_ms: f32) {
        if self.is_settled() {
            return;
        }
        let full = if self.on { self.enter_ms } else { self.exit_ms };
        let leg = (full * (self.target() - self.from).abs()).max(1.0);
        self.k += dt_ms.max(0.0) / leg;
        // Sums of frame steps land a hair under 1; that is the end.
        if self.k > 1.0 - 1e-4 {
            self.k = 1.0;
        }
    }

    fn target(&self) -> f32 {
        if self.on { 1.0 } else { 0.0 }
    }

    /// Eased presence: 0 hidden, 1 fully shown.
    #[must_use]
    pub fn value(&self) -> f32 {
        if self.is_settled() {
            return self.target();
        }
        let e = if self.on {
            emphasized_decelerate(self.k)
        } else {
            emphasized_accelerate(self.k)
        };
        (self.from + (self.target() - self.from) * e).clamp(0.0, 1.0)
    }

    /// `true` once the value rests (no frames are needed for it).
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.k >= 1.0
    }

    /// `true` while the surface must be drawn: shown, or still leaving.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.on || !self.is_settled()
    }

    /// `true` when the surface is (or is heading to) shown.
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.on
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(p: &mut Presence, ms: f32) {
        let mut left = ms;
        while left > 0.0 {
            p.advance(16.0_f32.min(left));
            left -= 16.0;
        }
    }

    #[test]
    fn a_new_presence_is_hidden_and_settled() {
        let p = Presence::popup();
        assert!(p.is_settled() && !p.visible());
        assert!(p.value().abs() < 1e-6);
    }

    #[test]
    fn popup_enters_in_motion_base_and_leaves_in_motion_fast() {
        let mut p = Presence::popup();
        p.set(true, false);
        assert!(p.visible() && !p.is_settled());
        assert!(p.value().abs() < 1e-6);
        run(&mut p, 100.0);
        // Decelerate: well past half way at half time.
        assert!(p.value() > 0.7, "{}", p.value());
        run(&mut p, 100.0);
        assert!(p.is_settled() && (p.value() - 1.0).abs() < 1e-6);
        p.set(false, false);
        assert!(p.visible());
        run(&mut p, 50.0);
        // Accelerate: still mostly there at half time.
        assert!(p.value() > 0.6, "{}", p.value());
        run(&mut p, 50.0);
        assert!(p.is_settled() && !p.visible());
    }

    #[test]
    fn a_reversal_starts_from_what_is_showing() {
        let mut p = Presence::popup();
        p.set(true, false);
        run(&mut p, 48.0);
        let showing = p.value();
        p.set(false, false);
        assert!((p.value() - showing).abs() < 1e-6, "no jump");
        // Leaving from part-way takes less than the full exit.
        run(&mut p, 100.0);
        assert!(!p.visible());
    }

    #[test]
    fn setting_the_same_state_does_not_restart() {
        let mut p = Presence::popup();
        p.set(true, false);
        run(&mut p, 64.0);
        let before = p;
        p.set(true, false);
        assert_eq!(p, before);
    }

    #[test]
    fn reduced_motion_swaps_at_once() {
        let mut p = Presence::popup();
        p.set(true, true);
        assert!(p.is_settled() && (p.value() - 1.0).abs() < 1e-6);
        p.set(false, true);
        assert!(p.is_settled() && !p.visible());
        p.restart(true);
        assert!(p.is_settled() && (p.value() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn nothing_lasts_longer_than_motion_slow() {
        let mut p = Presence::new(5_000, 5_000);
        p.set(true, false);
        run(&mut p, 400.0);
        assert!(p.is_settled());
    }

    #[test]
    fn values_at_fixed_times() {
        let mut p = Presence::new(200, 100);
        p.set(true, false);
        p.advance(50.0);
        let quarter = p.value();
        assert!((quarter - emphasized_decelerate(0.25)).abs() < 1e-5);
        p.advance(150.0);
        assert!((p.value() - 1.0).abs() < 1e-6);
        p.set(false, false);
        p.advance(25.0);
        assert!((p.value() - (1.0 - emphasized_accelerate(0.25))).abs() < 1e-5);
    }
}
