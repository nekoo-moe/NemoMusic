//! The slow drift of the window backdrop (living-backdrop spec): when it may move, where each
//! layer sits at a given moment, and the capped frame clock. Pure maths, no GUI dependency; the
//! widget in `gui/views/backdrop/field.rs` only reads these answers.
//!
//! The backdrop is the cover's pre-blurred canvas texture drawn twice (the second copy turned
//! half a circle and fainter), so two soft colour fields move against each other like a mesh
//! gradient. Each layer moves a few percent on a closed path and breathes slightly in scale over
//! one long cycle. Frames are capped at [`motion::AMBIENT_FPS`]; nothing moves under reduced
//! motion, under reduce transparency, while the window is not focused, while nothing plays,
//! while the backdrop is hidden and on the CPU renderer.

use std::time::{Duration, Instant};

use crate::theme::motion;

/// Largest translation of a layer, as a fraction of the window's larger side.
pub const SHIFT: f32 = 0.03;
/// Base scale of a layer over the window's larger side. It leaves room for the shift and the
/// breathe, so no edge of the texture ever shows (tested).
pub const BASE_SCALE: f32 = 1.12;
/// Scale breathe around [`BASE_SCALE`] (plus or minus).
pub const BREATHE: f32 = 0.02;
/// Opacity of the second, turned layer over the first.
pub const BLOB_ALPHA: f32 = 0.5;
/// Opacity of `surface-container-lowest` at the bottom of the window backdrop (a vignette for
/// depth; transparent at the top).
pub const VIGNETTE_ALPHA: f32 = 0.45;
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

// The cap stays low (living-backdrop spec: 20 to 30 fps), checked when the crate builds.
const _: () = assert!(motion::AMBIENT_FPS >= 20 && motion::AMBIENT_FPS <= 30);

/// Time between two drift frames.
#[must_use]
pub fn frame_interval() -> Duration {
    Duration::from_micros(1_000_000 / u64::from(motion::AMBIENT_FPS))
}

/// The drift clock: time spent moving, so a frozen backdrop resumes from the same pose.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Clock {
    /// Milliseconds of drift so far.
    pub elapsed_ms: f64,
    last: Option<Instant>,
}

impl Clock {
    /// One redraw at `now`. While `moving`, advance by the time since the last redraw (at most
    /// [`MAX_STEP_MS`]) and return when the next drift frame is due; otherwise stop the clock
    /// and ask for nothing, so an idle window draws no frames.
    pub fn tick(&mut self, now: Instant, moving: bool) -> Option<Instant> {
        if !moving {
            self.last = None;
            return None;
        }
        if let Some(prev) = self.last {
            let dt = now.saturating_duration_since(prev).as_secs_f64() * 1000.0;
            self.elapsed_ms += dt.min(MAX_STEP_MS);
        }
        self.last = Some(now);
        Some(now + frame_interval())
    }
}

/// Where one layer sits: offset from the centre (fractions of the window's larger side) and
/// scale over that side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// Horizontal offset.
    pub dx: f32,
    /// Vertical offset.
    pub dy: f32,
    /// Scale.
    pub scale: f32,
}

/// The pose of `layer` (0 or 1) after `elapsed_ms` of drift. A closed figure-eight path, so
/// the cycle repeats with no seam; the second layer runs the other way, a third of a cycle on.
#[must_use]
pub fn pose(elapsed_ms: f64, layer: u8) -> Pose {
    let cycle = f64::from(motion::AMBIENT_CYCLE_MS);
    let turn = (elapsed_ms.rem_euclid(cycle) / cycle) * std::f64::consts::TAU;
    let (dir, lag) = if layer == 0 {
        (1.0, 0.0)
    } else {
        (-1.0, std::f64::consts::TAU / 3.0)
    };
    let a = turn * dir + lag;
    // Drift maths in f64, drawn in f32: the values are small fractions.
    #[allow(clippy::cast_possible_truncation)]
    Pose {
        dx: (f64::from(SHIFT) * a.sin()) as f32,
        dy: (f64::from(SHIFT) * 0.7 * (2.0 * a).sin()) as f32,
        scale: (f64::from(BASE_SCALE) + f64::from(BREATHE) * (a + 1.0).cos()) as f32,
    }
}

/// The square a layer is drawn into for a window `(x, y, w, h)`: centred, `pose.scale` times
/// the larger side, moved by the pose offset. Returns `(x, y, side)`.
#[must_use]
pub fn placed(bounds: (f32, f32, f32, f32), pose: Pose) -> (f32, f32, f32) {
    let (left, top, width, height) = bounds;
    let long = width.max(height);
    let side = long * pose.scale;
    let cx = left + width / 2.0 + pose.dx * long;
    let cy = top + height / 2.0 + pose.dy * long;
    (cx - side / 2.0, cy - side / 2.0, side)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_clock_asks_for_at_most_the_capped_rate() {
        let mut c = Clock::default();
        let t0 = Instant::now();
        let mut now = t0;
        let mut frames = 0_u32;
        // Follow every request for ten seconds.
        while now < t0 + Duration::from_secs(10) {
            let next = c.tick(now, true).expect("moving asks for a frame");
            assert!(next.duration_since(now) >= frame_interval());
            now = next;
            frames += 1;
        }
        assert!(frames <= motion::AMBIENT_FPS * 10 + 1, "{frames} frames");
        assert!(frames >= motion::AMBIENT_FPS * 10 - 1, "{frames} frames");
        assert!((c.elapsed_ms - 10_000.0).abs() < 50.0, "{}", c.elapsed_ms);
    }

    #[test]
    fn a_still_clock_asks_for_nothing_and_resumes_where_it_stopped() {
        let mut c = Clock::default();
        let t0 = Instant::now();
        c.tick(t0, true);
        c.tick(t0 + Duration::from_millis(40), true);
        let frozen = c.elapsed_ms;
        assert_eq!(c.tick(t0 + Duration::from_secs(5), false), None);
        assert!((c.elapsed_ms - frozen).abs() < f64::EPSILON);
        // Resuming after a long pause adds nothing for the pause itself.
        c.tick(t0 + Duration::from_secs(60), true);
        assert!((c.elapsed_ms - frozen).abs() < f64::EPSILON);
        c.tick(t0 + Duration::from_secs(61), true);
        assert!(
            (c.elapsed_ms - frozen - MAX_STEP_MS).abs() < 1e-6,
            "a stall is clamped"
        );
    }

    #[test]
    fn the_drift_is_slow_small_and_seamless() {
        let cycle = f64::from(motion::AMBIENT_CYCLE_MS);
        assert!((30_000.0..=60_000.0).contains(&cycle));
        for layer in [0, 1] {
            let start = pose(0.0, layer);
            let end = pose(cycle, layer);
            assert!((start.dx - end.dx).abs() < 1e-5 && (start.scale - end.scale).abs() < 1e-5);
            // One drift frame moves the layer by well under a pixel of a 1920 px window.
            let step = 1000.0 / f64::from(motion::AMBIENT_FPS);
            for k in 0..200 {
                let at = f64::from(k) * 211.0;
                let (p, q) = (pose(at, layer), pose(at + step, layer));
                let moved = (p.dx - q.dx).hypot(p.dy - q.dy) * 1920.0;
                assert!(moved < 1.0, "{moved} px per frame");
                assert!(p.dx.abs() <= SHIFT && p.dy.abs() <= SHIFT);
                assert!((p.scale - BASE_SCALE).abs() <= BREATHE + 1e-6);
            }
        }
        assert_ne!(pose(5000.0, 0), pose(5000.0, 1), "the layers move apart");
    }

    #[test]
    fn a_layer_always_covers_the_whole_window() {
        for (w, h) in [
            (960.0, 600.0),
            (1366.0, 697.0),
            (1920.0, 1040.0),
            (420.0, 900.0),
        ] {
            for k in 0..480 {
                for layer in [0, 1] {
                    let p = pose(f64::from(k) * 100.0, layer);
                    let (left, top, side) = placed((10.0, 20.0, w, h), p);
                    assert!(left <= 10.0 && top <= 20.0, "{w}x{h} {p:?}");
                    assert!(
                        left + side >= 10.0 + w && top + side >= 20.0 + h,
                        "{w}x{h} {p:?}"
                    );
                }
            }
        }
    }
}
