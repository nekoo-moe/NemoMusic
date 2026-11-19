//! The stage around the step: the brand panel with the vertical stepper (from 1100 px wide) and
//! the top row with dots (below that). Shared by every step.

use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Pixels};

use crate::gui::app::{App, Message};
use crate::gui::assets::{Icon, display_font};
use crate::gui::style::{col, gap, icon, label};
use crate::palette::Palette;
use crate::setup::SetupStep;
use crate::theme::{TextStyle, color, shape, type_scale};

/// Side of the mark in the brand panel and in the top row.
const MARK_PANEL: f32 = 48.0;
const MARK_TOP: f32 = 32.0;
/// Side of a stepper marker.
const MARKER: f32 = 24.0;
/// Height of a stepper row, and of the connector between two rows.
const STEP_H: f32 = 32.0;
const LINK_H: f32 = 12.0;
/// Width of the connector.
const LINK_W: f32 = 2.0;
/// Height of the top row.
const TOP_H: f32 = 48.0;
/// Side of a step dot, and the width of the current one.
const DOT: f32 = 8.0;
const DOT_ON: f32 = 24.0;

/// A run of text in `font.display` at 600.
pub(super) fn display<'a>(
    content: impl Into<String>,
    style: TextStyle,
    ink: Color,
) -> iced::widget::Text<'a> {
    text(content.into())
        .size(Pixels(f32::from(style.size)))
        .line_height(text::LineHeight::Absolute(Pixels(f32::from(style.line))))
        .font(display_font(true))
        .color(ink)
        .wrapping(text::Wrapping::Word)
}

/// The brand mark: a disc in the accent with the note.
pub(super) fn mark<'a>(accent: Palette, side: f32) -> Element<'a, Message> {
    container(icon(Icon::Note, side / 2.0, col(accent.on_primary)))
        .width(Length::Fixed(side))
        .height(Length::Fixed(side))
        .center_x(Length::Fixed(side))
        .center_y(Length::Fixed(side))
        .style(move |_| container::Style {
            background: Some(Background::Color(col(accent.primary))),
            border: Border {
                radius: shape::FULL.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// The left panel: the mark and name, where the person is (`Step 2 of 6` and the step's name in
/// `font.display`), and the vertical stepper.
pub(super) fn brand_panel(app: &App, current: SetupStep, width: f32) -> Element<'_, Message> {
    let accent = app.accent();
    let dim = col(color::on_surface_variant());
    let brand = row![
        mark(accent, MARK_PANEL),
        Space::new().width(gap::MD),
        display("NemoMusic", type_scale::TITLE_MD, col(color::on_surface())),
    ]
    .align_y(Alignment::Center);
    let count = SetupStep::ALL.len();
    let place = format!("Step {} of {count}", current.index() + 1);
    let body = column![
        brand,
        Space::new().height(gap::XL),
        label(place, type_scale::LABEL_SM, false, dim),
        display(
            current.title(),
            type_scale::HEADLINE_LG,
            col(color::on_surface())
        )
        .width(Length::Fill),
        Space::new().height(gap::XL),
        stepper(app, current),
    ]
    .width(Length::Fixed(width - 2.0 * gap::SM));
    container(body)
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .padding(Padding::from([gap::SM, gap::SM]))
        .into()
}

/// One row per step: a marker (a check once done, the number otherwise), the step's name and a
/// connector to the next row.
fn stepper(app: &App, current: SetupStep) -> Element<'_, Message> {
    let accent = app.accent();
    let mut c = column![];
    for step in SetupStep::ALL {
        let done = step.index() < current.index();
        let on = step == current;
        let ink = if on || done {
            col(color::on_surface())
        } else {
            col(color::on_surface_variant())
        };
        c = c.push(
            row![
                marker(step, done, on, accent),
                Space::new().width(gap::MD),
                label(step.title(), type_scale::BODY_MD, on, ink),
            ]
            .align_y(Alignment::Center)
            .height(Length::Fixed(STEP_H)),
        );
        if step.next().is_some() {
            c = c.push(link(done, accent));
        }
    }
    c.into()
}

/// The marker of one step: filled with the number when current, a check in the container when
/// done, an outline with the number when still to come.
fn marker<'a>(step: SetupStep, done: bool, on: bool, accent: Palette) -> Element<'a, Message> {
    let (fill, ink, edge) = if on {
        (Some(accent.primary), accent.on_primary, 0.0)
    } else if done {
        (
            Some(accent.primary_container),
            accent.on_primary_container,
            0.0,
        )
    } else {
        (None, color::on_surface_variant(), 1.5)
    };
    let inside: Element<'a, Message> = if done {
        icon(Icon::Check, 16.0, col(ink)).into()
    } else {
        label(
            (step.index() + 1).to_string(),
            type_scale::LABEL_SM,
            true,
            col(ink),
        )
        .into()
    };
    container(inside)
        .width(Length::Fixed(MARKER))
        .height(Length::Fixed(MARKER))
        .center_x(Length::Fixed(MARKER))
        .center_y(Length::Fixed(MARKER))
        .style(move |_| container::Style {
            background: fill.map(|c| Background::Color(col(c))),
            border: Border {
                radius: shape::FULL.into(),
                width: edge,
                color: col(color::outline()),
            },
            ..container::Style::default()
        })
        .into()
}

/// The short line between two markers: in the accent once the step above it is done.
fn link<'a>(done: bool, accent: Palette) -> Element<'a, Message> {
    let fill = if done {
        col(accent.primary_container)
    } else {
        col(color::outline_variant())
    };
    container(
        container(Space::new())
            .width(Length::Fixed(LINK_W))
            .height(Length::Fixed(LINK_H))
            .style(move |_| container::Style {
                background: Some(Background::Color(fill)),
                ..container::Style::default()
            }),
    )
    .padding(Padding {
        left: (MARKER - LINK_W) / 2.0,
        ..Padding::ZERO
    })
    .into()
}

/// Below 1100 px: the small mark and name on the left, one dot per step in the middle.
pub(super) fn top_row(app: &App, current: SetupStep) -> Element<'_, Message> {
    let accent = app.accent();
    let side = 200.0;
    row![
        container(
            row![
                mark(accent, MARK_TOP),
                Space::new().width(gap::SM),
                display("NemoMusic", type_scale::TITLE_SM, col(color::on_surface())),
            ]
            .align_y(Alignment::Center)
        )
        .width(Length::Fixed(side)),
        Space::new().width(Length::Fill),
        dots(app, current),
        Space::new().width(Length::Fill),
        Space::new().width(Length::Fixed(side)),
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([0.0, gap::SM]))
    .height(Length::Fixed(TOP_H))
    .into()
}

/// One dot per step; the current one is a longer pill in the accent, finished ones are a
/// quieter accent.
fn dots(app: &App, current: SetupStep) -> Element<'_, Message> {
    let accent = app.accent();
    let mut r = row![].spacing(gap::SM).align_y(Alignment::Center);
    for step in SetupStep::ALL {
        let on = step == current;
        let fill = if on {
            col(accent.primary)
        } else if step.index() < current.index() {
            col(accent.primary_container)
        } else {
            col(color::on_surface().with_alpha(0.24))
        };
        r = r.push(
            container(Space::new())
                .width(Length::Fixed(if on { DOT_ON } else { DOT }))
                .height(Length::Fixed(DOT))
                .style(move |_| container::Style {
                    background: Some(Background::Color(fill)),
                    border: Border {
                        radius: shape::FULL.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }),
        );
    }
    r.into()
}
