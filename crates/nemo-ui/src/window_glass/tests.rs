use super::*;
use crate::palette::Seed;
use crate::scheme::{BaseMode, PANE_ALPHA, bg_colors, pane_tint};
use crate::theme::{Rgba, contrast_ratio};

fn glass(enabled: bool, reduce: bool, created: bool) -> WindowGlass {
    WindowGlass {
        enabled,
        opacity: OPACITY_DEFAULT,
        reduce_transparency: reduce,
        created_transparent: created,
    }
}

#[test]
fn the_opacity_is_limited_to_40_to_95() {
    assert_eq!(clamp_opacity(0), 40);
    assert_eq!(clamp_opacity(39), 40);
    assert_eq!(clamp_opacity(40), 40);
    assert_eq!(clamp_opacity(80), 80);
    assert_eq!(clamp_opacity(95), 95);
    assert_eq!(clamp_opacity(96), 95);
    assert_eq!(clamp_opacity(255), 95);
    assert_eq!(OPACITY_DEFAULT, 80);
}

#[test]
fn the_state_follows_the_switch_the_window_and_reduce_transparency() {
    assert_eq!(glass(false, false, false).state(), GlassState::Off);
    assert_eq!(glass(false, false, true).state(), GlassState::Off);
    assert_eq!(glass(false, true, true).state(), GlassState::Off);
    assert_eq!(glass(true, false, true).state(), GlassState::Clear);
    assert_eq!(glass(true, false, false).state(), GlassState::NeedsRestart);
    assert_eq!(glass(true, true, true).state(), GlassState::Overridden);
    assert_eq!(glass(true, true, false).state(), GlassState::Overridden);
}

#[test]
fn only_a_window_created_solid_needs_a_restart() {
    assert!(glass(true, false, false).needs_restart());
    assert!(!glass(true, false, true).needs_restart());
    assert!(!glass(false, false, false).needs_restart());
    assert!(!glass(true, true, false).needs_restart());
    // Turning it off in a see-through window applies at once.
    assert!(!glass(false, false, true).is_clear());
}

#[test]
fn solid_states_draw_everything_at_full_alpha() {
    for g in [
        glass(false, false, true),
        glass(true, true, true),
        glass(true, false, false),
    ] {
        assert!(
            (g.backdrop_alpha(false) - 1.0).abs() < f32::EPSILON,
            "{g:?}"
        );
        assert!((g.pane_alpha(PANE_ALPHA) - PANE_ALPHA).abs() < f32::EPSILON);
    }
}

#[test]
fn a_clear_window_draws_the_backdrop_at_the_opacity_and_the_panes_at_the_floor() {
    let g = WindowGlass {
        opacity: 60,
        ..glass(true, false, true)
    };
    assert!((g.backdrop_alpha(false) - 0.60).abs() < 1e-6);
    assert!((g.pane_alpha(PANE_ALPHA) - PANE_FLOOR).abs() < f32::EPSILON);
    assert!(
        (g.pane_alpha(0.95) - 0.95).abs() < f32::EPSILON,
        "never lowered"
    );
    // The full-screen Now Playing stays solid.
    assert!((g.backdrop_alpha(true) - 1.0).abs() < f32::EPSILON);
}

#[test]
fn a_hand_edited_opacity_is_clamped_when_it_is_read() {
    let low = WindowGlass {
        opacity: 3,
        ..glass(true, false, true)
    };
    let high = WindowGlass {
        opacity: 200,
        ..glass(true, false, true)
    };
    assert!((low.backdrop_alpha(false) - 0.40).abs() < 1e-6);
    assert!((high.backdrop_alpha(false) - 0.95).abs() < 1e-6);
}

#[test]
fn the_status_sentence_names_the_state() {
    assert_eq!(glass(false, false, false).status(), "This window is solid.");
    assert_eq!(
        glass(true, false, true).status(),
        "This window is see-through at 80 percent."
    );
    assert!(
        glass(true, true, true)
            .status()
            .contains("Reduce transparency")
    );
    assert!(glass(true, false, false).status().contains("restart"));
}

/// Desktops behind the window: white, black, saturated colours and a noisy mix.
fn desktops() -> Vec<Rgba> {
    let mut out = vec![
        Rgba::from_rgb_hex(0xFF_FFFF),
        Rgba::from_rgb_hex(0x00_0000),
        Rgba::from_rgb_hex(0xFF_0000),
        Rgba::from_rgb_hex(0x00_FF00),
        Rgba::from_rgb_hex(0x00_00FF),
        Rgba::from_rgb_hex(0xFF_FF00),
        Rgba::from_rgb_hex(0x00_FFFF),
        Rgba::from_rgb_hex(0xFF_00FF),
    ];
    // Noise: a small linear congruential generator, so the fixture never changes.
    let mut x: u32 = 0x2545_F491;
    for _ in 0..64 {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let [_, r, g, b] = x.to_be_bytes();
        out.push(Rgba { r, g, b, a: 255 });
    }
    out
}

/// The worst contrast of both inks over a pane at `pane_alpha`, over a backdrop of colour
/// `field` at `opacity` percent, over every desktop.
fn worst_contrast(base: BaseMode, tint: Rgba, field: Rgba, opacity: u8) -> f64 {
    let n = base.neutrals();
    let mut worst = f64::MAX;
    for desk in desktops() {
        let backdrop = field.with_alpha(f32::from(opacity) / 100.0).over(desk);
        let pane = tint.with_alpha(PANE_FLOOR).over(backdrop);
        for ink in [n.on_surface, n.on_surface_variant] {
            worst = worst.min(contrast_ratio(ink, pane));
        }
    }
    worst
}

#[test]
fn both_text_inks_keep_4_5_to_1_on_a_pane_over_any_desktop() {
    for base in BaseMode::ALL {
        let n = base.neutrals();
        for field in bg_colors(None, base) {
            for opacity in [OPACITY_MIN, 60, OPACITY_DEFAULT, OPACITY_MAX] {
                let worst = worst_contrast(base, n.container, field, opacity);
                assert!(
                    worst >= 4.5,
                    "{base:?} field {field:?} at {opacity}: {worst:.2}"
                );
            }
        }
    }
}

#[test]
fn a_song_tinted_pane_keeps_4_5_to_1_over_any_desktop() {
    for base in BaseMode::ALL {
        for hue in [0.0, 60.0, 120.0, 180.0, 240.0, 300.0] {
            let tint = pane_tint(Seed { hue, chroma: 0.2 }, base);
            let field = bg_colors(Some(Seed { hue, chroma: 0.2 }), base)[1];
            let worst = worst_contrast(base, tint, field, OPACITY_MIN);
            assert!(worst >= 4.5, "{base:?} hue {hue}: {worst:.2}");
        }
    }
}

#[test]
fn the_floor_is_what_keeps_the_contrast() {
    // The old pane alpha is not enough over a bright desktop with the lowest opacity: this is
    // why the panes are raised to the floor while the glass is clear.
    let base = BaseMode::Dark;
    let n = base.neutrals();
    let field = bg_colors(None, base)[1];
    let white = Rgba::from_rgb_hex(0xFF_FFFF);
    let backdrop = field.with_alpha(0.40).over(white);
    let at_floor = n.container.with_alpha(PANE_FLOOR).over(backdrop);
    let at_old = n.container.with_alpha(PANE_ALPHA).over(backdrop);
    assert!(
        contrast_ratio(n.on_surface_variant, at_floor)
            > contrast_ratio(n.on_surface_variant, at_old)
    );
}
