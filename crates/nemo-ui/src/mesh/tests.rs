//! Tests of the colour mesh: which colours it picks and the contrast of every ink over it.

use super::*;
use crate::canvas::text_inks;
use crate::drift::{VIGNETTE_ALPHA, VIGNETTE_MID_ALPHA};
use crate::palette::{MAX_CHROMA, Seed, mix_rgb};
use crate::scheme::{PANE_ALPHA, pane_tint};
use crate::theme::contrast_ratio;

/// Opacity of the white lift at the peak of the heartbeat (song-colour-theme design).
const PEAK_LIFT: f32 = 0.05;
const PX: usize = 24;

fn solid(c: [u8; 3]) -> Vec<u8> {
    (0..PX * PX).flat_map(|_| [c[0], c[1], c[2], 255]).collect()
}

/// Equal vertical stripes, one per colour.
fn stripes(colors: &[[u8; 3]]) -> Vec<u8> {
    (0..PX * PX)
        .flat_map(|i| {
            let c = colors[(i % PX) * colors.len() / PX];
            [c[0], c[1], c[2], 255]
        })
        .collect()
}

/// The fixture covers of `design.md` section 2: all-black, all-white, grey, saturated, yellow,
/// noisy.
fn fixtures() -> Vec<(&'static str, Vec<u8>)> {
    let noisy: Vec<u8> = (0..PX * PX)
        .flat_map(|i| {
            let v = (i as u32).wrapping_mul(2_654_435_761);
            // Low bytes of a hash: pseudo-random channels.
            #[allow(clippy::cast_possible_truncation)]
            [v as u8, (v >> 8) as u8, (v >> 16) as u8, 255]
        })
        .collect();
    vec![
        ("black", solid([0, 0, 0])),
        ("white", solid([255, 255, 255])),
        ("grey", solid([128, 128, 128])),
        ("saturated", solid([255, 20, 140])),
        ("yellow", solid([250, 230, 0])),
        ("noisy", noisy),
        (
            "rgb",
            stripes(&[[220, 30, 40], [30, 180, 60], [40, 60, 220]]),
        ),
        (
            "four",
            stripes(&[[220, 30, 40], [240, 220, 20], [20, 170, 200], [40, 60, 220]]),
        ),
    ]
}

#[test]
fn a_cover_of_several_hues_gives_fields_with_distinct_hues() {
    let three = picks(&stripes(&[[220, 30, 40], [30, 180, 60], [40, 60, 220]]));
    let four = picks(&stripes(&[
        [220, 30, 40],
        [240, 220, 20],
        [20, 170, 200],
        [40, 60, 220],
    ]));
    assert_eq!((three.len(), four.len()), (3, 4));
    for found in [three, four] {
        for (i, a) in found.iter().enumerate() {
            for b in &found[i + 1..] {
                assert!(hue_gap(a.h, b.h) >= MIN_HUE_GAP, "{a:?} {b:?}");
            }
        }
    }
}

#[test]
fn two_close_hues_take_one_slot_and_a_single_hue_is_padded() {
    // Red and orange are 40 degrees or less apart in the picks: one of them wins, the rest pad.
    let close = picks(&stripes(&[[220, 30, 40], [225, 60, 30]]));
    assert_eq!(close.len(), MIN_FIELDS);
    let one = picks(&solid([255, 20, 140]));
    assert_eq!(one.len(), MIN_FIELDS);
    assert!(one.iter().all(|p| p.c > 0.1), "{one:?}");
    assert!(hue_gap(one[0].h, one[1].h) > 20.0, "variants are turned");
}

#[test]
fn a_grey_cover_gives_greys() {
    for rgba in [
        solid([128, 128, 128]),
        solid([0, 0, 0]),
        solid([255, 255, 255]),
    ] {
        let found = picks(&rgba);
        assert_eq!(found.len(), MIN_FIELDS);
        assert!(found.iter().all(|p| p.c == 0.0), "{found:?}");
        let m = build(&rgba);
        for base in MODES {
            for c in m.used(base) {
                let spread = c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b);
                assert!(spread <= 2, "{base:?}: {c:?} is not grey");
            }
        }
    }
}

#[test]
fn the_background_is_richer_than_the_accent_cap_and_follows_the_cover() {
    let m = build(&solid([255, 20, 140]));
    let richest = m
        .used(BaseMode::Dark)
        .iter()
        .map(|c| rgb_to_oklch(*c).c)
        .fold(0.0, f64::max);
    assert!(richest > MAX_CHROMA - 0.02, "chroma {richest:.3}");
    let o = rgb_to_oklch(m.used(BaseMode::Dark)[0]);
    assert!(hue_gap(o.h, rgb_to_oklch(Rgba::from_rgb_hex(0xFF_148C)).h) < 12.0);
    // Another cover gives other colours.
    assert_ne!(
        m.used(BaseMode::Dark),
        build(&solid([20, 160, 255])).used(BaseMode::Dark)
    );
}

#[test]
fn textures_are_square_soft_and_empty_when_unused() {
    let m = build(&solid([255, 20, 140]));
    assert_eq!(m.n, MIN_FIELDS);
    let side = BLOB_PX as usize;
    let t = m.texture(BaseMode::Dark, 0);
    assert_eq!(t.len(), side * side * 4);
    let alpha = |x: usize, y: usize| t[(y * side + x) * 4 + 3];
    assert!(alpha(side / 2, side / 2) >= 200, "solid in the middle");
    assert_eq!(alpha(0, 0), 0, "transparent at the corner");
    assert!(alpha(side / 2, 2) < alpha(side / 2, side / 4), "fades out");
    assert!(m.texture(BaseMode::Dark, FIELDS - 1).is_empty());
}

/// Every pane tint the song can give: any hue at the largest seed chroma.
fn tints(base: BaseMode) -> Vec<Rgba> {
    (0..360)
        .step_by(30)
        .map(|h| {
            let seed = Seed {
                hue: f64::from(h),
                chroma: MAX_CHROMA,
            };
            pane_tint(seed, base)
        })
        .collect()
}

/// The field colours the window can show: each colour, the flat colour under them, and every
/// 50 % mix of two (fields overlap while they drift; a mix in gamma space can be darker than
/// both colours).
fn drawn(m: &Mesh, base: BaseMode) -> Vec<Rgba> {
    let used = m.used(base);
    let mut out = used.to_vec();
    out.push(m.under(base));
    for (i, a) in used.iter().enumerate() {
        for b in &used[i + 1..] {
            out.push(mix_rgb(*a, *b, 0.5));
        }
    }
    out
}

#[test]
fn every_ink_reaches_4_5_over_the_panes_and_the_two_main_inks_over_the_bare_field() {
    let white = Rgba::from_rgb_hex(0xFF_FFFF);
    for (name, rgba) in fixtures() {
        let m = build(&rgba);
        for base in MODES {
            let n = base.neutrals();
            let inks = text_inks(&n, base);
            for field in drawn(&m, base) {
                for lift in [0.0, PEAK_LIFT] {
                    for v in [0.0, VIGNETTE_MID_ALPHA, VIGNETTE_ALPHA] {
                        let lifted = white.with_alpha(lift).over(field);
                        let bare = n.lowest.with_alpha(v).over(lifted);
                        // The two main inks can sit on the bare field (the gaps between panes).
                        for (ink, fg) in &inks[..2] {
                            let r = contrast_ratio(fg.over(bare), bare);
                            assert!(r >= 4.5, "{name} {base:?} {ink} bare {r:.2}");
                        }
                        for tint in tints(base) {
                            let pane = tint.with_alpha(PANE_ALPHA).over(bare);
                            for (ink, fg) in &inks {
                                let r = contrast_ratio(fg.over(pane), pane);
                                assert!(r >= 4.5, "{name} {base:?} {ink} pane {r:.2}");
                            }
                        }
                    }
                }
            }
        }
    }
}
