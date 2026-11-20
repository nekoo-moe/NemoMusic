//! The hidden developer panel (dev-tools spec): an in-app overlay with a scrim, opaque
//! `surface-container-high`, five tabs as sliding chips, `Copy report` and `Clear`. Read-only:
//! nothing in it changes the app except Clear, Copy and the toggles of the panel itself.

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length, Padding};

use super::super::app::dev::{DevMsg, DevTab};
use super::super::app::{App, Message};
use super::super::assets::Icon;
use super::super::style::{col, gap, icon_button, label, scroll};
use super::kit::{sliding_chips_at, text_button, tonal_button, vspace};
use super::overlays::{card_style, modal};
use crate::theme::{color, shape, type_scale};

mod account;
mod log;
mod parts;
mod providers;
mod system;
mod trace;

/// The widest the panel gets.
const PANEL_MAX_W: f32 = 1040.0;
/// Space between the panel and the window edge.
const MARGIN: f32 = gap::XL;

/// The panel's width in the window.
fn panel_width(app: &App) -> f32 {
    (app.window.width - 2.0 * MARGIN).clamp(320.0, PANEL_MAX_W)
}

/// The width the tabs can use: the panel minus its padding and the scroll bar.
fn inner_width(app: &App) -> f32 {
    panel_width(app) - 2.0 * gap::XL - gap::MD - 8.0
}

/// The panel with its scrim.
pub fn panel(app: &App) -> Element<'_, Message> {
    let dim = col(color::on_surface_variant());
    let width = panel_width(app);
    let height = (app.window.height - 2.0 * MARGIN).max(240.0);
    let header = row![
        label(
            "Developer tools",
            type_scale::TITLE_MD,
            true,
            col(color::on_surface())
        ),
        Space::new().width(Length::Fill),
        tonal_button("Copy report", None, Some(Message::Dev(DevMsg::Copy))),
        Space::new().width(gap::SM),
        text_button("Clear", Message::Dev(DevMsg::Clear)),
        icon_button(
            Icon::Close,
            20.0,
            40.0,
            dim,
            iced::Color::TRANSPARENT,
            Some(Message::Dev(DevMsg::Close)),
        ),
    ]
    .align_y(Alignment::Center);
    let tabs = sliding_chips_at(
        app.accent(),
        &DevTab::labels(),
        app.dev.tab.index(),
        (app.dev.pill.0.value, app.dev.pill.1.value),
        |i| Message::Dev(DevMsg::Tab(i)),
    );
    let (bar, body) = match app.dev.tab {
        DevTab::Trace => trace::view(app),
        DevTab::Log => log::view(app),
        DevTab::Providers => (None, providers::view(app)),
        DevTab::Account => (None, account::view(app)),
        DevTab::System => (None, system::view(app)),
    };
    let mut card = column![header, vspace(gap::SM), tabs, vspace(gap::MD)];
    if let Some(bar) = bar {
        card = card.push(bar).push(vspace(gap::SM));
    }
    card = card.push(
        scroll(container(body).padding(Padding {
            right: gap::MD,
            ..Padding::ZERO
        }))
        .height(Length::Fill),
    );
    modal(
        container(card)
            .padding(gap::XL)
            .width(Length::Fixed(width))
            .height(Length::Fixed(height))
            .style(card_style(shape::xl())),
        app.surfaces.dev.value(),
        0.5,
        Some(Message::Dev(DevMsg::Close)),
        shape::xl(),
    )
}

#[cfg(test)]
mod tests;
