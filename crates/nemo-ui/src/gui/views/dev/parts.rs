//! Small pieces the tabs of the developer panel share: fixed-width cells, key and value cards,
//! and the table header.

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Border, Color, Element, Length, Padding};

use crate::gui::app::Message;
use crate::gui::style::{col, fit, gap, label};
use crate::theme::{color, shape, type_scale};

/// Height of one list row and one key and value line.
pub(super) const ROW_H: f32 = 32.0;

/// Width of the key column of a card.
pub(super) const KEY_W: f32 = 168.0;

/// How a cell is inked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ink {
    /// Normal text.
    Plain,
    /// Quiet text (a header, a missing value).
    Dim,
    /// A failure.
    Bad,
}

pub(super) fn ink_color(ink: Ink) -> Color {
    match ink {
        Ink::Plain => col(color::on_surface()),
        Ink::Dim => col(color::on_surface_variant()),
        Ink::Bad => col(color::error()),
    }
}

/// One cell of `width` px: the text is cut with an ellipsis to fit.
pub(super) fn cell<'a>(text: &str, width: f32, ink: Ink) -> Element<'a, Message> {
    let shown = fit(text, width, type_scale::LABEL_MD.size);
    container(label(shown, type_scale::LABEL_MD, false, ink_color(ink)))
        .width(Length::Fixed(width))
        .height(Length::Fixed(ROW_H))
        .align_y(Alignment::Center)
        .into()
}

/// A table row of cells, 8 px apart.
pub(super) fn table_row(cells: Vec<Element<'_, Message>>) -> Element<'_, Message> {
    row(cells)
        .spacing(gap::SM)
        .align_y(Alignment::Center)
        .into()
}

/// A line of `text` in the body text style.
pub(super) fn line<'a>(text: impl Into<String>, ink: Ink) -> Element<'a, Message> {
    container(label(
        text.into(),
        type_scale::BODY_MD,
        false,
        ink_color(ink),
    ))
    .height(Length::Fixed(ROW_H))
    .align_y(Alignment::Center)
    .into()
}

/// A section title.
pub(super) fn title<'a>(text: &str) -> Element<'a, Message> {
    container(label(
        text.to_owned(),
        type_scale::TITLE_SM,
        true,
        col(color::on_surface()),
    ))
    .padding(Padding {
        top: gap::SM,
        ..Padding::ZERO
    })
    .height(Length::Fixed(ROW_H + gap::SM))
    .align_y(Alignment::Center)
    .into()
}

/// A card of key and value lines on `surface-container-highest`.
pub(super) fn card<'a>(
    heading: &str,
    lines: Vec<(String, String, Ink)>,
    width: f32,
) -> Element<'a, Message> {
    let value_w = (width - KEY_W - 2.0 * gap::LG - gap::SM).max(80.0);
    let mut body = column![label(
        heading.to_owned(),
        type_scale::BODY_MD,
        true,
        col(color::on_surface())
    )]
    .spacing(gap::XS);
    for (key, value, ink) in lines {
        body = body.push(
            row![cell(&key, KEY_W, Ink::Dim), cell(&value, value_w, ink)]
                .spacing(gap::SM)
                .align_y(Alignment::Center),
        );
    }
    container(body)
        .padding(gap::LG)
        .width(Length::Fixed(width))
        .style(|_| container::Style {
            background: Some(col(color::surface_container_highest()).into()),
            border: Border {
                radius: shape::md().into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// A card whose lines run in two columns: the first half on the left, the rest on the right.
pub(super) fn card_two<'a>(
    heading: &str,
    lines: Vec<(String, String, Ink)>,
    width: f32,
) -> Element<'a, Message> {
    let half = lines.len().div_ceil(2);
    let column_w = (width - 2.0 * gap::LG - gap::XL) / 2.0;
    let key_w = 132.0;
    let value_w = (column_w - key_w - gap::SM).max(60.0);
    let mut left = column![].spacing(gap::XS);
    let mut right = column![].spacing(gap::XS);
    for (i, (key, value, ink)) in lines.into_iter().enumerate() {
        let line = row![cell(&key, key_w, Ink::Dim), cell(&value, value_w, ink)]
            .spacing(gap::SM)
            .align_y(Alignment::Center);
        if i < half {
            left = left.push(line);
        } else {
            right = right.push(line);
        }
    }
    let body = column![
        label(
            heading.to_owned(),
            type_scale::BODY_MD,
            true,
            col(color::on_surface())
        ),
        row![left, Space::new().width(gap::XL), right],
    ]
    .spacing(gap::XS);
    container(body)
        .padding(gap::LG)
        .width(Length::Fixed(width))
        .style(|_| container::Style {
            background: Some(col(color::surface_container_highest()).into()),
            border: Border {
                radius: shape::md().into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// The text that shows an unknown fact.
pub(super) const UNKNOWN: &str = "Unknown";

/// A placeholder line for an empty list.
pub(super) fn empty<'a>(text: &str) -> Element<'a, Message> {
    column![
        Space::new().height(gap::LG),
        line(text.to_owned(), Ink::Dim)
    ]
    .into()
}
