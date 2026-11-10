//! Token-to-iced glue: colours, text, and the widget styles every view shares. Views take every
//! colour, size and radius from here or from [`crate::theme`], never a literal.

use iced::widget::{Button, Container, Svg, Text, button, container, slider, svg, text};
use iced::{Background, Border, Color, Element, Length, Padding, Pixels, Shadow, Vector};

use super::assets::{Icon, font};
use crate::theme::{Rgba, TextStyle, color, glass, shape};

/// Convert a token colour.
#[must_use]
pub fn col(c: Rgba) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, c.alpha())
}

/// The iced theme: only the base colours; widgets are styled explicitly.
#[must_use]
pub fn theme() -> iced::Theme {
    iced::Theme::custom(
        "NemoMusic".to_owned(),
        iced::theme::Palette {
            background: col(color::SURFACE),
            text: col(color::ON_SURFACE),
            primary: col(color::PRIMARY_FALLBACK),
            success: col(color::SUCCESS),
            warning: col(color::WARNING),
            danger: col(color::ERROR),
        },
    )
}

/// A text run in a type-scale style.
pub fn label<'a>(
    content: impl text::IntoFragment<'a>,
    style: TextStyle,
    semibold: bool,
    color: Color,
) -> Text<'a> {
    text(content)
        .size(Pixels(f32::from(style.size)))
        .line_height(text::LineHeight::Absolute(Pixels(f32::from(style.line))))
        .font(font(semibold))
        .color(color)
        .wrapping(text::Wrapping::None)
}

/// Shorten `s` with an ellipsis so it fits `width` px at `size` px. iced 0.14 text has no
/// ellipsis, so this uses an average advance (0.56 em for the bundled font) and errs short.
#[must_use]
pub fn fit(s: &str, width: f32, size: u16) -> String {
    // Display arithmetic on small numbers.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let max = ((width / (f32::from(size) * 0.56)).floor().max(1.0)) as usize;
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let keep = max.saturating_sub(1).max(1);
        let mut out: String = s.chars().take(keep).collect();
        out.push('\u{2026}');
        out
    }
}

/// A tinted icon of `size` px.
pub fn icon<'a>(icon: Icon, size: f32, color: Color) -> Svg<'a> {
    svg(icon.handle())
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_, _| svg::Style { color: Some(color) })
}

/// A flat container background.
pub fn fill<'a, M: 'a>(content: impl Into<Element<'a, M>>, bg: Rgba) -> Container<'a, M> {
    container(content).style(move |_| container::Style {
        background: Some(Background::Color(col(bg))),
        ..container::Style::default()
    })
}

/// Container with a background and a radius.
pub fn rounded<'a, M: 'a>(
    content: impl Into<Element<'a, M>>,
    bg: Color,
    radius: f32,
) -> Container<'a, M> {
    container(content).style(move |_| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
}

/// State-layer overlay colour (`design.md` section 5): `on-surface` at 8% hover, 10% pressed.
fn state_layer(status: button::Status) -> Option<Color> {
    let a = match status {
        button::Status::Hovered => 0.08,
        button::Status::Pressed => 0.10,
        _ => return None,
    };
    Some(col(color::ON_SURFACE.with_alpha(a)))
}

/// Style for a button whose idle background is `idle` (maybe transparent), radius `radius`.
pub fn button_style(
    idle: Color,
    radius: f32,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let layer = state_layer(status);
        // A state layer over an idle fill: mix by drawing the layer colour on top via alpha add.
        let bg = match (layer, idle.a) {
            (Some(l), a) if a <= 0.0 => l,
            (Some(l), _) => Color {
                r: idle.r + (l.r - idle.r) * l.a,
                g: idle.g + (l.g - idle.g) * l.a,
                b: idle.b + (l.b - idle.b) * l.a,
                a: idle.a,
            },
            (None, _) => idle,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: col(color::ON_SURFACE),
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            shadow: Shadow::default(),
            snap: false,
        }
    }
}

/// A round icon button (`size` px square target, >= 32 px).
pub fn icon_button<'a, M: Clone + 'a>(
    glyph: Icon,
    glyph_size: f32,
    target: f32,
    ink: Color,
    idle_bg: Color,
    message: Option<M>,
) -> Button<'a, M> {
    let inner = container(icon(glyph, glyph_size, ink))
        .width(Length::Fixed(target))
        .height(Length::Fixed(target))
        .center_x(Length::Fixed(target))
        .center_y(Length::Fixed(target));
    let b = button(inner)
        .padding(0)
        .style(button_style(idle_bg, shape::FULL));
    match message {
        Some(m) => b.on_press(m),
        None => b,
    }
}

/// Slider style: a 4 px rail, `fill` for the played part, a small round handle.
pub fn slider_style(
    fill: Color,
    rail: Color,
    handle: Color,
    handle_radius: f32,
) -> impl Fn(&iced::Theme, slider::Status) -> slider::Style {
    move |_, status| {
        let r = match status {
            slider::Status::Hovered | slider::Status::Dragged => handle_radius + 1.0,
            slider::Status::Active => handle_radius,
        };
        slider::Style {
            rail: slider::Rail {
                backgrounds: (Background::Color(fill), Background::Color(rail)),
                width: 4.0,
                border: Border {
                    radius: shape::FULL.into(),
                    ..Border::default()
                },
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: r },
                background: Background::Color(handle),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
            },
        }
    }
}

/// Glass tint colour: `surface-container` at `alpha`.
#[must_use]
pub fn glass_tint(alpha: f32) -> Color {
    col(glass::TINT_BASE.with_alpha(alpha))
}

/// Padding from the spacing scale: `[vertical, horizontal]`.
#[must_use]
pub fn pad(v: u16, h: u16) -> Padding {
    Padding::from([f32::from(v), f32::from(h)])
}

/// Spacing tokens as f32 (`design.md` section 4; the test checks them against the scale).
pub mod gap {
    /// 4 px.
    pub const XS: f32 = 4.0;
    /// 8 px.
    pub const SM: f32 = 8.0;
    /// 12 px.
    pub const MD: f32 = 12.0;
    /// 16 px.
    pub const LG: f32 = 16.0;
    /// 24 px.
    pub const XL: f32 = 24.0;
    /// 32 px.
    pub const XXL: f32 = 32.0;
    /// 48 px.
    pub const XXXL: f32 = 48.0;
    /// 64 px.
    pub const HUGE: f32 = 64.0;
}
/// Vertical offset helper for shadows.
#[must_use]
pub fn shadow(offset_y: f32, blur: f32, c: Rgba) -> Shadow {
    Shadow {
        color: col(c),
        offset: Vector::new(0.0, offset_y),
        blur_radius: blur,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::spacing;

    #[test]
    fn gaps_are_on_the_spacing_scale() {
        for g in [
            gap::XS,
            gap::SM,
            gap::MD,
            gap::LG,
            gap::XL,
            gap::XXL,
            gap::XXXL,
            gap::HUGE,
        ] {
            assert!(
                spacing::SCALE
                    .iter()
                    .any(|s| (f32::from(*s) - g).abs() < f32::EPSILON),
                "{g}"
            );
        }
    }

    #[test]
    fn fit_shortens_with_an_ellipsis() {
        assert_eq!(fit("short", 200.0, 14), "short");
        let s = fit("A very long title that cannot fit", 100.0, 14);
        assert!(s.ends_with('\u{2026}') && s.chars().count() <= 13, "{s}");
        assert_eq!(fit("x", 1.0, 40).chars().count(), 1);
    }
}
