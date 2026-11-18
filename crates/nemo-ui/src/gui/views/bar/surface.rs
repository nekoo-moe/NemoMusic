//! The capsule's surface. Tier Low draws an opaque tonal fill with a soft tint from the cover's
//! three sampled colours and a glint on the top edge; tier Off and reduce transparency draw the
//! plain tonal fill. Both have the 1 px `outline-variant` edge. The tint is a gradient, not a
//! texture: images cannot be clipped to round corners on the software renderer.

use iced::widget::{Space, container};
use iced::{Background, Border, Element, Length, Padding};

use super::px;
use crate::gui::app::{App, Message};
use crate::gui::style::{col, shadow};
use crate::layout::PLAYER_BAR_HEIGHT;
use crate::palette::mix_rgb;
use crate::settings::GlassTier;
use crate::theme::{Rgba, color, contrast_ratio, glass, shape};

/// `true` when the capsule draws glass: tier Low with transparency not reduced.
#[must_use]
pub fn glassy(app: &App) -> bool {
    app.ui.settings.glass_tier == GlassTier::Low && !app.ui.settings.reduce_transparency
}

/// Fill of one stop: `base` with the sampled cover colour `c` on top at the tint share, eased
/// off in steps of 0.02 until `ink` (the weakest text on the capsule) keeps 4.5:1 on it.
#[must_use]
fn tinted(base: Rgba, ink: Rgba, c: Rgba) -> Rgba {
    let mut alpha = glass::BAR_TINT_ALPHA;
    loop {
        let fill = c.with_alpha(alpha).over(base);
        if alpha <= 0.0 || contrast_ratio(ink, fill) >= 4.5 {
            return fill;
        }
        alpha = (alpha - 0.02).max(0.0);
    }
}

/// The fill colours left, middle and right.
#[must_use]
pub fn fill_stops(base: Rgba, ink: Rgba, strip: [Rgba; 3]) -> [Rgba; 3] {
    strip.map(|c| tinted(base, ink, c))
}

/// The cover's sampled colours now, cross-faded from the previous cover's while they fade.
fn strip(app: &App) -> Option<[Rgba; 3]> {
    let now = app.cover.as_ref()?.strip;
    let fade = app.anim.cover();
    Some(match &app.prev_cover {
        Some(prev) if fade < 0.999 => {
            [0, 1, 2].map(|i| mix_rgb(prev.strip[i], now[i], f64::from(fade)))
        }
        _ => now,
    })
}

/// A left to right gradient through three stops.
fn across(stops: [iced::Color; 3]) -> Background {
    let g = iced::gradient::Linear::new(iced::Radians(std::f32::consts::FRAC_PI_2))
        .add_stop(0.0, stops[0])
        .add_stop(0.5, stops[1])
        .add_stop(1.0, stops[2]);
    Background::Gradient(g.into())
}

/// The capsule body: fill, edge and shadow, `bar_w` px wide.
pub(super) fn body<'a>(app: &App, bar_w: f32) -> Element<'a, Message> {
    let base = color::surface_container_high();
    let glass_on = glassy(app);
    let tinted = if glass_on {
        strip(app).map(|s| fill_stops(base, color::on_surface_variant(), s).map(col))
    } else {
        None
    };
    let shade = if glass_on {
        shadow(glass::SHADOW_OFFSET_Y, glass::SHADOW_BLUR, glass::SHADOW)
    } else {
        shadow(
            glass::ELEVATED_OFFSET_Y,
            glass::ELEVATED_BLUR,
            glass::SHADOW,
        )
    };
    let radius = shape::xl();
    container(Space::new())
        .width(Length::Fixed(bar_w))
        .height(Length::Fixed(px(PLAYER_BAR_HEIGHT)))
        .style(move |_| container::Style {
            background: Some(tinted.map_or_else(|| col(base).into(), across)),
            border: Border {
                color: col(color::outline_variant()),
                width: 1.0,
                radius: radius.into(),
            },
            shadow: shade,
            ..container::Style::default()
        })
        .into()
}

/// The glint on the top edge (tier Low only): a 1 px line that fades out toward both corners.
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
    container(line)
        .width(Length::Fixed(bar_w))
        .height(Length::Fixed(px(PLAYER_BAR_HEIGHT)))
        .padding(Padding {
            top: 1.0,
            left: shape::xl(),
            right: shape::xl(),
            bottom: 0.0,
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Neutrals;

    /// Cover colours that stress the tint: black, white, grey and a saturated red, green, blue.
    const COVERS: [u32; 6] = [
        0x00_0000, 0xFF_FFFF, 0x80_8080, 0xFF_0000, 0x00_FF00, 0x00_00FF,
    ];

    #[test]
    fn the_ink_keeps_its_contrast_on_the_tinted_capsule_over_any_cover() {
        for n in [Neutrals::DARK, Neutrals::AMOLED, Neutrals::LIGHT] {
            for hex in COVERS {
                let c = Rgba::from_rgb_hex(hex);
                for fill in fill_stops(n.high, n.on_surface_variant, [c, c, c]) {
                    assert!(
                        contrast_ratio(n.on_surface, fill) >= 4.5,
                        "on-surface over {hex:06x}"
                    );
                    assert!(
                        contrast_ratio(n.on_surface_variant, fill) >= 4.5,
                        "on-surface-variant over {hex:06x}"
                    );
                }
            }
        }
    }

    #[test]
    fn no_cover_colour_means_no_tint_stops_to_draw() {
        let base = Neutrals::DARK.high;
        let same = fill_stops(base, Neutrals::DARK.on_surface_variant, [base; 3]);
        assert_eq!(same, [base; 3]);
    }
}
