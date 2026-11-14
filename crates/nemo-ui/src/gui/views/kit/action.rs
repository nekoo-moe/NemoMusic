//! Pill buttons: filled, tonal and text.

use iced::font::Weight;
use iced::widget::{Space, button, container, row};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::gui::app::Message;
use crate::gui::assets::Icon;
use crate::gui::fx::fx;
use crate::gui::style::{animated, button_flat, col, gap, icon, label_w};
use crate::palette::Palette;
use crate::theme::{color, shape, type_scale};

/// Filled pill button in the accent colour (the one primary action of a region).
pub fn filled_button(
    text: &str,
    glyph: Icon,
    accent: Palette,
    message: Message,
) -> Element<'_, Message> {
    let b = button(
        row![
            icon(glyph, 20.0, col(accent.on_primary)),
            Space::new().width(gap::SM),
            label_w(
                text,
                type_scale::BODY_MD,
                Weight::Medium,
                col(accent.on_primary)
            ),
        ]
        .align_y(Alignment::Center)
        .padding(Padding::from([0.0, gap::XL]))
        .height(Length::Fixed(48.0)),
    )
    .padding(0)
    .on_press(message);
    fx(b.style(button_flat(col(accent.primary), shape::FULL)))
        .layer(col(accent.on_primary))
        .radius(shape::FULL)
        .into()
}

/// Tonal pill button.
pub fn tonal_button(
    text: &str,
    glyph: Option<Icon>,
    message: Option<Message>,
) -> Element<'_, Message> {
    let enabled = message.is_some();
    let ink = if enabled {
        col(color::on_surface())
    } else {
        col(color::on_surface().with_alpha(0.38))
    };
    let mut content = row![].align_y(Alignment::Center);
    if let Some(g) = glyph {
        content = content
            .push(icon(g, 20.0, ink))
            .push(Space::new().width(gap::SM));
    }
    content = content.push(label_w(text, type_scale::BODY_MD, Weight::Medium, ink));
    let b = button(
        content
            .padding(Padding::from([0.0, gap::XL]))
            .height(Length::Fixed(48.0)),
    )
    .padding(0);
    match message {
        Some(m) => animated(
            b.on_press(m),
            col(color::surface_container_highest()),
            shape::FULL,
        ),
        None => fx(b.style(button_flat(
            col(color::surface_container_high()),
            shape::FULL,
        )))
        .passive()
        .into(),
    }
}

/// Small text button (height 40).
pub fn text_button(text: &str, message: Message) -> Element<'_, Message> {
    let b = button(
        container(label_w(
            text,
            type_scale::BODY_MD,
            Weight::Medium,
            col(color::on_surface()),
        ))
        .padding(Padding::from([0.0, gap::LG]))
        .height(Length::Fixed(40.0))
        .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(message);
    animated(b, Color::TRANSPARENT, shape::FULL)
}
