//! The `Restart now` pill of the Window glass card (bar-material spec): an accent tonal pill
//! under the switch on the card's right edge. It rises 8 px and fades in with the `popup`
//! presence of the app (`motion.base`, instant under Reduce motion) while the switch needs a
//! restart, and it leaves the same way.

use iced::widget::{Space, button, column, container};
use iced::{Alignment, Element, Length};

use super::controls::SwitchMode;
use crate::copy::appearance as words;
use crate::gui::app::{App, Message};
use crate::gui::fx::Fx;
use crate::gui::style::{animated, col, gap, label};
use crate::gui::views::overlays::POPUP_RISE;
use crate::theme::{color, shape, type_scale};

/// Height of the pill: a 40 px target.
pub(super) const PILL_H: f32 = 40.0;
/// Width of the pill.
const PILL_W: f32 = 128.0;

/// The pill at presence `p` (0 hidden, 1 shown).
fn pill<'a>(app: &App, p: f32) -> Element<'a, Message> {
    let accent = app.accent();
    let ink = col(accent.on_secondary_container);
    let body = container(label(words::RESTART_PILL, type_scale::BODY_MD, true, ink))
        .width(Length::Fill)
        .height(Length::Fixed(PILL_H))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);
    let b = button(body)
        .padding(0)
        .width(Length::Fixed(PILL_W))
        .on_press(Message::Restart);
    let b = animated(b, col(accent.secondary_container), shape::FULL);
    Fx::new(b)
        .passive()
        .entrance(p, POPUP_RISE, col(color::surface_container_high()))
        .radius(shape::FULL)
        .into()
}

/// The switch with the pill's place under it. The place is always there, so the switch stays
/// where it is when the pill comes and goes.
pub(super) fn control<'a>(
    app: &App,
    switch: Element<'a, Message>,
    mode: SwitchMode,
) -> (Element<'a, Message>, f32) {
    let w = mode.box_w().max(PILL_W);
    let presence = &app.surfaces.restart;
    let slot: Element<'a, Message> = if presence.visible() {
        pill(app, presence.value())
    } else {
        Space::new().into()
    };
    let slot = container(slot)
        .width(Length::Fixed(w))
        .height(Length::Fixed(PILL_H))
        .align_x(Alignment::End);
    let control = column![switch, slot]
        .spacing(gap::SM)
        .width(Length::Fixed(w))
        .align_x(Alignment::End);
    (control.into(), w)
}
