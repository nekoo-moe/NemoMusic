//! The first-run setup flow (startup-flow and brand-intro specs): a calm stage on `surface`.
//! From 1100 px wide it has a brand panel on the left (the mark, the step title, a vertical
//! stepper) and the content card on the right; below that the stepper is a row of dots above the
//! card. The card holds one step (a headline, one sentence, the controls) and the bottom row:
//! `Skip setup` (always there), `Back` and the one forward button. The steps are in [`steps`],
//! [`notice`] and [`sources`]; the panel and the stepper in [`panel`].

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use super::kit::{filled_button, text_button, tonal_button};
use crate::gui::app::{App, Message, SLIDE_PX};
use crate::gui::assets::Icon;
use crate::gui::fx::Fx;
use crate::gui::style::{col, gap, scroll};
use crate::setup::{SetupEvent, SetupStep};
use crate::state::Event;
use crate::theme::{color, shape};

mod notice;
mod panel;
mod sources;
mod steps;

/// Window width from which the brand panel and the vertical stepper show.
const WIDE_AT: u32 = 1_100;
/// Window width from which the brand panel grows.
const WIDER_AT: u32 = 1_280;
/// Width of the brand panel below and from [`WIDER_AT`].
const PANEL_W: f32 = 264.0;
const PANEL_W_WIDER: f32 = 320.0;
/// Space around the stage, and between the panel and the card.
const STAGE_PAD: f32 = gap::LG;
/// Widest content card.
const CARD_MAX: f32 = 920.0;
/// Padding inside the card.
const CARD_PAD: f32 = gap::LG;
/// Widest text column of a step that has a single column.
const READING_W: f32 = 720.0;
/// Height of the bottom row of the card.
const FOOTER_H: f32 = 56.0;

/// The setup flow as a full-window layer.
pub fn view(app: &App) -> Element<'_, Message> {
    let (w, _) = app.window_px();
    let wide = w >= WIDE_AT;
    let step = app.ui.setup.step();
    // Window sizes are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let window_w = w as f32;
    let panel_w = if w >= WIDER_AT {
        PANEL_W_WIDER
    } else {
        PANEL_W
    };
    let free = if wide {
        window_w - 2.0 * STAGE_PAD - panel_w - STAGE_PAD
    } else {
        window_w - 2.0 * STAGE_PAD
    };
    let card_w = free.clamp(280.0, CARD_MAX);
    let body_w = card_w - 2.0 * CARD_PAD;
    let card = card(app, step, body_w);
    let stage: Element<'_, Message> = if wide {
        row![
            panel::brand_panel(app, step, panel_w),
            container(card).width(Length::Fill).center_x(Length::Fill),
        ]
        .spacing(STAGE_PAD)
        .padding(STAGE_PAD)
        .into()
    } else {
        column![
            panel::top_row(app, step),
            container(card).width(Length::Fill).center_x(Length::Fill),
        ]
        .spacing(gap::SM)
        .padding(Padding {
            top: gap::SM,
            ..Padding::new(STAGE_PAD)
        })
        .into()
    };
    container(stage)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(col(color::surface()))),
            ..container::Style::default()
        })
        .into()
}

/// The content card: the step, then the bottom row.
fn card(app: &App, step: SetupStep, body_w: f32) -> Element<'_, Message> {
    let body = match step {
        SetupStep::Welcome => steps::welcome(app, body_w.min(READING_W)),
        SetupStep::Look => steps::look(app, body_w),
        SetupStep::Notice => notice::view(app, body_w.min(READING_W)),
        SetupStep::Sources => sources::view(app, body_w),
        SetupStep::Playback => steps::playback(app, body_w.min(READING_W)),
        SetupStep::Done => steps::done(app, body_w.min(READING_W)),
    };
    let inner = column![
        container(transition(app, body))
            .width(Length::Fill)
            .height(Length::Fill),
        footer(app, step),
    ]
    .spacing(gap::SM)
    .width(Length::Fixed(body_w));
    container(inner)
        .padding(CARD_PAD)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(col(color::surface_container()))),
            border: Border {
                radius: shape::xl().into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// Slide and fade the step in (within `motion.slow`); reduced motion has no distance to cover.
fn transition<'a>(app: &App, body: Element<'a, Message>) -> Element<'a, Message> {
    let p = app.setup_progress();
    let shift = (1.0 - p) * SLIDE_PX * if app.ui.setup.forward() { 1.0 } else { -1.0 };
    // A shift is a difference between left and right padding, which moves the column.
    let pad = Padding {
        left: shift.max(0.0) * 2.0,
        right: (-shift).max(0.0) * 2.0,
        ..Padding::ZERO
    };
    let moved = container(scroll(container(body).width(Length::Fill).padding(pad)))
        .width(Length::Fill)
        .height(Length::Fill);
    Fx::new(moved)
        .passive()
        .entrance(p, 0.0, col(color::surface_container()))
        .into()
}

/// `Skip setup` on the left, `Back` and the forward button on the right.
fn footer(app: &App, step: SetupStep) -> Element<'_, Message> {
    let mut r = row![
        text_button("Skip setup", Message::Ui(Event::Setup(SetupEvent::Skip))),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .spacing(gap::SM);
    if step != SetupStep::Welcome {
        r = r.push(text_button(
            "Back",
            Message::Ui(Event::Setup(SetupEvent::Back)),
        ));
    }
    r.push(next_button(app, step))
        .height(Length::Fixed(FOOTER_H))
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
