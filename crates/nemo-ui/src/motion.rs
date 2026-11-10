//! Motion maths (`design.md` section 8): easing, the accent cross-fade and a critically damped
//! spring. Pure and deterministic: time is passed in, never read from a clock.

#![allow(
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::manual_midpoint
)] // colour and easing maths: short names are the standard notation, casts are of small bounded values

use crate::palette::Palette;
use crate::theme::motion::ACCENT_CROSSFADE_MS;

/// Evaluate a CSS-style cubic bezier easing `(x1, y1, x2, y2)` at time fraction `x` (0 to 1).
#[must_use]
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    let coord = |a: f32, b: f32, u: f32| {
        let inv = 1.0 - u;
        3.0 * inv * inv * u * a + 3.0 * inv * u * u * b + u * u * u
    };
    // Solve coord(x1, x2, u) = x for u by bisection; x(u) is monotonic for valid control points.
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..24 {
        let mid = 0.5 * (lo + hi);
        if coord(x1, x2, mid) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    coord(y1, y2, 0.5 * (lo + hi))
}

/// Emphasized-decelerate easing, `cubic-bezier(0.05, 0.7, 0.1, 1)`.
#[must_use]
pub fn emphasized_decelerate(t: f32) -> f32 {
    cubic_bezier(0.05, 0.7, 0.1, 1.0, t)
}

/// An accent cross-fade between two palettes, lasting [`ACCENT_CROSSFADE_MS`].
/// Only one derived palette is on screen at a time: a new target while a fade runs starts from
/// the colours showing right now.
#[derive(Debug, Clone, PartialEq)]
pub struct Crossfade {
    from: Palette,
    to: Palette,
    elapsed_ms: f32,
    duration_ms: f32,
}

impl Crossfade {
    /// A settled fade that shows `palette`.
    #[must_use]
    pub fn settled(palette: Palette) -> Self {
        Self {
            from: palette,
            to: palette,
            elapsed_ms: ACCENT_CROSSFADE_MS as f32,
            duration_ms: ACCENT_CROSSFADE_MS as f32,
        }
    }

    /// Start fading to `target` from what is showing now. With `reduced_motion` the swap is
    /// instant.
    pub fn retarget(&mut self, target: Palette, reduced_motion: bool) {
        if target == self.to {
            return;
        }
        self.from = self.current();
        self.to = target;
        self.elapsed_ms = if reduced_motion {
            self.duration_ms
        } else {
            0.0
        };
    }

    /// Advance by `dt_ms` milliseconds.
    pub fn advance(&mut self, dt_ms: f32) {
        self.elapsed_ms = (self.elapsed_ms + dt_ms.max(0.0)).min(self.duration_ms);
    }

    /// Eased progress, 0 (all `from`) to 1 (all `to`).
    #[must_use]
    pub fn progress(&self) -> f32 {
        emphasized_decelerate(self.elapsed_ms / self.duration_ms)
    }

    /// The palette to draw now.
    #[must_use]
    pub fn current(&self) -> Palette {
        if self.is_done() {
            return self.to;
        }
        self.from.mix(&self.to, self.progress())
    }

    /// `true` once the fade reached its target (no frames are needed).
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }

    /// The palette the fade is heading to.
    #[must_use]
    pub fn target(&self) -> Palette {
        self.to
    }
}

/// A critically damped spring on one value (`spatial.default`: stiffness 700, damping ratio 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    /// Current value.
    pub value: f32,
    velocity: f32,
    /// Where the value is heading.
    pub target: f32,
    stiffness: f32,
}

impl Spring {
    /// `spatial.default` stiffness.
    pub const DEFAULT_STIFFNESS: f32 = 700.0;
    /// Distance and speed under which the spring counts as settled.
    const REST: f32 = 0.25;

    /// A spring at rest at `value`.
    #[must_use]
    pub fn new(value: f32) -> Self {
        Self {
            value,
            velocity: 0.0,
            target: value,
            stiffness: Self::DEFAULT_STIFFNESS,
        }
    }

    /// Jump to `value` without animating.
    pub fn snap(&mut self, value: f32) {
        self.value = value;
        self.target = value;
        self.velocity = 0.0;
    }

    /// `true` once at rest on the target.
    #[must_use]
    pub fn is_settled(&self) -> bool {
        (self.value - self.target).abs() < Self::REST && self.velocity.abs() < Self::REST
    }

    /// Advance by `dt_s` seconds (substeps of at most 4 ms keep the integration stable).
    pub fn step(&mut self, dt_s: f32) {
        let damping = 2.0 * self.stiffness.sqrt();
        let mut left = dt_s.clamp(0.0, 0.1);
        while left > 0.0 {
            let h = left.min(0.004);
            let accel = self.stiffness * (self.target - self.value) - damping * self.velocity;
            self.velocity += accel * h;
            self.value += self.velocity * h;
            left -= h;
        }
        if self.is_settled() {
            self.value = self.target;
            self.velocity = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::{Seed, Tones, build_palette};

    fn pal(hue: f64) -> Palette {
        build_palette(Seed { hue, chroma: 0.12 }, Tones::DARK)
    }

    #[test]
    fn easing_hits_its_endpoints_and_is_monotonic() {
        assert!(emphasized_decelerate(0.0).abs() < 1e-4);
        assert!((emphasized_decelerate(1.0) - 1.0).abs() < 1e-4);
        let mut prev = 0.0;
        for i in 1..=50_u8 {
            let v = emphasized_decelerate(f32::from(i) / 50.0);
            assert!(v >= prev - 1e-6, "step {i}");
            prev = v;
        }
        // Decelerate: fast start.
        assert!(emphasized_decelerate(0.25) > 0.5);
    }

    #[test]
    fn crossfade_runs_400_ms_and_settles() {
        let (a, b) = (pal(30.0), pal(260.0));
        let mut fade = Crossfade::settled(a);
        assert!(fade.is_done());
        fade.retarget(b, false);
        assert!(!fade.is_done());
        assert_eq!(fade.current(), a);
        fade.advance(399.0);
        assert!(!fade.is_done());
        fade.advance(1.0);
        assert!(fade.is_done());
        assert_eq!(fade.current(), b);
    }

    #[test]
    fn crossfade_midpoint_is_between_and_legible() {
        let (a, b) = (pal(30.0), pal(260.0));
        let mut fade = Crossfade::settled(a);
        fade.retarget(b, false);
        for _ in 0..40 {
            fade.advance(10.0);
            assert!(fade.current().passes());
        }
    }

    #[test]
    fn retarget_mid_fade_starts_from_what_is_showing() {
        let (a, b, c) = (pal(30.0), pal(150.0), pal(260.0));
        let mut fade = Crossfade::settled(a);
        fade.retarget(b, false);
        fade.advance(100.0);
        let showing = fade.current();
        fade.retarget(c, false);
        assert_eq!(fade.current(), showing, "no jump when the target changes");
        fade.advance(400.0);
        assert_eq!(fade.current(), c);
    }

    #[test]
    fn same_target_does_not_restart() {
        let (a, b) = (pal(30.0), pal(260.0));
        let mut fade = Crossfade::settled(a);
        fade.retarget(b, false);
        fade.advance(200.0);
        let before = fade.clone();
        fade.retarget(b, false);
        assert_eq!(fade, before);
    }

    #[test]
    fn reduced_motion_swaps_instantly() {
        let (a, b) = (pal(30.0), pal(260.0));
        let mut fade = Crossfade::settled(a);
        fade.retarget(b, true);
        assert!(fade.is_done());
        assert_eq!(fade.current(), b);
    }

    #[test]
    fn spring_settles_without_overshoot() {
        let mut s = Spring::new(0.0);
        s.target = 300.0;
        let mut max = 0.0_f32;
        for _ in 0..120 {
            s.step(1.0 / 60.0);
            max = max.max(s.value);
            if s.is_settled() {
                break;
            }
        }
        assert!(s.is_settled());
        assert!((s.value - 300.0).abs() < 1e-3);
        assert!(
            max <= 300.0 + 0.5,
            "critically damped: no overshoot, got {max}"
        );
    }

    #[test]
    fn spring_settles_within_about_half_a_second() {
        let mut s = Spring::new(0.0);
        s.target = 200.0;
        let mut frames = 0;
        while !s.is_settled() && frames < 600 {
            s.step(1.0 / 60.0);
            frames += 1;
        }
        assert!(frames < 40, "took {frames} frames");
    }
}
