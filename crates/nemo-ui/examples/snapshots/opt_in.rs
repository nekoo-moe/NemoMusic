//! Shots of the shared opt-in row (integrations task 2): off, panel open, on, and the panel in a
//! narrow column.

use iced::widget::container;
use iced::{Background, Element, Length};
use nemo_core::opt_in::OptInInfo;
use nemo_ui::gui::app::{Message, SettingsSection};
use nemo_ui::gui::style::{col, gap};
use nemo_ui::gui::views::kit::tile;
use nemo_ui::gui::views::opt_in::{OptInActions, OptInPhase, OptInRow, opt_in_row};
use nemo_ui::theme::color;

/// A sample feature for the shots, worded like the Last.fm row of the integrations design.
const SAMPLE: OptInInfo = OptInInfo {
    id: "lastfm",
    title: "Last.fm scrobbling",
    sends: "The songs you play, with the time you played them.",
    hosts: &["ws.audioscrobbler.com"],
    risk: "Your plays show on your public Last.fm profile.",
    version: 1,
    default_on: false,
};

/// The row on a settings group card, on the window surface.
fn framed(phase: OptInPhase, width: f32) -> Element<'static, Message> {
    let actions = OptInActions {
        ask: Message::SettingsSection(SettingsSection::Privacy),
        turn_on: Message::SettingsSection(SettingsSection::Privacy),
        cancel: Message::SettingsSection(SettingsSection::Privacy),
        turn_off: Message::SettingsSection(SettingsSection::Privacy),
        delete_now: Some(Message::SettingsSection(SettingsSection::Privacy)),
    };
    let row = OptInRow {
        info: &SAMPLE,
        phase,
        status: Some("Connected as your Last.fm account"),
        stays: Some("Plays not sent yet stay in the queue on this computer."),
    };
    container(tile(
        opt_in_row(row, actions, width),
        color::surface_container(),
    ))
    .padding(gap::XL)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
        background: Some(Background::Color(col(color::surface()))),
        ..container::Style::default()
    })
    .into()
}

/// `(name, view, window size)` of every opt-in row shot.
pub(super) fn shots() -> Vec<(String, Element<'static, Message>, (u32, u32))> {
    vec![
        (
            "opt-in-off".to_owned(),
            framed(OptInPhase::Off, 640.0),
            (688, 520),
        ),
        (
            "opt-in-panel-open".to_owned(),
            framed(OptInPhase::Confirming, 640.0),
            (688, 560),
        ),
        (
            "opt-in-on".to_owned(),
            framed(OptInPhase::On, 640.0),
            (688, 460),
        ),
        (
            "opt-in-panel-narrow".to_owned(),
            framed(OptInPhase::Confirming, 320.0),
            (368, 760),
        ),
    ]
}
