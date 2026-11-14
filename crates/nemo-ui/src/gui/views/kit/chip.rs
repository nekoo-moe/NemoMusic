//! Filter chips and the row of chips with a sliding selection pill.

use iced::font::Weight;
use iced::widget::{Space, button, container, row, stack};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::gui::app::{App, Message};
use crate::gui::fx::fx;
use crate::gui::style::{button_outlined, col, gap, label_w};
use crate::palette::Palette;
use crate::theme::{color, shape, type_scale};

/// A filter chip, height 32: outlined when off, the cover's `secondary-container` when on.
pub fn chip(text: &str, selected: bool, accent: Palette, message: Message) -> Element<'_, Message> {
    chip_padded(text, selected, accent, message, gap::MD + gap::XS)
}

/// [`chip`] with its own side padding (the narrow library pane uses 12).
pub fn chip_padded(
    text: &str,
    selected: bool,
    accent: Palette,
    message: Message,
    side: f32,
) -> Element<'_, Message> {
    let (fill, ink, outline) = if selected {
        (
            col(accent.secondary_container),
            col(accent.on_secondary_container),
            None,
        )
    } else {
        (
            Color::TRANSPARENT,
            col(color::on_surface()),
            Some(col(color::outline())),
        )
    };
    let b = button(
        container(label_w(text, type_scale::LABEL_MD, Weight::Medium, ink))
            .padding(Padding::from([0.0, side]))
            .height(Length::Fixed(32.0))
            .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(message);
    fx(b.style(button_outlined(fill, outline, shape::FULL)))
        .layer(ink)
        .radius(shape::FULL)
        .into()
}

/// Side padding of a chip, and the gap between chips.
pub const CHIP_SIDE: f32 = 16.0;
/// Gap between sliding chips.
pub const CHIP_GAP: f32 = 8.0;

/// Width of a chip with this label.
#[must_use]
pub fn chip_width(text: &str) -> f32 {
    crate::gui::textfit::width(text, 12.0).ceil() + 2.0 * CHIP_SIDE
}

/// Left edge and width of chip `selected` in a row of chips with these labels.
#[must_use]
pub fn chip_geometry(labels: &[&str], selected: usize) -> (f32, f32) {
    let mut x = 0.0;
    for (i, l) in labels.iter().enumerate() {
        let w = chip_width(l);
        if i == selected {
            return (x, w);
        }
        x += w + CHIP_GAP;
    }
    (0.0, 0.0)
}

/// A row of chips with one selection pill that slides from the old chip to the new one (the
/// pill follows two springs kept in the app, slot `slot`). `pick` maps an index to a message.
pub fn sliding_chips<'a>(
    app: &App,
    labels: &[&'a str],
    selected: usize,
    slot: usize,
    pick: fn(usize) -> Message,
) -> Element<'a, Message> {
    let accent = app.accent();
    let (x, w) = {
        let (sx, sw) = &app.chip_pills[slot];
        (sx.value, sw.value)
    };
    let pill = container(Space::new())
        .width(Length::Fixed(w.max(0.0)))
        .height(Length::Fixed(32.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(col(accent.secondary_container))),
            border: Border {
                radius: shape::FULL.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let pill_layer = row![Space::new().width(Length::Fixed(x.max(0.0))), pill];
    let mut chips = row![].spacing(CHIP_GAP);
    for (i, text) in labels.iter().enumerate() {
        let on = i == selected;
        let (ink, outline) = if on {
            (col(accent.on_secondary_container), None)
        } else {
            (col(color::on_surface()), Some(col(color::outline())))
        };
        let b = button(
            container(label_w(*text, type_scale::LABEL_MD, Weight::Medium, ink))
                .width(Length::Fixed(chip_width(text)))
                .height(Length::Fixed(32.0))
                .center_x(Length::Fixed(chip_width(text)))
                .align_y(Alignment::Center),
        )
        .padding(0)
        .on_press(pick(i));
        chips = chips.push(
            fx(b.style(button_outlined(Color::TRANSPARENT, outline, shape::FULL)))
                .layer(ink)
                .radius(shape::FULL),
        );
    }
    // A stack takes the size of its first layer, so give it the width of the whole row.
    let total: f32 = labels.iter().map(|t| chip_width(t)).sum::<f32>()
        + CHIP_GAP * f32::from(u8::try_from(labels.len().saturating_sub(1)).unwrap_or(0));
    stack![pill_layer, chips]
        .width(Length::Fixed(total))
        .height(Length::Fixed(32.0))
        .into()
}
