//! The stepper of the setup flow: the vertical one of the brand panel and the row of dots below
//! 1100 px. A marker, a connector or a dot that changes state while the step changes morphs from
//! its old look to its new one over `motion.base` (brand-intro spec, task 3).

use iced::widget::{Space, column, container, row, stack};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::gui::app::{App, Message};
use crate::gui::assets::Icon;
use crate::gui::style::{col, gap, icon, label};
use crate::palette::Palette;
use crate::setup::SetupStep;
use crate::setup::motion::morph;
use crate::theme::{color, shape, type_scale};

/// Side of a stepper marker.
const MARKER: f32 = 24.0;
/// Width of the outline of a marker still to come.
const EDGE: f32 = 1.5;
/// Height of a stepper row, and of the connector between two rows.
const STEP_H: f32 = 32.0;
const LINK_H: f32 = 12.0;
/// Width of the connector.
const LINK_W: f32 = 2.0;
/// Side of a step dot, and the width of the current one.
const DOT: f32 = 8.0;
const DOT_ON: f32 = 24.0;
/// Side of the check at rest.
const CHECK: f32 = 16.0;

/// Where a step is, seen from the current one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Upcoming,
    Current,
    Done,
}

fn state_of(step: SetupStep, current: SetupStep) -> State {
    match step.index().cmp(&current.index()) {
        std::cmp::Ordering::Less => State::Done,
        std::cmp::Ordering::Equal => State::Current,
        std::cmp::Ordering::Greater => State::Upcoming,
    }
}

/// The state `step` was in before the change that is running, and the one it is in now.
fn states(app: &App, step: SetupStep, current: SetupStep) -> (State, State) {
    let now = state_of(step, current);
    (app.setup_from().map_or(now, |f| state_of(step, f)), now)
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

/// What a stepper marker looks like in one state.
#[derive(Debug, Clone, Copy)]
struct Look {
    fill: Color,
    ink: Color,
    edge: f32,
    /// Opacity of the number, and size (0 to 1) of the check.
    number: f32,
    check: f32,
}

fn look(state: State, accent: Palette) -> Look {
    match state {
        State::Current => Look {
            fill: col(accent.primary),
            ink: col(accent.on_primary),
            edge: 0.0,
            number: 1.0,
            check: 0.0,
        },
        State::Done => Look {
            fill: col(accent.primary_container),
            ink: col(accent.on_primary_container),
            edge: 0.0,
            number: 0.0,
            check: 1.0,
        },
        State::Upcoming => Look {
            fill: col(accent.primary.with_alpha(0.0)),
            ink: col(color::on_surface_variant()),
            edge: EDGE,
            number: 1.0,
            check: 0.0,
        },
    }
}

fn mix_look(a: Look, b: Look, t: f32) -> Look {
    Look {
        fill: mix(a.fill, b.fill, t),
        ink: mix(a.ink, b.ink, t),
        edge: a.edge + (b.edge - a.edge) * t,
        number: a.number + (b.number - a.number) * t,
        check: a.check + (b.check - a.check) * t,
    }
}

/// One row per step: a marker (a check once done, the number otherwise), the step's name and a
/// connector to the next row.
pub(super) fn stepper(app: &App, current: SetupStep) -> Element<'_, Message> {
    let accent = app.accent();
    let p = morph(app.setup_ms());
    let mut c = column![];
    for step in SetupStep::ALL {
        let (before, now) = states(app, step, current);
        let ink = if now == State::Upcoming {
            col(color::on_surface_variant())
        } else {
            col(color::on_surface())
        };
        let shown = mix_look(look(before, accent), look(now, accent), p);
        c = c.push(
            row![
                marker(step, shown),
                Space::new().width(gap::MD),
                label(
                    step.title(),
                    type_scale::BODY_MD,
                    now == State::Current,
                    ink
                ),
            ]
            .align_y(Alignment::Center)
            .height(Length::Fixed(STEP_H)),
        );
        if step.next().is_some() {
            c = c.push(link(before == State::Done, now == State::Done, p, accent));
        }
    }
    c.into()
}

/// The marker of one step: the number and the check cross-fade, the check growing from nothing.
fn marker<'a>(step: SetupStep, shown: Look) -> Element<'a, Message> {
    let centred = |inside: Element<'a, Message>| -> Element<'a, Message> {
        container(inside)
            .width(Length::Fixed(MARKER))
            .height(Length::Fixed(MARKER))
            .center_x(Length::Fixed(MARKER))
            .center_y(Length::Fixed(MARKER))
            .into()
    };
    let faded = |c: Color, a: f32| Color { a: c.a * a, ..c };
    let mut layers: Vec<Element<'a, Message>> = Vec::new();
    if shown.number > 0.02 {
        let number = label(
            (step.index() + 1).to_string(),
            type_scale::LABEL_SM,
            true,
            faded(shown.ink, shown.number),
        );
        layers.push(centred(number.into()));
    }
    if shown.check > 0.02 {
        let check = icon(
            Icon::Check,
            CHECK * shown.check,
            faded(shown.ink, shown.check),
        );
        layers.push(centred(check.into()));
    }
    let edge = col(color::outline());
    container(stack(layers))
        .width(Length::Fixed(MARKER))
        .height(Length::Fixed(MARKER))
        .style(move |_| container::Style {
            background: Some(Background::Color(shown.fill)),
            border: Border {
                radius: shape::FULL.into(),
                width: shown.edge,
                color: Color {
                    a: edge.a * (shown.edge / EDGE),
                    ..edge
                },
            },
            ..container::Style::default()
        })
        .into()
}

/// The short line between two markers: in the accent once the step above it is done.
fn link<'a>(was: bool, is: bool, p: f32, accent: Palette) -> Element<'a, Message> {
    let tone = |done: bool| {
        if done {
            col(accent.primary_container)
        } else {
            col(color::outline_variant())
        }
    };
    let fill = mix(tone(was), tone(is), p);
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

/// One dot per step; the current one is a longer pill in the accent, finished ones are a
/// quieter accent.
pub(super) fn dots(app: &App, current: SetupStep) -> Element<'_, Message> {
    let accent = app.accent();
    let p = morph(app.setup_ms());
    let dot = |state: State| match state {
        State::Current => (DOT_ON, col(accent.primary)),
        State::Done => (DOT, col(accent.primary_container)),
        State::Upcoming => (DOT, col(color::on_surface().with_alpha(0.24))),
    };
    let mut r = row![].spacing(gap::SM).align_y(Alignment::Center);
    for step in SetupStep::ALL {
        let (before, now) = states(app, step, current);
        let ((w0, c0), (w1, c1)) = (dot(before), dot(now));
        let (width, fill) = (w0 + (w1 - w0) * p, mix(c0, c1, p));
        r = r.push(
            container(Space::new())
                .width(Length::Fixed(width))
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
