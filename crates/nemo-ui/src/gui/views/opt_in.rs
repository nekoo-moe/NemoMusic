//! `OptInRow`: the one settings row of every opt-in feature (ADR 0004, "Opt-in settings
//! pattern", rules 3, 6, 7 and 9).
//!
//! The row draws the title and the switch, then "What is sent", "To" and "Risk" from an
//! [`OptInInfo`]. Turning the switch on does not turn the feature on: the caller moves the row
//! to [`OptInPhase::Confirming`], where a panel with the same three texts offers "Turn on" and
//! "Cancel", drawn alike so neither is pre-selected. Turning off is one click with no panel; the
//! row then says what stays on this computer and can offer "Delete now". After turn-on it shows
//! the hosts in use and one status line.
//!
//! The row knows no service by name: the caller passes the info, the phase and the messages.
//! Every control is wrapped in [`fx`], so Tab reaches it in drawing order with the focus ring of
//! `design.md` section 5, and Enter or Space presses it. The row has no animation of its own;
//! the shared state layers jump under reduced motion. Texts wrap instead of clipping.

use iced::widget::{Space, column, container, toggler};
use iced::{Alignment, Element, Length, Padding};
use nemo_core::opt_in::OptInInfo;

use super::kit::{paragraph, tile, tonal_button};
use super::settings::switch_style;
use crate::copy;
use crate::gui::app::Message;
use crate::gui::fx::fx;
use crate::gui::style::{col, gap, label};
use crate::theme::{color, shape, type_scale};

/// Width kept for the switch and its `On` / `Off` text.
const SWITCH_W: f32 = 96.0;
/// Narrowest text column, so a very narrow window still wraps instead of clipping.
const MIN_TEXT_W: f32 = 160.0;

/// Where an opt-in row stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptInPhase {
    /// Off: nothing is requested, built or started.
    Off,
    /// The user flipped the switch; the panel waits for "Turn on" or "Cancel". Still off.
    Confirming,
    /// On and confirmed.
    On,
}

/// What one row shows besides its [`OptInInfo`].
#[derive(Debug, Clone, Copy)]
pub struct OptInRow<'a> {
    /// What the feature sends, to which hosts, and its risk.
    pub info: &'a OptInInfo,
    /// Where the row stands.
    pub phase: OptInPhase,
    /// One status line while on, such as "Connected" or "yt-dlp missing".
    pub status: Option<&'a str>,
    /// While off: what stays on this computer, such as "Your scrobble queue stays here."
    pub stays: Option<&'a str>,
}

/// The messages the row's controls send. The caller builds them; the row only draws.
#[derive(Debug, Clone)]
pub struct OptInActions {
    /// The switch was turned on: open the panel. The feature stays off.
    pub ask: Message,
    /// "Turn on" in the panel.
    pub turn_on: Message,
    /// "Cancel" in the panel.
    pub cancel: Message,
    /// The switch was turned off: one click, no panel.
    pub turn_off: Message,
    /// "Delete now" while off, when the feature kept data on this computer.
    pub delete_now: Option<Message>,
}

/// A control the keyboard reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptInControl {
    /// The switch.
    Switch,
    /// "Turn on" in the panel.
    TurnOn,
    /// "Cancel" in the panel.
    Cancel,
    /// "Delete now".
    DeleteNow,
}

/// The controls of a row in Tab order, which is the order they are drawn in.
#[must_use]
pub fn controls(row: &OptInRow<'_>, actions: &OptInActions) -> Vec<OptInControl> {
    let mut out = vec![OptInControl::Switch];
    match row.phase {
        OptInPhase::Confirming => out.extend([OptInControl::TurnOn, OptInControl::Cancel]),
        OptInPhase::Off if actions.delete_now.is_some() => out.push(OptInControl::DeleteNow),
        OptInPhase::Off | OptInPhase::On => {}
    }
    out
}

/// The hosts line: the exact hosts, or that nothing leaves the computer.
#[must_use]
pub fn hosts_text(info: &OptInInfo) -> String {
    if info.hosts.is_empty() {
        copy::OPT_IN_NO_HOSTS.to_owned()
    } else {
        info.hosts.join(", ")
    }
}

/// Draw one opt-in row `width` px wide.
pub fn opt_in_row(row: OptInRow<'_>, actions: OptInActions, width: f32) -> Element<'_, Message> {
    let inner = (width - 2.0 * gap::LG).max(MIN_TEXT_W);
    let on = row.phase == OptInPhase::On;
    let header = iced::widget::row![
        paragraph(
            row.info.title,
            type_scale::BODY_MD,
            col(color::on_surface()),
            (inner - SWITCH_W - gap::LG).max(MIN_TEXT_W / 2.0),
        ),
        Space::new().width(Length::Fill),
        switch(on, actions.ask.clone(), actions.turn_off.clone()),
    ]
    .align_y(Alignment::Center);
    let mut body = column![header].spacing(gap::MD);
    let delete_now = actions.delete_now.clone();
    match row.phase {
        OptInPhase::Confirming => body = body.push(panel(row.info, actions, inner)),
        OptInPhase::On => {
            body = body.push(facts(row.info, inner));
            if let Some(status) = row.status {
                body = body.push(paragraph(
                    status,
                    type_scale::BODY_SM,
                    col(color::on_surface_variant()),
                    inner,
                ));
            }
        }
        OptInPhase::Off => {
            body = body.push(facts(row.info, inner));
            if let Some(stays) = row.stays {
                body = body.push(paragraph(
                    stays,
                    type_scale::BODY_SM,
                    col(color::on_surface_variant()),
                    inner,
                ));
            }
            if let Some(message) = delete_now {
                body = body.push(tonal_button(copy::OPT_IN_DELETE_NOW, None, Some(message)));
            }
        }
    }
    container(body)
        .padding(Padding::from([gap::MD, gap::LG]))
        .width(Length::Fixed(width))
        .into()
}

/// The switch with its `On` / `Off` text (state is never colour alone). Only the toggle is
/// focusable, so Enter or Space lands on it.
fn switch<'a>(on: bool, ask: Message, turn_off: Message) -> Element<'a, Message> {
    let toggle = toggler(on)
        .on_toggle(move |v| if v { ask.clone() } else { turn_off.clone() })
        .size(24.0)
        .style(switch_style(on));
    let target = container(toggle)
        .height(Length::Fixed(40.0))
        .align_y(Alignment::Center);
    iced::widget::row![
        label(
            if on { "On" } else { "Off" },
            type_scale::BODY_MD,
            false,
            col(color::on_surface_variant()),
        ),
        Space::new().width(gap::MD),
        fx(target).radius(shape::FULL),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// "What is sent", "To" and "Risk", each a heading over its wrapped text.
fn facts<'a>(info: &'a OptInInfo, width: f32) -> Element<'a, Message> {
    let fact = |heading: &'static str, text: String| -> Element<'a, Message> {
        column![
            label(
                heading,
                type_scale::LABEL_MD,
                true,
                col(color::on_surface_variant()),
            ),
            paragraph(text, type_scale::BODY_MD, col(color::on_surface()), width),
        ]
        .spacing(gap::XS)
        .into()
    };
    column![
        fact(copy::OPT_IN_SENDS, info.sends.to_owned()),
        fact(copy::OPT_IN_TO, hosts_text(info)),
        fact(copy::OPT_IN_RISK, info.risk.to_owned()),
    ]
    .spacing(gap::MD)
    .into()
}

/// The confirmation panel: the same three texts, then "Turn on" and "Cancel" drawn alike.
fn panel(info: &OptInInfo, actions: OptInActions, width: f32) -> Element<'_, Message> {
    let text_w = (width - 2.0 * gap::LG).max(MIN_TEXT_W);
    let buttons = iced::widget::row![
        tonal_button(copy::OPT_IN_TURN_ON, None, Some(actions.turn_on)),
        tonal_button(copy::OPT_IN_CANCEL, None, Some(actions.cancel)),
    ]
    .spacing(gap::SM);
    tile(
        column![facts(info, text_w), buttons]
            .spacing(gap::LG)
            .padding(gap::LG)
            .width(Length::Fixed(width)),
        color::surface_container_high(),
    )
}

#[cfg(test)]
mod tests;
