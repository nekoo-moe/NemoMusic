//! The notice step: the first-run notice text, the unticked checkbox, `Continue` (disabled until
//! the box is ticked) and `Not now`. Same behaviour as the first-run sheet.

use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Element, Length, Padding};

use super::super::kit::{paragraph, text_button, vspace};
use super::super::overlays::notice_checkbox;
use super::reveal::rise;
use super::steps::heading;
use crate::copy::{self, setup as text};
use crate::gui::app::{App, Message};
use crate::gui::assets::Icon;
use crate::gui::style::{animated, button_flat, col, gap, icon, label};
use crate::setup::SetupEvent;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// Height of the Continue button.
const BUTTON_H: f32 = 48.0;

pub(super) fn view(app: &App, width: f32) -> Element<'_, Message> {
    let mut c = column![heading(text::NOTICE_TITLE, None, width), vspace(gap::MD)]
        .width(Length::Fixed(width));
    for (i, p) in copy::TOS_NOTICE.into_iter().enumerate() {
        let text = paragraph(p, type_scale::BODY_MD, col(color::on_surface()), width);
        c = c.push(rise(app, i, text)).push(vspace(gap::MD));
    }
    if app.ui.settings.tos_acknowledged {
        let ink = col(color::success());
        c = c.push(
            row![
                icon(Icon::CheckCircle, 20.0, ink),
                Space::new().width(gap::SM),
                label(
                    text::NOTICE_ACCEPTED,
                    type_scale::BODY_MD,
                    true,
                    col(color::on_surface())
                ),
            ]
            .align_y(Alignment::Center),
        );
        return c.into();
    }
    let checked = app.ui.tos_checked;
    let n = copy::TOS_NOTICE.len();
    c = c
        .push(rise(app, n, notice_checkbox(checked)))
        .push(vspace(gap::LG))
        .push(rise(
            app,
            n + 1,
            row![
                continue_button(app, checked),
                Space::new().width(gap::SM),
                text_button(
                    "Not now",
                    Message::Ui(Event::Setup(SetupEvent::NoticeLater))
                ),
            ]
            .align_y(Alignment::Center)
            .into(),
        ))
        .push(vspace(gap::MD))
        .push(label(
            text::NOTICE_FOOT,
            type_scale::BODY_SM,
            false,
            col(color::on_surface_variant()),
        ));
    c.into()
}

/// `Continue`: filled in the accent once the box is ticked, flat and inert before.
fn continue_button(app: &App, checked: bool) -> Element<'_, Message> {
    let accent = app.accent();
    let ink = if checked {
        col(accent.on_primary)
    } else {
        col(color::disabled())
    };
    let b = button(
        container(label("Continue", type_scale::BODY_MD, true, ink))
            .padding(Padding::from([0.0, gap::XXL]))
            .height(Length::Fixed(BUTTON_H))
            .align_y(Alignment::Center),
    )
    .padding(0);
    if checked {
        animated(
            b.on_press(Message::Ui(Event::Setup(SetupEvent::NoticeContinue))),
            col(accent.primary),
            shape::FULL,
        )
    } else {
        b.style(button_flat(
            col(color::on_surface().with_alpha(0.12)),
            shape::FULL,
        ))
        .into()
    }
}
