//! The gentle heartbeat of the window backdrop (song-colour-theme and backdrop-calm specs): a slow
//! breath of the colour fields. Every field grows by [`SCALE`] and its centre moves a little
//! toward the middle of the window, eased in and out over [`motion::HEART_CYCLE_MS`] (2.6 s).
//! It moves the fields and changes no colour: nothing is lifted, so the brightness of the window
//! stays within a point or two of its range. Pure maths; `drift::field_rect` applies it.
//!
//! The breath runs on the drift clock, so it follows the same stop rules (paused, unfocused,
//! reduced motion, reduce transparency, backdrop hidden, CPU renderer): it costs no frame, no
//! `update` and no `view` call of its own and no re-blur.

use crate::theme::motion;

/// Growth of the colour fields at the top of a breath: 2 %.
pub const SCALE: f32 = 0.02;
/// How far a field's centre moves toward the middle of the window at the top of a breath, as a
/// share of the distance between the two (a field 0.3 of the window away moves 0.9 % of it).
pub const SHIFT: f32 = 0.03;

// One breath takes 2 to 3 s and grows the fields by 1.5 to 2.5 % (checked when the crate builds).
const _: () = assert!(motion::HEART_CYCLE_MS >= 2_000 && motion::HEART_CYCLE_MS <= 3_000);
const _: () = assert!(SCALE >= 0.015 && SCALE <= 0.025);

/// Where the backdrop is in its breath after `elapsed_ms` of drift clock: 0 at rest, 1 at the
/// top. A raised cosine, so the speed is 0 at both ends and there is no corner anywhere.
#[must_use]
pub fn breath(elapsed_ms: f64) -> f32 {
    let cycle = f64::from(motion::HEART_CYCLE_MS);
    let phase = elapsed_ms.rem_euclid(cycle) / cycle;
    #[allow(clippy::cast_possible_truncation)]
    let out = (0.5 - 0.5 * (std::f64::consts::TAU * phase).cos()) as f32;
    out
}

/// What the breath does to the field anchored at `(ax, ay)` (fractions of the window): the
/// factor for its size and the shift of its centre, in fractions of the window's larger side.
#[must_use]
pub fn nudge(elapsed_ms: f64, ax: f32, ay: f32) -> (f32, f32, f32) {
    let b = breath(elapsed_ms);
    (
        1.0 + SCALE * b,
        SHIFT * (0.5 - ax) * b,
        SHIFT * (0.5 - ay) * b,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_one_slow_eased_breath_per_cycle() {
        let cycle = f64::from(motion::HEART_CYCLE_MS);
        assert!(breath(0.0).abs() < 1e-6, "rest at the start");
        assert!(
            (breath(cycle / 2.0) - 1.0).abs() < 1e-6,
            "top in the middle"
        );
        assert!(breath(cycle).abs() < 1e-6, "rest again after one cycle");
        assert!(
            (breath(cycle * 3.0 + 400.0) - breath(400.0)).abs() < 1e-5,
            "periodic"
        );
        // Eased: the first and last 100 ms hardly move.
        assert!(breath(100.0) < 0.02 && breath(cycle - 100.0) < 0.02);
    }

    #[test]
    fn it_stays_in_range_and_moves_a_little() {
        let mut t = 0.0;
        while t < 6_000.0 {
            assert!((0.0..=1.0).contains(&breath(t)), "{t}");
            t += 7.0;
        }
        let (grow, dx, dy) = nudge(f64::from(motion::HEART_CYCLE_MS) / 2.0, 0.18, 0.22);
        assert!((grow - 1.02).abs() < 1e-6, "{grow}");
        // A field on the left moves right, one above the middle moves down; under 1.5 % of the
        // window's larger side.
        assert!(
            dx > 0.0 && dy > 0.0 && dx < 0.015 && dy < 0.015,
            "{dx} {dy}"
        );
        let (rest, rx, ry) = nudge(0.0, 0.18, 0.22);
        assert!((rest - 1.0).abs() < 1e-6 && rx.abs() < 1e-6 && ry.abs() < 1e-6);
    }
}
