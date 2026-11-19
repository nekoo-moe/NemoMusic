//! The capsule's surface (owner, 2026-11-19; bar-material): a Material 3 tonal surface. The fill
//! is `surface-container-high` at 0.90 alpha in dark and AMOLED and 0.92 in light, so the window
//! backdrop shows through faintly. There is no outline, no rim, no glint and no inner highlight:
//! depth comes from the tone and the level 3 shadow (`design.md` section 5). Reduce
//! transparency, and a see-through window (Window glass), draw the fill fully opaque. The
//! `glass_tier` setting stays so old config files load; both of its values draw this surface.

use iced::widget::{Space, container};
use iced::{Border, Element, Length};

use super::px;
use crate::gui::app::{App, Message};
use crate::gui::style::{col, shadow};
use crate::layout::PLAYER_BAR_HEIGHT;
use crate::theme::{Rgba, color, glass, shape};

/// `true` when the capsule draws fully opaque: Reduce transparency is on, or the window itself is
/// see-through (the bar stays solid there, `design.md` section 6).
#[must_use]
pub fn solid(app: &App) -> bool {
    app.ui.settings.reduce_transparency || app.window_glass().is_clear()
}

/// Alpha of the capsule fill: 1 when `solid`, else [`glass::BAR_ALPHA`] (dark and AMOLED) or
/// [`glass::BAR_ALPHA_LIGHT`].
#[must_use]
pub fn fill_alpha(light: bool, solid: bool) -> f32 {
    if solid {
        1.0
    } else if light {
        glass::BAR_ALPHA_LIGHT
    } else {
        glass::BAR_ALPHA
    }
}

/// The fill of the capsule now: `surface-container-high` at [`fill_alpha`].
#[must_use]
pub fn fill(app: &App) -> Rgba {
    let alpha = fill_alpha(app.ui.settings.base.is_light(), solid(app));
    color::surface_container_high().with_alpha(alpha)
}

/// The capsule's container style for `fill`: a flat colour, rounded corners, the level 3 shadow,
/// and no border.
fn style(fill: Rgba) -> container::Style {
    container::Style {
        background: Some(col(fill).into()),
        border: Border {
            radius: shape::xl().into(),
            ..Border::default()
        },
        shadow: shadow(
            glass::ELEVATED_OFFSET_Y,
            glass::ELEVATED_BLUR,
            glass::SHADOW,
        ),
        ..container::Style::default()
    }
}

/// The capsule body: fill and shadow, `bar_w` px wide.
pub(super) fn body<'a>(app: &App, bar_w: f32) -> Element<'a, Message> {
    let fill = fill(app);
    container(Space::new())
        .width(Length::Fixed(bar_w))
        .height(Length::Fixed(px(PLAYER_BAR_HEIGHT)))
        .style(move |_| style(fill))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::MODES;
    use crate::drift::{VIGNETTE_ALPHA, VIGNETTE_MID_ALPHA};
    use crate::mesh::{self, BLOB_PEAK, Mesh};
    use crate::palette::{Seed, mix_rgb};
    use crate::scheme::{BaseMode, bg_colors, window_neutrals};
    use crate::theme::{Neutrals, contrast_ratio};

    fn solid(c: [u8; 3]) -> Vec<u8> {
        (0..24 * 24).flat_map(|_| [c[0], c[1], c[2], 255]).collect()
    }

    fn stripes(colors: &[[u8; 3]]) -> Vec<u8> {
        (0..24 * 24)
            .flat_map(|i| {
                let c = colors[(i % 24) * colors.len() / 24];
                [c[0], c[1], c[2], 255]
            })
            .collect()
    }

    /// Covers that stress the backdrop under the bar: black, white, grey, saturated, yellow,
    /// noisy, three hues.
    fn covers() -> Vec<(&'static str, Vec<u8>)> {
        let noisy = (0..24 * 24_u32)
            .flat_map(|i| {
                let v = i.wrapping_mul(2_654_435_761);
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
        ]
    }

    /// Every colour the backdrop can show behind the bar: the flat colour, each field, each field
    /// at its peak over the flat colour, and two fields half and half.
    fn backdrops(m: &Mesh, base: BaseMode) -> Vec<Rgba> {
        let (under, used) = (m.under(base), m.used(base));
        let mut out = vec![under];
        for (i, &c) in used.iter().enumerate() {
            out.push(c);
            out.push(mix_rgb(under, c, f64::from(BLOB_PEAK)));
            out.extend(used[i + 1..].iter().map(|&d| mix_rgb(c, d, 0.5)));
        }
        out
    }

    /// Check both inks over the Material surface over `back`, with and without the shadow the CPU
    /// renderer also draws under the fill.
    fn check(n: &Neutrals, light: bool, back: Rgba, what: &str) {
        let shadow_colour = Rgba::from_rgb_hex(0x00_0000);
        for v in [0.0, VIGNETTE_MID_ALPHA, VIGNETTE_ALPHA] {
            let seen = n.lowest.with_alpha(v).over(back);
            for shade in [0.0, glass::SHADOW.alpha()] {
                let behind = shadow_colour.with_alpha(shade).over(seen);
                let f = n.high.with_alpha(fill_alpha(light, false)).over(behind);
                for (ink, name) in [
                    (n.on_surface, "on-surface"),
                    (n.on_surface_variant, "on-surface-variant"),
                ] {
                    let r = contrast_ratio(ink, f);
                    assert!(
                        r >= 4.5,
                        "{what} {name} {r:.2} (vignette {v}, shadow {shade})"
                    );
                }
            }
        }
    }

    #[test]
    fn the_ink_keeps_4_5_on_the_glass_over_any_cover_in_every_mode() {
        let seeds = [
            None,
            Some(Seed {
                hue: 25.0,
                chroma: 0.2,
            }),
            Some(Seed {
                hue: 100.0,
                chroma: 0.2,
            }),
        ];
        for (name, rgba) in covers() {
            let m = mesh::build(&rgba);
            for base in MODES {
                for seed in seeds {
                    let n = window_neutrals(base, seed);
                    for back in backdrops(&m, base) {
                        check(
                            &n,
                            base.is_light(),
                            back,
                            &format!("{name} {base:?} {seed:?}"),
                        );
                    }
                }
            }
        }
        // Song colours off: the flat field.
        for base in MODES {
            for seed in seeds {
                let n = window_neutrals(base, seed);
                for flat in bg_colors(seed, base) {
                    check(
                        &n,
                        base.is_light(),
                        flat,
                        &format!("flat {base:?} {seed:?}"),
                    );
                }
            }
        }
    }

    #[test]
    fn the_fill_is_90_percent_in_dark_92_in_light_and_opaque_when_solid() {
        assert!((fill_alpha(false, false) - 0.90).abs() < 1e-6);
        assert!((fill_alpha(true, false) - 0.92).abs() < 1e-6);
        assert!((fill_alpha(false, true) - 1.0).abs() < 1e-6);
        assert!((fill_alpha(true, true) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_capsule_has_no_outline_no_gradient_and_the_level_3_shadow() {
        let st = style(Neutrals::DARK.high.with_alpha(fill_alpha(false, false)));
        assert!(st.border.width.abs() < f32::EPSILON, "no outline, no rim");
        assert!(
            matches!(st.background, Some(iced::Background::Color(_))),
            "a flat fill, no glint or highlight gradient"
        );
        assert!((st.shadow.offset.y - 4.0).abs() < f32::EPSILON);
        assert!((st.shadow.blur_radius - 16.0).abs() < f32::EPSILON);
        assert!((st.shadow.color.a - 0.32).abs() < 0.01);
    }
}
