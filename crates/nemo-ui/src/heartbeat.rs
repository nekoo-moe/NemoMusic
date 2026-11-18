//! The gentle heartbeat of the window backdrop (song-colour-theme spec): a soft double pulse,
//! "lub-dub", in a cycle of 1.6 s, layered over the slow drift. Pure maths; the widget in
//! `gui/views/backdrop/field.rs` reads it and applies [`SCALE`] and [`LIGHT`].
//!
//! The pulse runs on the drift clock, so it follows the same stop rules (paused, unfocused,
//! reduced motion, reduce transparency, backdrop hidden, CPU renderer) and the same 24 fps
//! redraw requests: it costs no extra frames, no `update` or `view` call and no re-blur.

use crate::drift::Mode;
use crate::theme::{Rgba, motion};

/// Scale of the colour fields at the top of a beat: 2.5 % (about).
pub const SCALE: f32 = 0.025;
/// Opacity of the white lift over the fields at the top of a beat: 5 % brightness (about).
pub const LIGHT: f32 = 0.05;

/// The colour of the lift: white, at [`LIGHT`] times the pulse, over the fields. White lifts a
/// dark backdrop and a pale one alike, so contrast over the panes only improves in light mode.
pub const LIFT: Rgba = Rgba::from_rgb_hex(0xFF_FFFF);

/// Where the first beat ("lub") peaks, as a fraction of the cycle.
const LUB_AT: f64 = 0.10;
/// Where the second beat ("dub") peaks.
const DUB_AT: f64 = 0.34;
/// Height of the second beat against the first.
const DUB_HEIGHT: f64 = 0.7;
/// Half the width of one beat, as a fraction of the cycle.
const HALF_WIDTH: f64 = 0.10;

// The cycle is about 1.6 s and each beat is smooth (checked when the crate builds).
const _: () = assert!(motion::HEART_CYCLE_MS >= 1_400 && motion::HEART_CYCLE_MS <= 1_800);
const _: () = assert!(SCALE <= 0.03 && LIGHT <= 0.06);

/// One smooth beat: a raised cosine of height 1 at `at`, 0 further than [`HALF_WIDTH`] away.
fn bump(phase: f64, at: f64) -> f64 {
    let d = (phase - at).abs();
    if d >= HALF_WIDTH {
        0.0
    } else {
        0.5 * (1.0 + (std::f64::consts::PI * d / HALF_WIDTH).cos())
    }
}

/// The pulse after `elapsed_ms` of drift clock: 0 at rest, 1 at the top of the first beat.
#[must_use]
pub fn beat(elapsed_ms: f64) -> f32 {
    let phase = elapsed_ms.rem_euclid(f64::from(motion::HEART_CYCLE_MS))
        / f64::from(motion::HEART_CYCLE_MS);
    let v = bump(phase, LUB_AT) + DUB_HEIGHT * bump(phase, DUB_AT);
    // At most 1: the two beats do not overlap.
    #[allow(clippy::cast_possible_truncation)]
    let out = v.clamp(0.0, 1.0) as f32;
    out
}

/// `true` when the backdrop pulses: the heartbeat is available and on, and the backdrop moves
/// (so every stop rule of the drift stops it too).
#[must_use]
pub const fn active(mode: Mode, switch_on: bool) -> bool {
    switch_on && matches!(mode, Mode::Moving)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drift::Inputs;

    #[test]
    fn it_is_a_double_pulse_that_rests_between_cycles() {
        let cycle = f64::from(motion::HEART_CYCLE_MS);
        assert!((beat(0.0)).abs() < 1e-6, "rest at the start of the cycle");
        assert!((beat(cycle * LUB_AT) - 1.0).abs() < 1e-5, "lub is the top");
        let dub = beat(cycle * DUB_AT);
        assert!((dub - 0.7).abs() < 1e-5, "dub is lower: {dub}");
        assert!(beat(cycle * 0.25) < 0.1, "a dip between the two beats");
        assert!(beat(cycle * 0.7).abs() < 1e-6, "rest after the beats");
        assert!(
            (beat(cycle * 3.0 + 250.0) - beat(250.0)).abs() < 1e-5,
            "periodic"
        );
    }

    #[test]
    fn it_is_smooth_at_the_capped_frame_rate_and_stays_in_range() {
        let step = 1000.0 / f64::from(motion::AMBIENT_FPS);
        let mut t = 0.0;
        let mut prev = beat(0.0);
        while t < 4_000.0 {
            t += step;
            let now = beat(t);
            assert!((0.0..=1.0).contains(&now), "{now}");
            assert!(
                (now - prev).abs() < 0.45,
                "jump of {} at {t} ms",
                (now - prev).abs()
            );
            prev = now;
        }
        // About 2.5 % scale and 5 % brightness at the top.
        assert!((0.02..=0.03).contains(&SCALE) && (0.04..=0.06).contains(&LIGHT));
    }

    #[test]
    fn every_stop_rule_of_the_drift_stops_the_pulse_too() {
        let on = Inputs {
            reduce_motion: false,
            reduce_transparency: false,
            focused: true,
            playing: true,
            visible: true,
            software: false,
        };
        assert!(active(Mode::of(on), true));
        assert!(!active(Mode::of(on), false), "the switch is off");
        for off in [
            Inputs {
                playing: false,
                ..on
            },
            Inputs {
                focused: false,
                ..on
            },
            Inputs {
                reduce_motion: true,
                ..on
            },
            Inputs {
                reduce_transparency: true,
                ..on
            },
            Inputs {
                visible: false,
                ..on
            },
            Inputs {
                software: true,
                ..on
            },
        ] {
            assert!(!active(Mode::of(off), true), "{off:?}");
        }
    }
}
