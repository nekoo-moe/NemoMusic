//! The slow drift of the window backdrop (living-backdrop spec): when it may move, where each
//! layer sits at a given moment, and the capped frame clock. Pure maths, no GUI dependency; the
//! widget in `gui/views/backdrop/field.rs` only reads these answers.
//!
//! The backdrop is the cover's colour mesh (`crate::mesh`): three or four soft colour fields at
//! fixed anchors, each moving a few percent on its own closed path and breathing slightly in
//! scale over one long cycle, so the fields drift against each other like a mesh gradient.
//! The backdrop has no frame timer of its own: it draws on the redraws the window already gets
//! from the app's one frame ticker (`gui/app/subscribe.rs`) and reads their time. Nothing moves
//! under reduced motion, under reduce transparency, while the window is not focused, while
//! nothing plays, while the backdrop is hidden and on the CPU renderer.

use std::time::Instant;

use crate::theme::motion;

/// Largest translation of a field, as a fraction of the window's larger side.
pub const SHIFT: f32 = 0.05;
/// Scale breathe around 1 (plus or minus).
pub const BREATHE: f32 = 0.02;
/// Opacity of `surface-container-lowest` at the bottom of the window backdrop (a vignette for
/// depth; transparent at the top).
pub const VIGNETTE_ALPHA: f32 = 0.55;
/// Opacity of the vignette at 40 % of the window height.
pub const VIGNETTE_MID_ALPHA: f32 = 0.15;
/// Where each colour field sits: horizontal and vertical centre as fractions of the window, and
/// its radius as a fraction of the window's larger side.
pub const ANCHORS: [(f32, f32, f32); crate::mesh::FIELDS] = [
    (0.18, 0.22, 0.55),
    (0.82, 0.18, 0.50),
    (0.22, 0.82, 0.50),
    (0.84, 0.80, 0.55),
];
/// Longest step one frame may add to the clock: after a stall the drift goes on calmly instead
/// of jumping.
pub const MAX_STEP_MS: f64 = 100.0;

/// How the backdrop is drawn now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Slow drift at the capped frame rate.
    Moving,
    /// The cover backdrop, frozen where it was.
    Still,
    /// Reduce transparency: one flat tonal colour, no texture.
    Flat,
}

/// What decides the [`Mode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // independent switches, named for the reader
pub struct Inputs {
    /// The reduce-motion setting or the OS preference.
    pub reduce_motion: bool,
    /// The reduce-transparency setting.
    pub reduce_transparency: bool,
    /// The window has keyboard focus.
    pub focused: bool,
    /// A song is playing (not paused, not stopped).
    pub playing: bool,
    /// The backdrop is on screen (not under the full-screen Now Playing).
    pub visible: bool,
    /// The window draws with the CPU renderer (tiny-skia): there one drift frame costs a
    /// full-window image scale on the CPU, so the backdrop stays still.
    pub software: bool,
}

impl Mode {
    /// The mode for `i`. Reduce transparency wins over everything; any other reason to stop
    /// freezes the backdrop.
    #[must_use]
    pub const fn of(i: Inputs) -> Self {
        if i.reduce_transparency {
            Self::Flat
        } else if i.reduce_motion || !i.focused || !i.playing || !i.visible || i.software {
            Self::Still
        } else {
            Self::Moving
        }
    }
}

/// The drift clock: time spent moving, so a frozen backdrop resumes from the same pose. It reads
/// the time of each redraw and never asks for one.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Clock {
    /// Milliseconds of drift so far.
    pub elapsed_ms: f64,
    last: Option<Instant>,
}

impl Clock {
    /// One redraw at `now`. While `moving`, advance by the time since the last redraw (at most
    /// [`MAX_STEP_MS`]); otherwise stop the clock, so the pause is never counted.
    pub fn tick(&mut self, now: Instant, moving: bool) {
        if !moving {
            self.stop();
            return;
        }
        if let Some(prev) = self.last {
            let dt = now.saturating_duration_since(prev).as_secs_f64() * 1000.0;
            self.elapsed_ms += dt.min(MAX_STEP_MS);
        }
        self.last = Some(now);
    }

    /// Stop the clock (the window lost focus): the next redraw starts from the pose it holds.
    pub fn stop(&mut self) {
        self.last = None;
    }
}

/// Where one field sits: offset from its anchor (fractions of the window's larger side) and
/// scale around 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// Horizontal offset.
    pub dx: f32,
    /// Vertical offset.
    pub dy: f32,
    /// Scale.
    pub scale: f32,
}

/// The pose of field `layer` (0 to 3) after `elapsed_ms` of drift. A closed figure-eight path,
/// so the cycle repeats with no seam; neighbouring fields run the other way, a quarter of a
/// cycle apart.
#[must_use]
pub fn pose(elapsed_ms: f64, layer: u8) -> Pose {
    let cycle = f64::from(motion::AMBIENT_CYCLE_MS);
    let turn = (elapsed_ms.rem_euclid(cycle) / cycle) * std::f64::consts::TAU;
    let dir = if layer.is_multiple_of(2) { 1.0 } else { -1.0 };
    let lag = std::f64::consts::TAU * f64::from(layer) / 4.0;
    let a = turn * dir + lag;
    // Drift maths in f64, drawn in f32: the values are small fractions.
    #[allow(clippy::cast_possible_truncation)]
    Pose {
        dx: (f64::from(SHIFT) * a.sin()) as f32,
        dy: (f64::from(SHIFT) * 0.7 * (2.0 * a).sin()) as f32,
        scale: (1.0 + f64::from(BREATHE) * (a + 1.0).cos()) as f32,
    }
}

/// The square field `i` is drawn into for a window `(x, y, w, h)`: centred on its anchor moved by
/// the pose offset, `2 x radius x pose.scale x extra` of the larger side wide. `extra` is 1 at
/// rest (the bloom and the heartbeat scale it). Returns `(x, y, side)`.
#[must_use]
pub fn blob_rect(
    bounds: (f32, f32, f32, f32),
    i: usize,
    pose: Pose,
    extra: f32,
) -> (f32, f32, f32) {
    let (left, top, width, height) = bounds;
    let (ax, ay, radius) = ANCHORS[i.min(ANCHORS.len() - 1)];
    let long = width.max(height);
    let side = 2.0 * radius * long * pose.scale * extra;
    let cx = left + width * ax + pose.dx * long;
    let cy = top + height * ay + pose.dy * long;
    (cx - side / 2.0, cy - side / 2.0, side)
}

/// The rectangle of field `i` after `elapsed_ms` of drift clock: the drift pose, and with
/// `breathing` the heartbeat's growth and shift on top (`crate::heartbeat`). `layer_scale` is the
/// bloom's scale of the layer (1 once settled). The one place that decides where a field is
/// drawn, for the window and for the tests. Returns `(x, y, side)`.
#[must_use]
pub fn field_rect(
    bounds: (f32, f32, f32, f32),
    i: usize,
    elapsed_ms: f64,
    layer_scale: f32,
    breathing: bool,
) -> (f32, f32, f32) {
    let mut at = pose(elapsed_ms, u8::try_from(i).unwrap_or(0));
    let mut extra = layer_scale;
    if breathing {
        let (ax, ay, _) = ANCHORS[i.min(ANCHORS.len() - 1)];
        let (grow, dx, dy) = crate::heartbeat::nudge(elapsed_ms, ax, ay);
        at.dx += dx;
        at.dy += dy;
        extra *= grow;
    }
    blob_rect(bounds, i, at, extra)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const ON: Inputs = Inputs {
        reduce_motion: false,
        reduce_transparency: false,
        focused: true,
        playing: true,
        visible: true,
        software: false,
    };

    #[test]
    fn only_a_focused_playing_visible_window_drifts() {
        assert_eq!(Mode::of(ON), Mode::Moving);
        for off in [
            Inputs {
                reduce_motion: true,
                ..ON
            },
            Inputs {
                focused: false,
                ..ON
            },
            Inputs {
                playing: false,
                ..ON
            },
            Inputs {
                visible: false,
                ..ON
            },
            Inputs {
                software: true,
                ..ON
            },
        ] {
            assert_eq!(Mode::of(off), Mode::Still, "{off:?}");
        }
        let flat = Inputs {
            reduce_transparency: true,
            ..ON
        };
        assert_eq!(Mode::of(flat), Mode::Flat);
        assert_eq!(
            Mode::of(Inputs {
                reduce_motion: true,
                ..flat
            }),
            Mode::Flat
        );
    }

    #[test]
    fn the_clock_follows_the_redraws_and_asks_for_none() {
        let mut c = Clock::default();
        let t0 = Instant::now();
        // Ten seconds of redraws at about 60 per second, each a little early or late.
        for k in 0..600_u64 {
            let jitter = if k % 3 == 0 { 2 } else { 0 };
            c.tick(t0 + Duration::from_micros(k * 16_667 + jitter * 1000), true);
        }
        assert!((c.elapsed_ms - 9_983.0).abs() < 25.0, "{}", c.elapsed_ms);
    }

    #[test]
    fn a_still_clock_resumes_where_it_stopped() {
        let mut c = Clock::default();
        let t0 = Instant::now();
        c.tick(t0, true);
        c.tick(t0 + Duration::from_millis(40), true);
        let frozen = c.elapsed_ms;
        c.tick(t0 + Duration::from_secs(5), false);
        assert!((c.elapsed_ms - frozen).abs() < f64::EPSILON);
        // Resuming after a long pause adds nothing for the pause itself.
        c.tick(t0 + Duration::from_secs(60), true);
        assert!((c.elapsed_ms - frozen).abs() < f64::EPSILON);
        c.tick(t0 + Duration::from_secs(61), true);
        assert!(
            (c.elapsed_ms - frozen - MAX_STEP_MS).abs() < 1e-6,
            "a stall is clamped"
        );
        // A lost focus stops the clock the same way.
        c.stop();
        c.tick(t0 + Duration::from_secs(90), true);
        assert!((c.elapsed_ms - frozen - MAX_STEP_MS).abs() < 1e-6);
    }

    #[test]
    fn the_drift_is_slow_small_and_seamless() {
        let cycle = f64::from(motion::AMBIENT_CYCLE_MS);
        assert!((30_000.0..=60_000.0).contains(&cycle));
        for layer in 0..4 {
            let start = pose(0.0, layer);
            let end = pose(cycle, layer);
            assert!((start.dx - end.dx).abs() < 1e-5 && (start.scale - end.scale).abs() < 1e-5);
            // One drift frame (60 per second) moves a field by under a pixel of a 1920 px window.
            let step = 1000.0 / 60.0;
            for k in 0..200 {
                let at = f64::from(k) * 211.0;
                let (p, q) = (pose(at, layer), pose(at + step, layer));
                let moved = (p.dx - q.dx).hypot(p.dy - q.dy) * 1920.0;
                assert!(moved < 1.0, "{moved} px per frame");
                assert!(p.dx.abs() <= SHIFT && p.dy.abs() <= SHIFT);
                assert!((p.scale - 1.0).abs() <= BREATHE + 1e-6);
            }
        }
        assert_ne!(pose(5000.0, 0), pose(5000.0, 1), "the fields move apart");
    }

    #[test]
    fn a_field_stays_near_its_anchor_and_scales_with_extra() {
        for (w, h) in [(960.0, 600.0), (1366.0, 697.0), (1920.0, 1040.0)] {
            let long = f32::max(w, h);
            for (i, &(ax, ay, r)) in ANCHORS.iter().enumerate() {
                for k in 0..480 {
                    let p = pose(f64::from(k) * 100.0, u8::try_from(i).unwrap_or(0));
                    let (x, y, side) = blob_rect((10.0, 20.0, w, h), i, p, 1.0);
                    let (cx, cy) = (x + side / 2.0, y + side / 2.0);
                    assert!((cx - (10.0 + w * ax)).abs() <= SHIFT * long + 0.01);
                    assert!((cy - (20.0 + h * ay)).abs() <= SHIFT * long + 0.01);
                    assert!((side - 2.0 * r * long * p.scale).abs() < 0.01);
                }
            }
        }
        let rest = blob_rect((0.0, 0.0, 1000.0, 600.0), 0, pose(0.0, 0), 1.0);
        let big = blob_rect((0.0, 0.0, 1000.0, 600.0), 0, pose(0.0, 0), 1.04);
        assert!((big.2 / rest.2 - 1.04).abs() < 1e-4);
    }

    #[test]
    fn the_breath_only_grows_the_fields_a_little_and_nudges_them_inward() {
        let b = (0.0, 0.0, 1280.0, 800.0);
        let top = f64::from(motion::HEART_CYCLE_MS) / 2.0;
        for i in 0..ANCHORS.len() {
            let rest = field_rect(b, i, top, 1.0, false);
            assert_eq!(
                rest,
                blob_rect(b, i, pose(top, u8::try_from(i).unwrap_or(0)), 1.0)
            );
            let (x, y, side) = field_rect(b, i, top, 1.0, true);
            assert!((side / rest.2 - 1.02).abs() < 1e-4, "grows by 2 %");
            let (cx, cy) = (x + side / 2.0, y + side / 2.0);
            let (rx, ry) = (rest.0 + rest.2 / 2.0, rest.1 + rest.2 / 2.0);
            assert!(((cx - rx).abs().max((cy - ry).abs())) < 0.015 * 1280.0);
            // Toward the middle of the window.
            assert!((cx - 640.0).abs() <= (rx - 640.0).abs() + 1e-3);
        }
    }
}
