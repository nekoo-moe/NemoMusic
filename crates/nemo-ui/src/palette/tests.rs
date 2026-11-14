//! Unit tests of the palette pipeline.

use super::*;
use image::{ImageFormat, Rgba as ImgRgba, RgbaImage};
use std::io::Cursor;

fn png(img: &RgbaImage) -> Vec<u8> {
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .expect("invariant: encoding a small RGBA image cannot fail");
    out
}

fn solid(r: u8, g: u8, b: u8) -> Vec<u8> {
    png(&RgbaImage::from_pixel(96, 96, ImgRgba([r, g, b, 255])))
}

/// Deterministic noise, no `rand` dependency.
fn noise() -> Vec<u8> {
    let mut state = 0x1234_5678_u32;
    let mut img = RgbaImage::new(96, 96);
    for p in img.pixels_mut() {
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state.to_be_bytes()[0]
        };
        *p = ImgRgba([next(), next(), next(), 255]);
    }
    png(&img)
}

fn accent_roles(p: &Palette) -> [Rgba; 6] {
    [
        p.primary,
        p.primary_container,
        p.secondary_container,
        p.tertiary,
        p.on_primary,
        p.surface_tint,
    ]
}

fn assert_good(name: &str, p: &Palette) {
    for (pair, fg, bg) in p.text_pairs() {
        let ratio = contrast_ratio(fg, bg);
        assert!(ratio >= TEXT_CONTRAST, "{name}: {pair} is {ratio:.2}:1");
    }
    assert!(
        contrast_ratio(p.primary, color::SURFACE) >= UI_CONTRAST,
        "{name}: primary non-text"
    );
    for r in accent_roles(p) {
        let c = rgb_to_oklch(r).c;
        assert!(c <= MAX_CHROMA + 0.01, "{name}: chroma {c:.3}");
    }
}

#[test]
fn oklch_round_trips() {
    for hex in [
        0x80_80_80, 0xFF_00_00, 0x00_FF_00, 0x33_66_CC, 0xFF_FF_FF, 0x00_00_00,
    ] {
        let c = Rgba::from_rgb_hex(hex);
        let back = oklch_to_rgb(rgb_to_oklch(c));
        for (a, b) in [(c.r, back.r), (c.g, back.g), (c.b, back.b)] {
            assert!(a.abs_diff(b) <= 2, "{hex:06X}: {a} vs {b}");
        }
    }
}

#[test]
fn all_black_and_all_white_and_grey_give_neutral_passing_palettes() {
    for (name, bytes) in [
        ("black", solid(0, 0, 0)),
        ("white", solid(255, 255, 255)),
        ("grey", solid(128, 128, 128)),
    ] {
        let p = from_image_bytes(&bytes).expect("decodes");
        assert_good(name, &p);
        assert!(!p.is_fallback, "{name} should not need the fallback");
        assert!(rgb_to_oklch(p.primary).c < 0.01, "{name} accent is neutral");
    }
}

#[test]
fn saturated_covers_keep_their_hue() {
    // Oklch hues: red about 29, green about 142, blue about 264.
    for (name, rgb, hue) in [
        ("red", (230, 20, 30), 29.0),
        ("green", (20, 200, 60), 142.0),
        ("blue", (20, 40, 230), 264.0),
    ] {
        let p = from_image_bytes(&solid(rgb.0, rgb.1, rgb.2)).expect("decodes");
        assert_good(name, &p);
        let lch = rgb_to_oklch(p.primary);
        let diff = (lch.h - hue + 540.0).rem_euclid(360.0) - 180.0;
        assert!(diff.abs() < 15.0, "{name}: hue {:.0} vs {hue}", lch.h);
        assert!(lch.c > 0.05, "{name}: accent should be coloured");
    }
}

#[test]
fn noise_gives_a_passing_palette() {
    let p = from_image_bytes(&noise()).expect("decodes");
    assert_good("noise", &p);
}

#[test]
fn a_small_vivid_area_beats_a_large_grey_one() {
    let mut img = RgbaImage::from_pixel(64, 64, ImgRgba([120, 120, 120, 255]));
    for y in 0..16 {
        for x in 0..16 {
            img.put_pixel(x, y, ImgRgba([20, 60, 230, 255]));
        }
    }
    let p = from_image_bytes(&png(&img)).expect("decodes");
    let h = rgb_to_oklch(p.primary).h;
    assert!((h - 264.0).abs() < 20.0, "hue {h}");
}

#[test]
fn large_images_are_shrunk_and_transparent_pixels_ignored() {
    let mut img = RgbaImage::from_pixel(400, 300, ImgRgba([0, 0, 0, 0]));
    for p in img.pixels_mut().take(30_000) {
        *p = ImgRgba([230, 20, 30, 255]);
    }
    let p = from_image_bytes(&png(&img)).expect("decodes");
    assert_good("transparent", &p);
}

#[test]
fn bad_input_is_an_error_not_a_panic() {
    assert_eq!(from_image_bytes(b"not an image"), Err(PaletteError::Decode));
    assert_eq!(from_rgba(2, 2, &[0; 3]), Err(PaletteError::BadBuffer));
    assert!(from_rgba(0, 0, &[]).is_ok());
}

#[test]
fn fallback_palette_passes_and_uses_the_brief_primary() {
    let f = Palette::fallback();
    assert_good("fallback", &f);
    assert_eq!(f.primary, color::PRIMARY_FALLBACK);
    assert!(f.is_fallback);
}

#[test]
fn one_step_tone_search_repairs_a_slightly_low_tone() {
    let seed = Seed {
        hue: 264.0,
        chroma: 0.12,
    };
    // Tone 50 on the surface is below 4.5:1; the search moves it up within 10 steps.
    let bad = Tones {
        primary: 50.0,
        ..Tones::DARK
    };
    let broken = render(seed, &bad);
    assert!(contrast_ratio(broken.primary, color::SURFACE) < TEXT_CONTRAST);
    let fixed = build_palette(seed, bad);
    assert!(fixed.passes());
    assert!(
        !fixed.is_fallback,
        "a small miss should not need the fallback"
    );
}

#[test]
fn a_hopeless_tone_uses_the_fallback() {
    let seed = Seed {
        hue: 264.0,
        chroma: 0.12,
    };
    let hopeless = Tones {
        on_primary: 80.0,
        primary: 80.0,
        primary_container: 85.0,
        on_primary_container: 82.0,
        ..Tones::DARK
    };
    // on-primary (tone 80) over primary (tone 80) is 1:1 and 10 steps cannot fix it.
    let p = build_palette(seed, hopeless);
    assert!(p.is_fallback);
    assert!(p.passes());
}

#[test]
fn mix_endpoints_and_midpoints_stay_legible() {
    let red = from_image_bytes(&solid(230, 20, 30)).expect("decodes");
    let blue = from_image_bytes(&solid(20, 40, 230)).expect("decodes");
    assert_eq!(red.mix(&blue, 0.0), red);
    assert_eq!(red.mix(&blue, 1.0), blue);
    for i in 0..=20_u8 {
        let m = red.mix(&blue, f32::from(i) / 20.0);
        assert!(m.passes(), "step {i}: worst {:.2}", m.worst_contrast());
    }
}
