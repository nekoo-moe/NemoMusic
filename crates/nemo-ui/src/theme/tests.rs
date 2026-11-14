//! Unit tests of the design tokens.

use super::*;

#[test]
fn contrast_known_values() {
    let white = Rgba::from_rgb_hex(0xFF_FFFF);
    let black = Rgba::from_rgb_hex(0x00_0000);
    assert!((contrast_ratio(white, black) - 21.0).abs() < 1e-9);
    assert!((contrast_ratio(white, white) - 1.0).abs() < 1e-9);
    // Symmetric.
    assert!((contrast_ratio(black, white) - contrast_ratio(white, black)).abs() < 1e-12);
}

#[test]
fn design_pairs_meet_aa_over_surface() {
    let pairs = [
        ("on-surface", color::ON_SURFACE),
        ("on-surface-variant", color::ON_SURFACE_VARIANT),
        ("success", color::SUCCESS),
        ("warning", color::WARNING),
        ("error", color::ERROR),
    ];
    for (name, fg) in pairs {
        let ratio = contrast_ratio(fg, color::SURFACE);
        assert!(ratio >= 4.5, "{name} over surface is {ratio:.2}:1");
    }
}

#[test]
fn info_and_fallback_primary_meet_aa_over_surface() {
    for fg in [color::INFO, color::PRIMARY_FALLBACK] {
        assert!(contrast_ratio(fg, color::SURFACE) >= 4.5);
    }
}

#[test]
fn spacing_is_multiple_of_four() {
    for v in spacing::SCALE {
        assert_eq!(v % spacing::UNIT, 0, "{v} is not a multiple of 4");
    }
    for v in [
        spacing::XS,
        spacing::SM,
        spacing::MD,
        spacing::LG,
        spacing::XL,
    ] {
        assert!(spacing::SCALE.contains(&v));
    }
}

#[test]
fn glass_tint_over_surface_keeps_text_legible() {
    let tint = glass::TINT_BASE.with_alpha(glass::TINT_ALPHA);
    let bar = tint.over(color::SURFACE);
    assert!(contrast_ratio(color::ON_SURFACE, bar) >= 4.5);
}

#[test]
fn alpha_round_trip() {
    assert_eq!(Rgba::from_rgb_hex(0).with_alpha(0.55).a, 140);
    assert_eq!(Rgba::from_rgb_hex(0).with_alpha(2.0).a, 255);
    assert_eq!(color::OUTLINE_VARIANT.a, 0x14);
}
