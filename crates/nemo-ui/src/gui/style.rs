//! Token-to-iced glue: colours, text, and the widget styles every view shares. Views take every
//! colour, size and radius from here or from [`crate::theme`], never a literal.

use iced::widget::{Button, Container, Svg, Text, button, container, slider, svg, text, tooltip};
use iced::{Background, Border, Color, Element, Length, Padding, Pixels, Shadow, Vector};

use super::assets::{Icon, font, font_w};
use super::fx::fx;
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
            background: col(color::surface()),
            text: col(color::on_surface()),
            primary: col(color::PRIMARY_FALLBACK),
            success: col(color::success()),
            warning: col(color::warning()),
            danger: col(color::error()),
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

/// A text run at an exact weight (`Normal`, `Medium` or `Semibold`).
pub fn label_w<'a>(
    content: impl text::IntoFragment<'a>,
    style: TextStyle,
    weight: iced::font::Weight,
    color: Color,
) -> Text<'a> {
    text(content)
        .size(Pixels(f32::from(style.size)))
        .line_height(text::LineHeight::Absolute(Pixels(f32::from(style.line))))
        .font(font_w(weight))
        .color(color)
        .wrapping(text::Wrapping::None)
}

/// Shorten `s` with an ellipsis so it fits `width` px at `size` px. iced 0.14 text has no
/// ellipsis, so this measures with the real advances of the bundled font
/// ([`super::textfit`]) and cuts at a word end when that keeps most of the text.
#[must_use]
pub fn fit(s: &str, width: f32, size: u16) -> String {
    super::textfit::fit(s, width, f32::from(size))
}

/// Break `s` into at most `lines` lines of `width` px (see [`super::textfit::fit_lines`]).
#[must_use]
pub fn fit_lines(s: &str, width: f32, size: u16, lines: usize) -> String {
    super::textfit::fit_lines(s, width, f32::from(size), lines)
}

/// A tinted icon of `size` px.
pub fn icon<'a>(icon: Icon, size: f32, color: Color) -> Svg<'a> {
    svg(icon.handle())
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_, _| svg::Style { color: Some(color) })
}

/// The progress ring turned by `angle` radians (the app advances it while buffering).
pub fn spinner<'a>(size: f32, color: Color, angle: f32) -> Svg<'a> {
    icon(Icon::Spinner, size, color).rotation(iced::Radians(angle))
}

/// Wrap `content` in a tooltip that appears after 500 ms (`design.md` section 13).
pub fn tip<'a, M: 'a>(content: impl Into<Element<'a, M>>, text: &'a str) -> Element<'a, M> {
    tooltip(
        content,
        container(label(
            text,
            crate::theme::type_scale::LABEL_SM,
            false,
            col(color::on_surface()),
        ))
        .padding(Padding::from([gap::XS, gap::SM]))
        .style(|_| container::Style {
            background: Some(Background::Color(col(color::surface_container_high()))),
            border: Border {
                radius: shape::sm().into(),
                color: col(color::outline_variant()),
                width: 1.0,
            },
            ..container::Style::default()
        }),
        tooltip::Position::Top,
    )
    .delay(std::time::Duration::from_millis(500))
    .into()
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
    Some(col(color::on_surface().with_alpha(a)))
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
            text_color: col(color::on_surface()),
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            shadow: Shadow::default(),
            snap: false,
        }
    }
}

/// A button with a fixed fill and no state change of its own: [`Fx`] draws the animated state
/// layer on top. `radius` is the corner radius.
pub fn button_flat(
    idle: Color,
    radius: f32,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, _| button::Style {
        background: Some(Background::Color(idle)),
        text_color: col(color::on_surface()),
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        shadow: Shadow::default(),
        snap: false,
    }
}

/// A flat button wrapped in [`Fx`] (animated hover and press layers). The caller builds the
/// `button` (content, padding, size, message); this only styles and wraps it.
pub fn animated<'a, M: Clone + 'a>(
    button: Button<'a, M>,
    idle: Color,
    radius: f32,
) -> Element<'a, M> {
    fx(button.style(button_flat(idle, radius)))
        .radius(radius)
        .into()
}

/// A round icon button (`size` px square target, >= 32 px) with an animated state layer. With no
/// message it is drawn at 38% and does nothing.
pub fn icon_button<'a, M: Clone + 'a>(
    glyph: Icon,
    glyph_size: f32,
    target: f32,
    ink: Color,
    idle_bg: Color,
    message: Option<M>,
) -> Element<'a, M> {
    let ink = if message.is_some() {
        ink
    } else {
        Color {
            a: ink.a * 0.38,
            ..ink
        }
    };
    let inner = container(icon(glyph, glyph_size, ink))
        .width(Length::Fixed(target))
        .height(Length::Fixed(target))
        .center_x(Length::Fixed(target))
        .center_y(Length::Fixed(target));
    let enabled = message.is_some();
    let b = button(inner).padding(0);
    let b = match message {
        Some(m) => b.on_press(m),
        None => b,
    };
    let wrapped = fx(b.style(button_flat(idle_bg, shape::FULL))).radius(target / 2.0);
    if enabled {
        wrapped.into()
    } else {
        wrapped.passive().into()
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

/// A flat button with a fill and an optional 1 px outline, for chips. [`Fx`] adds the layer.
pub fn button_outlined(
    fill: Color,
    outline: Option<Color>,
    radius: f32,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, _| button::Style {
        background: Some(Background::Color(fill)),
        text_color: col(color::on_surface()),
        border: Border {
            radius: radius.into(),
            width: if outline.is_some() { 1.0 } else { 0.0 },
            color: outline.unwrap_or(Color::TRANSPARENT),
        },
        shadow: Shadow::default(),
        snap: false,
    }
}

/// Angle of a top-to-bottom gradient in iced (to be checked against a snapshot).
pub const DOWN: f32 = std::f32::consts::PI;

/// A vertical two-stop gradient background, `top` to `bottom`.
#[must_use]
pub fn vertical_gradient(top: Color, bottom: Color) -> Background {
    Background::Gradient(
        iced::gradient::Linear::new(iced::Radians(DOWN))
            .add_stop(0.0, top)
            .add_stop(1.0, bottom)
            .into(),
    )
}

/// Thin scroll bars that match the panes: a 6 px pill at 24% `on-surface`, 40% on hover.
pub fn scroll_style(
    theme: &iced::Theme,
    status: iced::widget::scrollable::Status,
) -> iced::widget::scrollable::Style {
    use iced::widget::scrollable::Status;
    let mut style = iced::widget::scrollable::default(theme, status);
    let alpha = match status {
        Status::Hovered {
            is_vertical_scrollbar_hovered: true,
            ..
        }
        | Status::Dragged {
            is_vertical_scrollbar_dragged: true,
            ..
        } => 0.40,
        _ => 0.20,
    };
    style.container = container::Style::default();
    style.vertical_rail.background = None;
    style.vertical_rail.border = Border::default();
    style.vertical_rail.scroller.background =
        Background::Color(col(color::on_surface().with_alpha(alpha)));
    style.vertical_rail.scroller.border = Border {
        radius: shape::FULL.into(),
        ..Border::default()
    };
    style
}

/// A vertical scroll area with the thin scroll bar.
pub fn scroll<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> iced::widget::Scrollable<'a, M> {
    iced::widget::scrollable(content)
        .direction(iced::widget::scrollable::Direction::Vertical(
            iced::widget::scrollable::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .margin(2),
        ))
        .style(scroll_style)
}

/// Glass tint colour: `surface-container` at `alpha`.
#[must_use]
pub fn glass_tint(alpha: f32) -> Color {
    col(glass::tint_base().with_alpha(alpha))
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
        assert!(s.ends_with('\u{2026}') && s.chars().count() <= 16, "{s}");
        assert_eq!(fit("x", 1.0, 40).chars().count(), 1);
    }
}
