//! The Log tab: redacted log lines, newest first, with level chips, a text filter and a pause
//! switch.

use iced::widget::{Space, column, row, text_input};
use iced::{Alignment, Background, Border, Element, Length, Padding};
use nemo_core::dev::DevLevel;

use super::parts::{Ink, cell, empty};
use crate::gui::app::dev::report::{clock, level_word};
use crate::gui::app::dev::{DevMsg, LEVELS};
use crate::gui::app::{App, Message};
use crate::gui::assets::font;
use crate::gui::style::{col, gap};
use crate::gui::views::kit::chip;
use crate::theme::{color, shape, type_scale};

const TIME_W: f32 = 92.0;
const LEVEL_W: f32 = 56.0;
const TARGET_W: f32 = 200.0;
const TEXT_W: f32 = 480.0;

/// The most lines drawn at once; the rest is in the report.
const MAX_LINES: usize = 300;

/// The filter bar and the body of the tab.
pub(super) fn view(app: &App) -> (Option<Element<'_, Message>>, Element<'_, Message>) {
    (Some(filter_bar(app)), body(app))
}

fn filter_bar(app: &App) -> Element<'_, Message> {
    let accent = app.accent();
    let dim = col(color::on_surface_variant());
    let mut chips = row![].spacing(gap::SM).align_y(Alignment::Center);
    for level in LEVELS {
        chips = chips.push(chip(
            level_name(level),
            app.dev.level == level,
            accent,
            Message::Dev(DevMsg::Level(level)),
        ));
    }
    let filter = text_input("Filter the lines", &app.dev.filter)
        .on_input(|s| Message::Dev(DevMsg::LogFilter(s)))
        .font(font(false))
        .size(f32::from(type_scale::BODY_MD.size))
        .padding(Padding::from([gap::SM, gap::MD]))
        .width(Length::Fixed(220.0))
        .style(move |_, status| {
            let focused = matches!(status, text_input::Status::Focused { .. });
            text_input::Style {
                background: Background::Color(col(color::surface_container_highest())),
                border: Border {
                    radius: shape::md().into(),
                    width: if focused { 2.0 } else { 1.0 },
                    color: if focused {
                        col(accent.primary)
                    } else {
                        col(color::outline())
                    },
                },
                icon: dim,
                placeholder: dim,
                value: col(color::on_surface()),
                selection: col(accent.primary.with_alpha(0.35)),
            }
        });
    row![
        chips,
        Space::new().width(Length::Fill),
        filter,
        Space::new().width(gap::SM),
        chip(
            "Pause",
            app.dev.paused,
            accent,
            Message::Dev(DevMsg::Pause(!app.dev.paused)),
        ),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn level_name(level: DevLevel) -> &'static str {
    match level {
        DevLevel::Error => "Error",
        DevLevel::Warn => "Warn",
        DevLevel::Info => "Info",
        DevLevel::Debug => "Debug",
        DevLevel::Trace => "Trace",
    }
}

fn body(app: &App) -> Element<'_, Message> {
    let lines = app.dev.log_lines();
    if lines.is_empty() {
        return empty("No log lines at this level.");
    }
    let mut list = column![];
    for l in lines.into_iter().take(MAX_LINES) {
        let ink = match l.level {
            DevLevel::Error => Ink::Bad,
            DevLevel::Warn => Ink::Plain,
            _ => Ink::Dim,
        };
        list = list.push(
            row![
                cell(&clock(l.at_ms), TIME_W, Ink::Dim),
                cell(level_word(l.level), LEVEL_W, ink),
                cell(&l.target, TARGET_W, Ink::Dim),
                cell(&l.text, TEXT_W, Ink::Plain),
            ]
            .spacing(gap::SM)
            .align_y(Alignment::Center),
        );
    }
    list.into()
}
