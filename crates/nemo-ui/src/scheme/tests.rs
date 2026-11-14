//! Unit tests of the themes: contrast of every palette, fades and corner scales.

use crate::theme::Neutrals;

use super::*;
use crate::theme::contrast_ratio;

fn all_palettes() -> Vec<(String, BaseMode, Palette)> {
    let mut out = Vec::new();
    for base in BaseMode::ALL {
        for choice in ThemeChoice::ALL {
            for intensity in AccentIntensity::ALL {
                // A few covers for Dynamic: saturated, dull, grey.
                let covers = if choice == ThemeChoice::Dynamic {
                    vec![
                        None,
                        Some(Seed {
                            hue: 25.0,
                            chroma: 0.14,
                        }),
                        Some(Seed {
                            hue: 145.0,
                            chroma: 0.06,
                        }),
                        Some(Seed {
                            hue: 265.0,
                            chroma: 0.0,
                        }),
                        Some(Seed {
                            hue: 95.0,
                            chroma: 0.14,
                        }),
                    ]
                } else {
                    vec![None]
                };
                for cover in covers {
                    out.push((
                        format!("{choice:?}/{base:?}/{intensity:?}/{cover:?}"),
                        base,
                        accent_palette(choice, base, intensity, cover),
                    ));
                }
            }
        }
    }
    out
}

#[test]
fn every_palette_keeps_text_contrast_on_its_surfaces() {
    for (name, base, p) in all_palettes() {
        let n = base.neutrals();
        for (pair, fg, bg) in p.text_pairs_on(&n) {
            let ratio = contrast_ratio(fg, bg);
            assert!(ratio >= 4.5, "{name}: {pair} is {ratio:.2}:1");
        }
        assert!(
            contrast_ratio(p.primary, n.surface) >= 3.0,
            "{name}: primary is not a 3:1 non-text colour"
        );
    }
}

#[test]
fn neutral_text_reaches_4_5_on_every_surface_of_every_mode() {
    for base in BaseMode::ALL {
        let n = base.neutrals();
        for (name, fg, bg) in n.text_pairs() {
            let ratio = contrast_ratio(fg, bg);
            assert!(ratio >= 4.5, "{base:?}: {name} is {ratio:.2}:1");
        }
    }
}

#[test]
fn text_on_accent_containers_is_legible() {
    for (name, _, p) in all_palettes() {
        assert!(
            contrast_ratio(p.on_primary, p.primary) >= 4.5,
            "{name}: on-primary"
        );
        assert!(
            contrast_ratio(p.on_secondary_container, p.secondary_container) >= 4.5,
            "{name}: on-secondary-container"
        );
    }
}

#[test]
fn curated_seeds_give_distinct_accents() {
    let primaries: Vec<Rgba> = ThemeChoice::ALL
        .iter()
        .skip(1)
        .map(|c| accent_palette(*c, BaseMode::Dark, AccentIntensity::Standard, None).primary)
        .collect();
    for (i, a) in primaries.iter().enumerate() {
        for b in primaries.iter().skip(i + 1) {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn dynamic_without_a_cover_is_the_fallback() {
    assert_eq!(
        accent_palette(
            ThemeChoice::Dynamic,
            BaseMode::Dark,
            AccentIntensity::Standard,
            None
        ),
        Palette::fallback()
    );
}

#[test]
fn vivid_is_more_saturated_than_soft() {
    let chroma = |i| {
        let p = accent_palette(ThemeChoice::Sunset, BaseMode::Dark, i, None);
        let (_, a, b) = crate::palette::rgb_to_oklab(p.primary_container);
        a.hypot(b)
    };
    assert!(chroma(AccentIntensity::Vivid) > chroma(AccentIntensity::Soft));
}

#[test]
fn scheme_fade_runs_400_ms_between_modes() {
    let mut f = SchemeFade::settled(Neutrals::DARK);
    assert!(f.is_done());
    f.retarget(Neutrals::LIGHT, false);
    assert!(!f.is_done());
    f.advance(100.0);
    let mid = f.current();
    assert!(mid.surface != Neutrals::DARK.surface && mid.surface != Neutrals::LIGHT.surface);
    f.advance(400.0);
    assert!(f.is_done());
    assert_eq!(f.current(), Neutrals::LIGHT);
    // Reduced motion swaps at once.
    f.retarget(Neutrals::AMOLED, true);
    assert!(f.is_done() && f.current() == Neutrals::AMOLED);
}

#[test]
fn background_field_keeps_text_legible_for_any_seed_and_mode() {
    let seeds: Vec<Option<Seed>> = std::iter::once(None)
        .chain((0..36).flat_map(|h| {
            [0.0, 0.06, 0.14].into_iter().map(move |c| {
                Some(Seed {
                    hue: f64::from(h) * 10.0,
                    chroma: c,
                })
            })
        }))
        .collect();
    for base in BaseMode::ALL {
        let n = base.neutrals();
        for seed in &seeds {
            for (k, bg) in bg_colors(*seed, base).into_iter().enumerate() {
                // Text straight on the field, and on a pane over the field.
                let pane = n.container.with_alpha(PANE_ALPHA).over(bg);
                for (name, fg, under) in [
                    ("on-surface/field", n.on_surface, bg),
                    ("variant/field", n.on_surface_variant, bg),
                    ("on-surface/pane", n.on_surface, pane),
                    ("variant/pane", n.on_surface_variant, pane),
                ] {
                    let ratio = contrast_ratio(fg, under);
                    assert!(
                        ratio >= 4.5,
                        "{base:?} {seed:?} stop {k} {name}: {ratio:.2}"
                    );
                }
            }
        }
    }
}

#[test]
fn background_fade_takes_under_a_second_and_snaps_when_reduced() {
    let a = bg_colors(None, BaseMode::Dark);
    let b = bg_colors(
        Some(Seed {
            hue: 20.0,
            chroma: 0.12,
        }),
        BaseMode::Dark,
    );
    let mut f = BgFade::settled(a);
    f.retarget(b, false);
    assert!(!f.is_done());
    let mut ms = 0.0;
    while !f.is_done() && ms < 3000.0 {
        f.advance(16.0);
        ms += 16.0;
    }
    assert!((300.0..1100.0).contains(&ms), "settled after {ms} ms");
    assert_eq!(f.current(), b);
    f.retarget(a, true);
    assert!(f.is_done() && f.current() == a);
}

#[test]
fn corner_scales_are_ordered() {
    assert!(CornerStyle::Sharp.percent() < CornerStyle::Soft.percent());
    assert!(CornerStyle::Soft.percent() < CornerStyle::Round.percent());
    assert_eq!(CornerStyle::Soft.percent(), 100);
}
