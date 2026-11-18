//! The first-run setup flow (startup-flow spec): a calm full-window layer with a top bar
//! (`Skip setup`, always there), one step at a time, and a bottom bar with Back, the step dots
//! and Next. The steps are in [`steps`] and [`sources`].

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use super::kit::{filled_button, text_button, tonal_button};
use crate::gui::app::{App, Message, SLIDE_PX};
use crate::gui::assets::Icon;
use crate::gui::fx::Fx;
use crate::gui::style::{col, gap, label, scroll};
use crate::setup::{SetupEvent, SetupStep};
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

mod notice;
mod sources;
mod steps;

/// Widest content column.
const CONTENT_W: f32 = 560.0;
/// Width reserved for the Back and Next buttons, so the dots stay centred.
const SIDE_W: f32 = 168.0;
/// Height of the top and bottom bars.
const BAR_H: f32 = 72.0;
/// Side of a step dot.
const DOT: f32 = 8.0;
/// Width of the dot of the current step.
const DOT_ON: f32 = 24.0;

/// The setup flow as a full-window layer.
pub fn view(app: &App) -> Element<'_, Message> {
    let (w, _) = app.window_px();
    // Window sizes are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let width = (w as f32 - 2.0 * gap::XXL).clamp(280.0, CONTENT_W);
    let step = app.ui.setup.step();
    let body = match step {
        SetupStep::Welcome => steps::welcome(app, width),
        SetupStep::Look => steps::look(app, width),
        SetupStep::Notice => notice::view(app, width),
        SetupStep::Sources => sources::view(app, width),
        SetupStep::Playback => steps::playback(app, width),
        SetupStep::Done => steps::done(app, width),
    };
    let page = column![
        top_bar(),
        container(transition(app, body))
            .width(Length::Fill)
            .height(Length::Fill),
        nav_bar(app, step),
    ];
    container(page)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(col(color::surface()))),
            ..container::Style::default()
        })
        .into()
}

/// Slide and fade the step in (within `motion.slow`); reduced motion has no distance to cover.
fn transition<'a>(app: &App, body: Element<'a, Message>) -> Element<'a, Message> {
    let p = app.setup_progress();
    let shift = (1.0 - p) * SLIDE_PX * if app.ui.setup.forward() { 1.0 } else { -1.0 };
    // A shift is a difference between left and right padding, which moves the centred column.
    let pad = Padding {
        left: shift.max(0.0) * 2.0,
        right: (-shift).max(0.0) * 2.0,
        top: gap::SM,
        bottom: gap::XL,
    };
    let centred = container(scroll(
        container(body)
            .width(Length::Fill)
            .center_x(Length::Fill)
            .padding(pad),
    ))
    .width(Length::Fill)
    .height(Length::Fill);
    Fx::new(centred)
        .passive()
        .entrance(p, 0.0, col(color::surface()))
        .into()
}

fn top_bar<'a>() -> Element<'a, Message> {
    container(
        row![
            label(
                "NemoMusic",
                type_scale::TITLE_SM,
                true,
                col(color::on_surface_variant()),
            ),
            Space::new().width(Length::Fill),
            text_button("Skip setup", Message::Ui(Event::Setup(SetupEvent::Skip))),
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([0.0, gap::XL]))
    .height(Length::Fixed(BAR_H))
    .align_y(Alignment::Center)
    .into()
}

fn nav_bar(app: &App, step: SetupStep) -> Element<'_, Message> {
    let back: Element<'_, Message> = if step == SetupStep::Welcome {
        Space::new().width(Length::Fixed(SIDE_W)).into()
    } else {
        container(text_button(
            "Back",
            Message::Ui(Event::Setup(SetupEvent::Back)),
        ))
        .width(Length::Fixed(SIDE_W))
        .into()
    };
    let next = container(next_button(app, step))
        .width(Length::Fixed(SIDE_W))
        .align_x(Alignment::End);
    container(
        row![
            back,
            Space::new().width(Length::Fill),
            dots(app, step),
            Space::new().width(Length::Fill),
            next,
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([0.0, gap::XL]))
    .height(Length::Fixed(BAR_H))
    .align_y(Alignment::Center)
    .into()
}

/// The one forward button: `Next`, `Start listening` on the last step, and a disabled `Next`
/// on the notice step until the notice has an answer.
fn next_button(app: &App, step: SetupStep) -> Element<'_, Message> {
    let accent = app.accent();
    match step {
        SetupStep::Done => filled_button(
            "Start listening",
            Icon::Play,
            accent,
            Message::Ui(Event::Setup(SetupEvent::Finish)),
        ),
        SetupStep::Notice if !app.ui.notice_decided() => tonal_button("Next", None, None),
        _ => filled_button(
            "Next",
            Icon::ArrowForward,
            accent,
            Message::Ui(Event::Setup(SetupEvent::Next)),
        ),
    }
}

/// One dot per step; the current one is a longer pill in the accent.
fn dots(app: &App, current: SetupStep) -> Element<'_, Message> {
    let accent = app.accent();
    let mut r = row![].spacing(gap::SM).align_y(Alignment::Center);
    for step in SetupStep::ALL {
        let on = step == current;
        let fill = if on {
            col(accent.primary)
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
