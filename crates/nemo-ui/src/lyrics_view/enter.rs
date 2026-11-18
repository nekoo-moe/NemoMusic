//! The two moments around the arrival of lyrics (`lyrics-rebuild` spec, after Better Lyrics): the
//! calm shimmer of the loading line, and the list sliding in from below once the lyrics are
//! there. Pure: time is passed in.

use crate::motion::cubic_bezier;

/// The list slides in over this long, ms.
pub const ENTER_MS: f32 = 600.0;
/// How far below its place the list starts, as a share of the viewport height.
pub const ENTER_SLIDE: f32 = 0.25;
/// One pass of the shimmer, ms.
pub const SHIMMER_MS: f32 = 3000.0;
/// Alpha of the shimmer's highlight at its middle, over the grey ink.
pub const SHIMMER_PEAK: f32 = 0.55;

/// Progress of the list entering, 0 to 1, `elapsed_ms` after the lyrics arrived, with
/// `cubic-bezier(0.22, 1, 0.36, 1)`.
#[must_use]
pub fn enter_progress(elapsed_ms: f32) -> f32 {
    cubic_bezier(
        0.22,
        1.0,
        0.36,
        1.0,
        (elapsed_ms / ENTER_MS).clamp(0.0, 1.0),
    )
}

/// Where the shimmer's highlight is on its way across the text, 0 (before the text) to 1 (past
/// it), `elapsed_ms` into the loading; it repeats every [`SHIMMER_MS`].
#[must_use]
pub fn shimmer_phase(elapsed_ms: f32) -> f32 {
    (elapsed_ms.rem_euclid(SHIMMER_MS)) / SHIMMER_MS
}

/// Alpha of the shimmer's highlight `offset` bands from its middle (`-1` to `1` is inside it):
/// a soft bump that is 0 at the edges.
#[must_use]
pub fn shimmer_alpha(offset: f32) -> f32 {
    let x = offset.abs().min(1.0);
    SHIMMER_PEAK * (1.0 - x) * (1.0 - x) * (1.0 + 2.0 * x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_slides_in_over_600_ms_and_starts_fast() {
        assert!(enter_progress(0.0).abs() < 1e-4);
        assert!((enter_progress(600.0) - 1.0).abs() < 1e-4);
        assert!((enter_progress(5000.0) - 1.0).abs() < 1e-4);
        assert!(
            enter_progress(150.0) > 0.4,
            "decelerating: {}",
            enter_progress(150.0)
        );
        let mut last = 0.0;
        for t in 0..=60_u8 {
            let p = enter_progress(f32::from(t) * 10.0);
            assert!(p >= last - 1e-5, "{t}");
            last = p;
        }
    }

    #[test]
    fn the_shimmer_loops_every_three_seconds() {
        assert!(shimmer_phase(0.0).abs() < 1e-6);
        assert!((shimmer_phase(1500.0) - 0.5).abs() < 1e-6);
        assert!((shimmer_phase(3000.0 + 750.0) - 0.25).abs() < 1e-5);
        assert!(shimmer_phase(2999.0) < 1.0);
        assert!(shimmer_phase(-750.0) > 0.7, "never negative");
    }

    #[test]
    fn the_highlight_is_a_soft_bump() {
        assert!((shimmer_alpha(0.0) - SHIMMER_PEAK).abs() < 1e-6);
        assert!(shimmer_alpha(1.0).abs() < 1e-6 && shimmer_alpha(3.0).abs() < 1e-6);
        assert!(shimmer_alpha(0.5) > 0.0 && shimmer_alpha(0.5) < SHIMMER_PEAK);
        assert!(
            (shimmer_alpha(-0.4) - shimmer_alpha(0.4)).abs() < 1e-6,
            "symmetric"
        );
    }
}
