//! The Sort control above the track list of an album or playlist page, as on YouTube Music: a
//! `Sort` pill that names the order in use and opens a short menu of orders under it. The menu
//! is opaque `surface-container-high` (no glass in content), opens in the list's flow so it never
//! covers a row, and closes on a pick, on the pill, on Esc and on leaving the page.

use iced::font::Weight;
use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Border, Color, Element, Length, Padding};

use crate::collection_sort::CollectionSort;
use crate::gui::app::{App, DetailKey, Message};
use crate::gui::assets::Icon;
use crate::gui::fx::fx;
use crate::gui::style::{button_flat, col, gap, icon, label_w};
use crate::theme::{color, shape, type_scale};

/// Height of the pill and of each menu entry.
const ENTRY_H: f32 = 40.0;
/// Width of the menu.
const MENU_W: f32 = 200.0;

/// The pill text: `Sort` in the default order, else the order's name.
#[must_use]
pub fn pill_text(sort: CollectionSort) -> String {
    match sort {
        CollectionSort::Default => "Sort".to_owned(),
        other => format!("Sort: {}", other.label()),
    }
}

/// The control for page `key`: the pill, and the menu while it is open.
pub fn view<'a>(app: &'a App, key: &DetailKey, album_page: bool) -> Element<'a, Message> {
    let current = app.collection_sort(key);
    let open = app.sort_menu.as_ref() == Some(key);
    let ink = col(color::on_surface());
    let pill = button(
        row![
            label_w(pill_text(current), type_scale::BODY_MD, Weight::Medium, ink),
            Space::new().width(gap::XS),
            icon(Icon::ChevronDown, 20.0, ink),
        ]
        .align_y(Alignment::Center)
        .padding(Padding::from([0.0, gap::LG]))
        .height(Length::Fixed(ENTRY_H)),
    )
    .padding(0)
    .on_press(Message::SortMenu((!open).then(|| key.clone())));
    let pill = fx(pill.style(button_flat(
        col(color::surface_container_highest()),
        shape::FULL,
    )))
    .radius(shape::FULL);
    let mut c = column![pill];
    if open {
        c = c
            .push(Space::new().height(gap::XS))
            .push(menu(app, key, current, album_page));
    }
    c.into()
}

fn menu<'a>(
    app: &'a App,
    key: &DetailKey,
    current: CollectionSort,
    album_page: bool,
) -> Element<'a, Message> {
    let accent = app.accent();
    let mut list = column![];
    for sort in CollectionSort::offered(album_page) {
        let on = sort == current;
        let (bg, ink) = if on {
            (
                col(accent.secondary_container),
                col(accent.on_secondary_container),
            )
        } else {
            (Color::TRANSPARENT, col(color::on_surface()))
        };
        let weight = if on { Weight::Semibold } else { Weight::Normal };
        let entry = button(
            container(label_w(sort.label(), type_scale::BODY_MD, weight, ink))
                .padding(Padding::from([0.0, gap::LG]))
                .height(Length::Fixed(ENTRY_H))
                .width(Length::Fill)
                .align_y(Alignment::Center),
        )
        .padding(0)
        .width(Length::Fill)
        .on_press(Message::SortCollection {
            key: key.clone(),
            sort,
        });
        list = list.push(fx(entry.style(button_flat(bg, shape::sm()))).radius(shape::sm()));
    }
    container(list)
        .width(Length::Fixed(MENU_W))
        .padding(gap::XS)
        .style(|_| container::Style {
            background: Some(col(color::surface_container_high()).into()),
            border: Border {
                radius: shape::md().into(),
                width: 1.0,
                color: col(color::outline_variant()),
            },
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::pill_text;
    use crate::collection_sort::CollectionSort;

    #[test]
    fn the_pill_names_the_order_in_use() {
        assert_eq!(pill_text(CollectionSort::Default), "Sort");
        assert_eq!(pill_text(CollectionSort::Duration), "Sort: Duration");
    }
}
