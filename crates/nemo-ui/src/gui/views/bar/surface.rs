//! The capsule's surface. Tier Low draws real glass (owner, 2026-11-19; backdrop-calm): the
//! `surface-container` fill at half opacity over the window backdrop, which shows through (the
//! panes end above the capsule, so only the backdrop lies behind it), a light inner gradient, a
//! 1 px white rim with a brighter glint on the top edge, a 1 px `outline-variant` edge and a soft
//! outer shadow. Tier Off and reduce transparency draw the opaque `surface-container-high` with
//! the 1 px `outline-variant` edge. The edges are borders and gradients, not textures: images
//! cannot be clipped to round corners on the software renderer.

use iced::widget::{Space, container, stack};
use iced::{Border, Element, Length, Padding};

use super::px;
use crate::gui::app::{App, Message};
use crate::gui::style::{col, shadow, vertical_gradient};
use crate::layout::PLAYER_BAR_HEIGHT;
use crate::settings::GlassTier;
use crate::theme::{Rgba, color, glass, shape};

/// `true` when the capsule draws glass: tier Low with transparency not reduced.
#[must_use]
pub fn glassy(app: &App) -> bool {
    app.ui.settings.glass_tier == GlassTier::Low && !app.ui.settings.reduce_transparency
}

/// The glass fill at the top and at the bottom edge, with alpha: `base` at
/// [`glass::BAR_FILL_ALPHA`] plus and minus [`glass::BAR_FILL_SLOPE`], the top lifted toward
/// white by [`glass::BAR_LIFT`].
#[must_use]
pub fn glass_stops(base: Rgba) -> [Rgba; 2] {
    let top = glass::EDGE
        .with_alpha(glass::BAR_LIFT)
        .over(base)
        .with_alpha(glass::BAR_FILL_ALPHA + glass::BAR_FILL_SLOPE);
    let bottom = base.with_alpha(glass::BAR_FILL_ALPHA - glass::BAR_FILL_SLOPE);
    [top, bottom]
}

/// A left to right gradient through three stops.
fn across(stops: [iced::Color; 3]) -> iced::Background {
    let g = iced::gradient::Linear::new(iced::Radians(std::f32::consts::FRAC_PI_2))
        .add_stop(0.0, stops[0])
        .add_stop(0.5, stops[1])
        .add_stop(1.0, stops[2]);
    iced::Background::Gradient(g.into())
}

/// The capsule body: fill, edge and shadow, `bar_w` px wide.
pub(super) fn body<'a>(app: &App, bar_w: f32) -> Element<'a, Message> {
    let (background, edge, shade) = if glassy(app) {
        let [top, bottom] = glass_stops(color::surface_container());
        (
            vertical_gradient(col(top), col(bottom)),
            col(color::outline_variant().with_alpha(glass::BAR_OUTLINE_ALPHA)),
            shadow(glass::SHADOW_OFFSET_Y, glass::SHADOW_BLUR, glass::SHADOW),
        )
    } else {
        (
            col(color::surface_container_high()).into(),
            col(color::outline_variant()),
            shadow(
                glass::ELEVATED_OFFSET_Y,
                glass::ELEVATED_BLUR,
                glass::SHADOW,
            ),
        )
    };
    container(Space::new())
        .width(Length::Fixed(bar_w))
        .height(Length::Fixed(px(PLAYER_BAR_HEIGHT)))
        .style(move |_| container::Style {
            background: Some(background),
            border: Border {
                color: edge,
                width: 1.0,
                radius: shape::xl().into(),
            },
            shadow: shade,
            ..container::Style::default()
        })
        .into()
}

/// The 1 px white rim just inside the edge (tier Low only).
fn rim<'a>(bar_w: f32) -> Element<'a, Message> {
    let ring = container(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            border: Border {
                color: col(glass::EDGE.with_alpha(glass::EDGE_ALPHA)),
                width: 1.0,
                radius: (shape::xl() - 1.0).into(),
            },
            ..container::Style::default()
        });
    container(ring)
        .width(Length::Fixed(bar_w))
        .height(Length::Fixed(px(PLAYER_BAR_HEIGHT)))
        .padding(1)
        .into()
}

/// The rim and the glint on the top edge (tier Low only): a 1 px line that fades out toward
/// both corners.
pub(super) fn glint<'a>(app: &App, bar_w: f32) -> Element<'a, Message> {
    if !glassy(app) {
        return Space::new().into();
    }
    let peak = col(glass::EDGE.with_alpha(glass::EDGE_ALPHA_TOP));
    let none = col(glass::EDGE.with_alpha(0.0));
    let line = container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_| container::Style {
            background: Some(across([none, peak, none])),
            ..container::Style::default()
        });
    let top = container(line)
        .width(Length::Fixed(bar_w))
        .height(Length::Fixed(px(PLAYER_BAR_HEIGHT)))
        .padding(Padding {
            top: 1.0,
            left: shape::xl(),
            right: shape::xl(),
            bottom: 0.0,
        });
    stack![rim(bar_w), top].into()
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

    /// Check both inks over the glass over `back`, with and without the shadow the CPU renderer
    /// also draws under the fill.
    fn check(n: &Neutrals, back: Rgba, what: &str) {
        let shadow_colour = Rgba::from_rgb_hex(0x00_0000);
        for v in [0.0, VIGNETTE_MID_ALPHA, VIGNETTE_ALPHA] {
            let seen = n.lowest.with_alpha(v).over(back);
            for shade in [0.0, glass::SHADOW.alpha()] {
                let behind = shadow_colour.with_alpha(shade).over(seen);
                for fill in glass_stops(n.container) {
                    let f = fill.over(behind);
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
                        check(&n, back, &format!("{name} {base:?} {seed:?}"));
                    }
                }
            }
        }
        // Song colours off: the flat field.
        for base in MODES {
            for seed in seeds {
                let n = window_neutrals(base, seed);
                for flat in bg_colors(seed, base) {
                    check(&n, flat, &format!("flat {base:?} {seed:?}"));
                }
            }
        }
    }

    #[test]
    fn the_glass_is_translucent_and_a_little_lighter_on_top() {
        for n in [Neutrals::DARK, Neutrals::AMOLED, Neutrals::LIGHT] {
            let [top, bottom] = glass_stops(n.container);
            for a in [top.alpha(), bottom.alpha()] {
                assert!((0.40..=0.55).contains(&a), "fill alpha {a}");
            }
            assert!(top.alpha() > bottom.alpha(), "more fill at the top edge");
            assert_ne!(top.with_alpha(1.0), bottom.with_alpha(1.0), "a lifted top");
        }
    }
}
