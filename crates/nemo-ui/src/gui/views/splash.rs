//! The start-up splash: the app mark, the wordmark, a thin progress line that follows the real
//! start-up steps, and the name of the step running (startup-flow spec, task 1).
//!
//! It draws on the neutral baseline of the saved base mode and the idle accent, never on a song
//! colour. `opacity` fades every part of it together, so the fade into the app needs no group
//! opacity from the renderer.

use iced::widget::{button, column, container};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use super::super::assets::Icon;
use super::super::style::{animated, col, gap, icon, label};
use crate::palette::Palette;
use crate::theme::{Rgba, color, shape, type_scale};

/// Width of the progress line.
const LINE_W: f32 = 240.0;
/// Height of the progress line (the 3 px rail of `design.md` section 10).
const LINE_H: f32 = 3.0;
/// Side of the mark tile.
const MARK: f32 = 64.0;
/// Size of the note glyph in the mark.
const GLYPH: f32 = 32.0;
/// Height of the Close button.
const BUTTON_H: f32 = 40.0;

/// What the splash draws at one moment.
#[derive(Debug, Clone, Copy)]
pub struct SplashLook<'a> {
    /// Finished share of the start-up steps, 0 to 1 (already eased by the caller).
    pub fraction: f32,
    /// The step running, or the plain sentence of a failure.
    pub status: &'a str,
    /// Opacity of the whole splash, 0 to 1.
    pub opacity: f32,
    /// The idle accent (not a song colour).
    pub accent: Palette,
    /// The start failed: the status is a sentence and the Close button shows.
    pub failed: bool,
}

fn tint(c: Rgba, opacity: f32) -> Color {
    let mut out = col(c);
    out.a *= opacity;
    out
}

/// The splash as a full-window layer. `on_close` is the message of the Close button, drawn only
/// when the start failed.
pub fn view<'a, M: Clone + 'a>(look: &SplashLook<'_>, on_close: M) -> Element<'a, M> {
    let o = look.opacity.clamp(0.0, 1.0);
    let accent = look.accent;
    let mark = container(icon(
        Icon::Note,
        GLYPH,
        tint(accent.on_primary_container, o),
    ))
    .width(Length::Fixed(MARK))
    .height(Length::Fixed(MARK))
    .center_x(Length::Fixed(MARK))
    .center_y(Length::Fixed(MARK))
    .style(move |_| container::Style {
        background: Some(Background::Color(tint(accent.primary_container, o))),
        border: Border {
            radius: shape::xl().into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let wordmark = label(
        "NemoMusic",
        type_scale::HEADLINE_MD,
        true,
        tint(color::on_surface(), o),
    );
    let status = label(
        look.status.to_owned(),
        type_scale::LABEL_SM,
        false,
        tint(color::on_surface_variant(), o),
    );

    let mut body = column![mark, wordmark]
        .spacing(gap::LG)
        .align_x(Alignment::Center);
    if look.failed {
        body = body.push(
            label(
                look.status.to_owned(),
                type_scale::BODY_MD,
                false,
                tint(color::on_surface(), o),
            )
            .wrapping(iced::widget::text::Wrapping::Word)
            .width(Length::Fixed(LINE_W + 2.0 * gap::XXL))
            .align_x(iced::alignment::Horizontal::Center),
        );
        body = body.push(close_button(accent, o, on_close));
    } else {
        body = body.push(progress_line(look.fraction, accent.primary, o));
        body = body.push(status);
    }

    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(tint(color::surface(), o))),
            ..container::Style::default()
        })
        .into()
}

/// The thin line: a rail with the finished part filled in the accent.
fn progress_line<'a, M: 'a>(fraction: f32, fill: Rgba, o: f32) -> Element<'a, M> {
    let filled = LINE_W * fraction.clamp(0.0, 1.0);
    let rail = container(
        container(iced::widget::Space::new())
            .width(Length::Fixed(filled))
            .height(Length::Fixed(LINE_H))
            .style(move |_| container::Style {
                background: Some(Background::Color(tint(fill, o))),
                border: Border {
                    radius: shape::FULL.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    )
    .width(Length::Fixed(LINE_W))
    .height(Length::Fixed(LINE_H))
    .style(move |_| container::Style {
        background: Some(Background::Color(tint(
            color::on_surface().with_alpha(0.12),
            o,
        ))),
        border: Border {
            radius: shape::FULL.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    container(rail)
        .padding(Padding::from([gap::XS, 0.0]))
        .into()
}

/// A tonal Close button for the failed start.
fn close_button<'a, M: Clone + 'a>(accent: Palette, o: f32, on_close: M) -> Element<'a, M> {
    let ink = tint(accent.on_primary_container, o);
    let b = button(
        container(label("Close", type_scale::BODY_MD, true, ink))
            .padding(Padding::from([0.0, gap::XL]))
            .height(Length::Fixed(BUTTON_H))
            .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(on_close);
    animated(b, tint(accent.primary_container, o), shape::FULL)
}
