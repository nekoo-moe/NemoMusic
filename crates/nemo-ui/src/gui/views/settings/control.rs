//! Building blocks of the settings screen: group card, rows and the segmented control.

use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Element, Length, Padding};

use crate::gui::app::{App, Message};
use crate::gui::assets::Icon;
use crate::gui::style::{animated, col, gap, icon, label};
use crate::gui::views::kit::{paragraph, tile, vspace};
use crate::settings::SettingChange;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// A group card: rows stacked on `surface-container`, radius `md`.
pub(super) fn group(rows: Vec<Element<'_, Message>>, width: f32) -> Element<'_, Message> {
    tile(
        column(rows)
            .width(Length::Fixed(width))
            .padding(Padding::from([gap::SM, 0.0])),
        color::surface_container(),
    )
}

/// A row: label and helper on the left, a control on the right; at least 56 px high.
pub(super) fn setting_row<'a>(
    width: f32,
    title: &'a str,
    helper: Option<String>,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    let control_w = 200.0;
    let text_w = (width - control_w - 2.0 * gap::LG - gap::LG).max(160.0);
    let mut text = column![label(
        title,
        type_scale::BODY_MD,
        false,
        col(color::on_surface())
    )]
    .spacing(gap::XS)
    .width(Length::Fixed(text_w));
    if let Some(h) = helper {
        text = text.push(paragraph(
            h,
            type_scale::BODY_SM,
            col(color::on_surface_variant()),
            text_w,
        ));
    }
    // The 32 px spacer plus 2 x 12 px padding makes every row at least 56 px high.
    row![
        Space::new().height(32.0),
        text,
        Space::new().width(Length::Fill),
        container(control)
            .width(Length::Fixed(control_w))
            .align_x(Alignment::End)
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([gap::MD, gap::LG]))
    .into()
}

/// A row whose control needs the full width: text on top, the control under it.
pub(super) fn stacked_row<'a>(
    width: f32,
    title: &'a str,
    helper: Option<String>,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    let inner = width - 2.0 * gap::LG;
    let mut c = column![label(
        title,
        type_scale::BODY_MD,
        false,
        col(color::on_surface())
    )]
    .spacing(gap::XS)
    .width(Length::Fixed(inner));
    if let Some(h) = helper {
        c = c.push(paragraph(
            h,
            type_scale::BODY_SM,
            col(color::on_surface_variant()),
            inner,
        ));
    }
    c = c.push(vspace(gap::SM)).push(control);
    container(c)
        .padding(Padding::from([gap::MD, gap::LG]))
        .into()
}

/// A segmented control: one pill per option, the chosen one filled with the accent container.
pub(super) fn segmented<'a, T: Copy + PartialEq + 'a>(
    app: &App,
    options: &[(T, &'static str, Option<Icon>)],
    current: T,
    change: fn(T) -> SettingChange,
) -> Element<'a, Message> {
    let accent = app.accent();
    let mut r = row![].spacing(gap::SM).align_y(Alignment::Center);
    for (value, text, glyph) in options {
        let on = *value == current;
        let (fill, ink) = if on {
            (
                col(accent.secondary_container),
                col(accent.on_secondary_container),
            )
        } else {
            (
                col(color::on_surface().with_alpha(0.06)),
                col(color::on_surface()),
            )
        };
        let mut content = row![].align_y(Alignment::Center);
        if on {
            content = content
                .push(icon(Icon::Check, 18.0, ink))
                .push(Space::new().width(gap::XS));
        } else if let Some(g) = glyph {
            content = content
                .push(icon(*g, 18.0, ink))
                .push(Space::new().width(gap::XS));
        }
        content = content.push(label(*text, type_scale::BODY_MD, on, ink));
        let b = button(
            container(content)
                .padding(Padding::from([0.0, gap::LG]))
                .height(Length::Fixed(40.0))
                .align_y(Alignment::Center),
        )
        .padding(0)
        .on_press(Message::Ui(Event::Setting(change(*value))));
        r = r.push(animated(b, fill, shape::FULL));
    }
    r.into()
}
