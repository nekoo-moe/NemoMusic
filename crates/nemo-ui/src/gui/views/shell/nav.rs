//! The left pane: brand, navigation pills and the library list below them.

use iced::font::Weight;
use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Border, Color, Element, Length, Padding};

use super::pane_surface;
use super::sidebar::{library_chips, library_header, library_list};
use crate::gui::app::{App, Message};
use crate::gui::assets::Icon;
use crate::gui::fx::fx;
use crate::gui::style::{button_flat, col, gap, icon, icon_button, label, label_w, tip};
use crate::gui::views::kit::vspace;
use crate::route::{NavEntry, Route};
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// Height of a nav pill.
const PILL_H: f32 = 48.0;

pub(super) fn left_pane(app: &App, width: f32, _h: u32) -> Element<'_, Message> {
    let wide = width >= 200.0;
    let route = app.ui.route();
    let selected = app.ui.nav.active_entry();
    let accent = app.accent();

    let brand: Element<'_, Message> = {
        let mark = container(icon(Icon::Note, 20.0, col(accent.on_primary)))
            .width(Length::Fixed(36.0))
            .height(Length::Fixed(36.0))
            .center_x(Length::Fixed(36.0))
            .center_y(Length::Fixed(36.0))
            .style(move |_| container::Style {
                background: Some(col(accent.primary).into()),
                border: Border {
                    radius: shape::md().into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });
        if wide {
            row![
                mark,
                Space::new().width(gap::MD),
                label(
                    "NemoMusic",
                    type_scale::TITLE_SM,
                    true,
                    col(color::on_surface())
                )
            ]
            .align_y(Alignment::Center)
            .padding(Padding::from([0.0, gap::SM]))
            .height(Length::Fixed(48.0))
            .into()
        } else {
            container(mark)
                .width(Length::Fill)
                .center_x(Length::Fill)
                .height(Length::Fixed(48.0))
                .into()
        }
    };

    let mut nav = column![brand, vspace(gap::SM)].spacing(gap::XS);
    for entry in [
        NavEntry::Home,
        NavEntry::Explore,
        NavEntry::Search,
        NavEntry::Library,
    ] {
        let on = selected == Some(entry);
        let (glyph, text) = match entry {
            NavEntry::Home => (if on { Icon::HomeFill } else { Icon::Home }, "Home"),
            NavEntry::Explore => (Icon::Explore, "Explore"),
            NavEntry::Search => (if on { Icon::SearchFill } else { Icon::Search }, "Search"),
            _ => (
                if on { Icon::LibraryFill } else { Icon::Library },
                "Library",
            ),
        };
        nav = nav.push(nav_pill(app, glyph, text, on, entry.route(), wide));
    }

    let mut c = column![nav].width(Length::Fill);
    if wide {
        c = c
            .push(vspace(gap::LG))
            .push(library_header())
            .push(vspace(gap::SM))
            .push(library_chips(app))
            .push(vspace(gap::SM))
            .push(library_list(app, width - 2.0 * gap::MD, route));
    } else {
        c = c.push(Space::new().height(Length::Fill)).push(
            container(tip(
                icon_button(
                    Icon::Settings,
                    24.0,
                    48.0,
                    col(color::on_surface_variant()),
                    Color::TRANSPARENT,
                    Some(Message::Ui(Event::Go(Route::Settings))),
                ),
                "Settings",
            ))
            .width(Length::Fill)
            .center_x(Length::Fill)
            .padding(Padding {
                bottom: crate::gui::views::kit::PAGE_BOTTOM - gap::SM,
                ..Padding::ZERO
            }),
        );
    }
    pane_surface(
        c.padding(Padding::from([gap::MD, gap::MD])),
        Length::Fixed(width),
        col(color::surface_container().with_alpha(crate::scheme::PANE_ALPHA)).into(),
    )
    .into()
}

/// A navigation pill: filled with the cover's `secondary-container` when active.
fn nav_pill<'a>(
    app: &App,
    glyph: Icon,
    text: &'static str,
    on: bool,
    target: Route,
    wide: bool,
) -> Element<'a, Message> {
    let accent = app.accent();
    let ink = if on {
        col(accent.on_secondary_container)
    } else {
        col(color::on_surface_variant())
    };
    let idle = if on {
        col(accent.secondary_container)
    } else {
        Color::TRANSPARENT
    };
    let inner: Element<'_, Message> = if wide {
        row![
            icon(glyph, 24.0, ink),
            Space::new().width(gap::MD),
            label_w(
                text,
                type_scale::BODY_MD,
                if on { Weight::Semibold } else { Weight::Medium },
                if on { ink } else { col(color::on_surface()) }
            )
        ]
        .align_y(Alignment::Center)
        .padding(Padding::from([0.0, gap::LG]))
        .height(Length::Fixed(PILL_H))
        .into()
    } else {
        container(icon(glyph, 24.0, ink))
            .width(Length::Fill)
            .height(Length::Fixed(PILL_H))
            .center_x(Length::Fill)
            .center_y(Length::Fixed(PILL_H))
            .into()
    };
    let b = button(inner)
        .padding(0)
        .width(Length::Fill)
        .on_press(Message::Ui(Event::Go(target)));
    let el = fx(b.style(button_flat(idle, shape::FULL)))
        .layer(ink)
        .radius(shape::FULL);
    if wide { el.into() } else { tip(el, text) }
}
