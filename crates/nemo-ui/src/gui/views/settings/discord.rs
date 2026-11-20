//! The Privacy section with the Discord presence row (discord-presence spec, criterion 20): the
//! hosts list, then the shared opt-in row (what is sent, to which hosts, the risk, `Turn on` in
//! a panel with no pre-selected button) and the switch that hides the song names.

use iced::widget::column;
use iced::{Element, Length};

use super::control::{group, setting_row};
use super::switch_with;
use crate::copy::discord as text;
use crate::gui::app::{App, DiscordMsg, Message};
use crate::gui::style::gap;
use crate::gui::views::opt_in::{OptInActions, OptInPhase, OptInRow, opt_in_row};

/// The Privacy rows, and the Discord card under them when the app offers Discord presence.
pub(super) fn page<'a>(
    app: &'a App,
    rows: Vec<Element<'a, Message>>,
    width: f32,
) -> Element<'a, Message> {
    // The switches come first: they are what the user opens Privacy for; the list of hosts
    // follows.
    let mut page = column![].spacing(gap::LG).width(Length::Fixed(width));
    if let Some(card) = card(app, width) {
        page = page.push(card);
    }
    page.push(group(rows, width)).into()
}

fn phase(app: &App) -> OptInPhase {
    if app.discord_confirming {
        OptInPhase::Confirming
    } else if app.discord_on() {
        OptInPhase::On
    } else {
        OptInPhase::Off
    }
}

fn actions() -> OptInActions {
    OptInActions {
        ask: Message::Discord(DiscordMsg::Ask),
        turn_on: Message::Discord(DiscordMsg::TurnOn),
        cancel: Message::Discord(DiscordMsg::Cancel),
        turn_off: Message::Discord(DiscordMsg::TurnOff),
        delete_now: None,
    }
}

fn card(app: &App, width: f32) -> Option<Element<'_, Message>> {
    let info = app.discord_info.as_ref()?;
    let phase = phase(app);
    let row = OptInRow {
        info,
        phase,
        status: Some(text::STATUS_ON),
        stays: Some(text::STAYS),
    };
    let hide = setting_row(
        width,
        text::HIDE_TITLE,
        Some(text::HIDE_HELPER.to_owned()),
        switch_with(app.discord_hidden(), |on| {
            Message::Discord(DiscordMsg::Hide(on))
        }),
    );
    Some(group(vec![opt_in_row(row, actions(), width), hide], width))
}
