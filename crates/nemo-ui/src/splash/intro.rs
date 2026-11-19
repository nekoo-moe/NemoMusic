//! The brand intro (brand-intro spec, task 1): what the splash draws at any moment.
//!
//! Pure. Every number is a function of the time since the first frame, so a view (or a test, or
//! a snapshot) asks for one moment and nothing is stored between frames. Durations come from the
//! motion tokens; the one overshoot is the mark's scale-in.

use crate::motion::{emphasized_decelerate, spring_response};
use crate::theme::Rgba;
use crate::theme::motion::{BASE_MS, HEART_CYCLE_MS, SLOW_MS};

/// The splash stays at least this long, so the signature plays through (requirements, 4).
pub const MIN_SHOW_MS: u64 = 1_200;
/// With reduced motion nothing plays; the floor only keeps the splash from flashing.
pub const MIN_SHOW_REDUCED_MS: u64 = 500;
/// The wordmark.
pub const WORDMARK: &str = "NemoMusic";
/// The line under the wordmark (`product.md`).
pub const TAGLINE: &str = "A quiet room for your music.";

/// The mark starts to scale in.
const MARK_AT: u64 = 80;
/// Mark spring: stiffness, and a damping ratio of 0.6 (overshoot of about 9.5 %, the one
/// expressive overshoot of the intro; the play button's 0.8 would not show on a mark).
const MARK_STIFFNESS: f32 = 380.0;
const MARK_DAMPING: f32 = 0.6;
/// The mark fades in over `motion.fast` while it grows.
const MARK_FADE_MS: u64 = 100;
/// The note rises into the mark.
const GLYPH_AT: u64 = 240;
/// Distance the note rises, in px (at most 16 by `design.md` section 8).
const GLYPH_RISE: f32 = 12.0;
/// The wordmark's first letter.
const WORD_AT: u64 = 420;
/// Time between two letters.
pub const LETTER_STEP_MS: u64 = 35;
/// Distance a letter rises, in px.
const LETTER_RISE: f32 = 8.0;
/// The tagline follows the last letter.
const TAGLINE_AT: u64 = WORD_AT + 8 * LETTER_STEP_MS + 60;
/// Distance the tagline rises, in px.
const TAGLINE_RISE: f32 = 4.0;
/// The progress line fades in.
const RAIL_AT: u64 = 500;
/// The bloom starts to grow, and when it starts to breathe.
const BLOOM_AT: u64 = 0;
const BREATH_AT: u64 = BLOOM_AT + SLOW_MS as u64;
/// Smallest bloom, as a share of its full size.
const BLOOM_FROM: f32 = 0.7;
/// Growth of the bloom at the top of a breath.
const BREATH_GROW: f32 = 0.04;
/// Share of the bloom's opacity that does not breathe.
const BREATH_BASE: f32 = 0.82;

/// Side of the generated bloom texture.
pub const BLOOM_PX: u32 = 96;
/// Opacity of the bloom at its centre, before it fades in and breathes.
pub const BLOOM_PEAK: f32 = 0.30;
/// Diameter of the bloom at full size, in px.
pub const BLOOM_SIZE: f32 = 560.0;
/// Diameter of the mark disc at rest, in px.
pub const MARK_SIZE: f32 = 96.0;
/// Size of the note inside the mark, in px.
pub const GLYPH_SIZE: f32 = 48.0;
/// Space between the mark and the wordmark, in px.
pub const GAP_MARK: f32 = 24.0;
/// Space between the wordmark and the tagline, in px.
pub const GAP_TAGLINE: f32 = 8.0;
/// Height of the box of a wordmark letter: the line (`display-md`, 48) plus the rise, so a
/// rising letter never moves its neighbours.
pub const WORD_BOX: f32 = 48.0 + LETTER_RISE;
/// The same for the tagline (`body-md`, 20).
pub const TAGLINE_BOX: f32 = 20.0 + TAGLINE_RISE;
/// Height of the whole signature (mark, wordmark, tagline), in px.
pub const SIGNATURE_H: f32 = MARK_SIZE + GAP_MARK + WORD_BOX + GAP_TAGLINE + TAGLINE_BOX;

/// One thing that rises into place while it fades in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reveal {
    /// 0 to 1.
    pub opacity: f32,
    /// Distance still to rise, in px (0 at rest).
    pub rise_px: f32,
}

impl Reveal {
    /// Fully shown, in place.
    pub const REST: Self = Self {
        opacity: 1.0,
        rise_px: 0.0,
    };
}

/// What the splash draws at one moment, without the progress line's own fill.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// Scale of the mark disc (above 1 during the overshoot).
    pub mark_scale: f32,
    /// Opacity of the mark disc.
    pub mark_opacity: f32,
    /// The note inside the mark.
    pub glyph: Reveal,
    /// Opacity of the bloom, 0 to 1.
    pub bloom_opacity: f32,
    /// Scale of the bloom against its full size.
    pub bloom_scale: f32,
    /// The tagline.
    pub tagline: Reveal,
    /// Opacity of the progress line and its step name.
    pub rail_opacity: f32,
}

/// Eased 0 to 1 progress of something that starts at `at` and lasts `ms`.
fn eased(t_ms: u64, at: u64, ms: u64) -> f32 {
    if t_ms <= at {
        return 0.0;
    }
    // Both numbers are under a few thousand: exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let p = (t_ms - at) as f32 / ms as f32;
    emphasized_decelerate(p.min(1.0))
}

fn reveal(t_ms: u64, at: u64, ms: u64, rise: f32) -> Reveal {
    let e = eased(t_ms, at, ms);
    Reveal {
        opacity: e,
        rise_px: rise * (1.0 - e),
    }
}

/// 0 to 1 and back in the heartbeat cycle, eased at both ends; 0 before it starts.
fn breath(t_ms: u64) -> f32 {
    if t_ms <= BREATH_AT {
        return 0.0;
    }
    // Under a minute of splash: exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let phase = (t_ms - BREATH_AT) as f32 / HEART_CYCLE_MS as f32;
    0.5 - 0.5 * (std::f32::consts::TAU * phase).cos()
}

impl Frame {
    /// The intro at rest: everything in place, the bloom still. Used for reduced motion and for
    /// the failure screen.
    pub const REST: Self = Self {
        mark_scale: 1.0,
        mark_opacity: 1.0,
        glyph: Reveal::REST,
        bloom_opacity: 1.0,
        bloom_scale: 1.0,
        tagline: Reveal::REST,
        rail_opacity: 1.0,
    };

    /// The intro `t_ms` after the first frame. With `reduced` it is always [`Frame::REST`].
    #[must_use]
    pub fn at(t_ms: u64, reduced: bool) -> Self {
        if reduced {
            return Self::REST;
        }
        let grow = eased(t_ms, BLOOM_AT, u64::from(SLOW_MS));
        let b = breath(t_ms);
        // Milliseconds are small: exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let since_mark = t_ms.saturating_sub(MARK_AT) as f32 / 1000.0;
        Self {
            mark_scale: spring_response(since_mark, MARK_STIFFNESS, MARK_DAMPING),
            mark_opacity: eased(t_ms, MARK_AT, MARK_FADE_MS),
            glyph: reveal(t_ms, GLYPH_AT, u64::from(SLOW_MS), GLYPH_RISE),
            bloom_opacity: grow * (BREATH_BASE + (1.0 - BREATH_BASE) * b),
            bloom_scale: (BLOOM_FROM + (1.0 - BLOOM_FROM) * grow) * (1.0 + BREATH_GROW * b),
            tagline: reveal(t_ms, TAGLINE_AT, u64::from(BASE_MS), TAGLINE_RISE),
            rail_opacity: eased(t_ms, RAIL_AT, u64::from(BASE_MS)),
        }
    }

    /// Letter `i` of the wordmark: [`LETTER_STEP_MS`] after the letter before it.
    #[must_use]
    pub fn letter(i: usize, t_ms: u64, reduced: bool) -> Reveal {
        if reduced {
            return Reveal::REST;
        }
        let at = WORD_AT + u64::try_from(i).unwrap_or(0) * LETTER_STEP_MS;
        reveal(t_ms, at, u64::from(BASE_MS), LETTER_RISE)
    }

    /// The moment the last thing has come to rest.
    #[must_use]
    pub fn settled_at() -> u64 {
        TAGLINE_AT + u64::from(BASE_MS)
    }
}

/// The bloom texture: `color` at a smooth radial opacity, `peak` (0 to 1) at the centre and 0 at
/// the edge, [`BLOOM_PX`] square, RGBA8 with straight alpha. Built once, then scaled by the
/// renderer.
#[must_use]
pub fn bloom_rgba(color: Rgba, peak: f32) -> Vec<u8> {
    let px = BLOOM_PX as usize;
    let mut out = Vec::with_capacity(px * px * 4);
    for y in 0..px {
        for x in 0..px {
            // Pixel indexes are tiny: exact in f32.
            #[allow(clippy::cast_precision_loss)]
            let unit = |v: usize| (v as f32 + 0.5) / px as f32 * 2.0 - 1.0;
            let (dx, dy) = (unit(x), unit(y));
            let d2 = (dx * dx + dy * dy).min(1.0);
            let falloff = (1.0 - d2) * (1.0 - d2);
            // 0 to 255 by construction.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let alpha = (falloff * peak.clamp(0.0, 1.0) * 255.0).round() as u8;
            out.extend_from_slice(&[color.r, color.g, color.b, alpha]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheme::{AccentIntensity, BaseMode, ThemeChoice, accent_palette};
    use crate::theme::contrast_ratio;

    #[test]
    fn the_first_frame_draws_nothing_but_the_surface() {
        let f = Frame::at(0, false);
        assert!(f.mark_opacity.abs() < f32::EPSILON);
        assert!(f.mark_scale.abs() < f32::EPSILON);
        assert!(f.bloom_opacity.abs() < f32::EPSILON);
        assert!(f.glyph.opacity.abs() < f32::EPSILON);
        assert!(Frame::letter(0, 0, false).opacity.abs() < f32::EPSILON);
    }

    #[test]
    fn the_mark_overshoots_once_by_five_to_twelve_percent_and_settles() {
        let scales: Vec<f32> = (0..=MIN_SHOW_MS)
            .map(|t| Frame::at(t, false).mark_scale)
            .collect();
        let peak = scales.iter().copied().fold(0.0_f32, f32::max);
        assert!((1.05..=1.12).contains(&peak), "peak {peak}");
        // After the peak the scale comes back and stays within 1 % of rest.
        let settled = Frame::at(MARK_AT + 500, false).mark_scale;
        assert!((settled - 1.0).abs() < 0.01, "settled {settled}");
        assert!((Frame::at(MIN_SHOW_MS, false).mark_scale - 1.0).abs() < 0.005);
    }

    fn near(a: Reveal, b: Reveal) -> bool {
        (a.opacity - b.opacity).abs() < 1e-3 && (a.rise_px - b.rise_px).abs() < 0.01
    }

    #[test]
    fn letters_start_thirty_five_milliseconds_apart_and_rise_eight_pixels() {
        let first = Frame::letter(0, WORD_AT, false);
        assert!(first.opacity.abs() < f32::EPSILON);
        assert!((first.rise_px - 8.0).abs() < f32::EPSILON);
        // The second letter has not started when the first is 35 ms in, and is a step behind.
        let (a, b) = (
            Frame::letter(0, WORD_AT + 70, false),
            Frame::letter(1, WORD_AT + 70, false),
        );
        assert!(a.opacity > b.opacity && b.opacity > 0.0);
        assert!(
            Frame::letter(1, WORD_AT + LETTER_STEP_MS, false)
                .opacity
                .abs()
                < f32::EPSILON
        );
        let last = Frame::letter(WORDMARK.len() - 1, MIN_SHOW_MS, false);
        assert!(near(last, Reveal::REST), "{last:?}");
    }

    #[test]
    fn everything_is_in_place_before_the_floor_ends() {
        assert!(Frame::settled_at() < MIN_SHOW_MS);
        let f = Frame::at(Frame::settled_at(), false);
        assert!(near(f.glyph, Reveal::REST), "{:?}", f.glyph);
        assert!(near(f.tagline, Reveal::REST), "{:?}", f.tagline);
        assert!((f.rail_opacity - 1.0).abs() < 1e-3);
    }

    #[test]
    fn nothing_moves_further_than_sixteen_pixels() {
        for t in (0..MIN_SHOW_MS).step_by(10) {
            let f = Frame::at(t, false);
            assert!(f.glyph.rise_px <= 16.0 && f.tagline.rise_px <= 16.0);
            assert!(Frame::letter(0, t, false).rise_px <= 16.0);
        }
    }

    #[test]
    fn the_bloom_breathes_in_the_heartbeat_cycle_and_never_vanishes() {
        let start = BREATH_AT;
        let (lo, hi) = (
            Frame::at(start, false).bloom_opacity,
            Frame::at(start + u64::from(HEART_CYCLE_MS) / 2, false).bloom_opacity,
        );
        assert!(hi > lo, "{lo} to {hi}");
        let back = Frame::at(start + u64::from(HEART_CYCLE_MS), false).bloom_opacity;
        assert!(
            (back - lo).abs() < 0.01,
            "a full cycle returns: {back} vs {lo}"
        );
        assert!(lo > 0.7);
    }

    #[test]
    fn reduced_motion_is_the_resting_frame_at_every_moment() {
        for t in [0, 100, 700, 5_000] {
            assert_eq!(Frame::at(t, true), Frame::REST);
            assert_eq!(Frame::letter(3, t, true), Reveal::REST);
        }
    }

    #[test]
    fn the_bloom_texture_is_brightest_in_the_middle_and_clear_at_the_edge() {
        let c = Rgba::from_rgb_hex(0xF0_90_B0);
        let px = bloom_rgba(c, 0.4);
        let side = BLOOM_PX as usize;
        assert_eq!(px.len(), side * side * 4);
        let alpha = |x: usize, y: usize| px[(y * side + x) * 4 + 3];
        assert!(alpha(side / 2, side / 2) > 90);
        assert_eq!(alpha(0, 0), 0);
        assert!(alpha(side / 2, side / 2) > alpha(side / 4, side / 2));
    }

    /// The farthest corner of the tagline lies about 170 px from the mark's centre, inside the
    /// bloom: the ink keeps 4.5:1 there in every base mode, at the bloom's brightest.
    #[test]
    fn the_wordmark_and_tagline_stay_legible_over_the_bloom() {
        let d = 170.0 / (BLOOM_SIZE / 2.0);
        let alpha = BLOOM_PEAK * (1.0 - d * d) * (1.0 - d * d);
        for base in [BaseMode::Dark, BaseMode::Amoled, BaseMode::Light] {
            let n = base.neutrals();
            let accent = accent_palette(ThemeChoice::Dynamic, base, AccentIntensity::Vivid, None);
            let under = accent.primary.with_alpha(alpha).over(n.surface);
            for ink in [n.on_surface, n.on_surface_variant] {
                let ratio = contrast_ratio(ink, under);
                assert!(ratio >= 4.5, "{base:?}: {ratio:.2}");
            }
        }
    }
}
