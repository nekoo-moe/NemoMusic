//! The left pane: brand, navigation pills and the library list below them.

use iced::font::Weight;
use iced::widget::{Space, button, column, container, row, stack};
use iced::{Alignment, Border, Color, Element, Length, Padding};

use super::pane_surface;
use super::sidebar::{library_chips, library_header, library_list};
use crate::gui::app::{App, Message, NAV_ENTRIES};
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

    let mut entries = column![].spacing(gap::XS);
    for entry in NAV_ENTRIES {
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
        entries = entries.push(nav_pill(app, glyph, text, on, entry.route(), wide));
    }
    let nav = column![brand, vspace(gap::SM), nav_stack(app, entries.into())].spacing(gap::XS);

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

/// The entries over one selection pill that slides from entry to entry (`spatial.fast`) and
/// fades when no entry is current. The first layer sizes the stack.
fn nav_stack<'a>(app: &App, entries: Element<'a, Message>) -> Element<'a, Message> {
    let fill = col(app.accent().secondary_container).scale_alpha(app.micro.nav_on.value);
    let pill = container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(PILL_H))
        .style(move |_| container::Style {
            background: Some(fill.into()),
            border: Border {
                radius: shape::FULL.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let layer = column![
        Space::new().height(Length::Fixed(nav_pill_y(app.micro.nav.value))),
        pill
    ];
    let count = f32::from(u8::try_from(NAV_ENTRIES.len()).unwrap_or(4));
    stack![
        Space::new()
            .width(Length::Fill)
            .height(Length::Fixed(count * PILL_H + (count - 1.0) * gap::XS)),
        layer,
        entries
    ]
    .into()
}

/// Top of the pill at entry position `at` (entry steps; 1 is the second entry).
#[must_use]
fn nav_pill_y(at: f32) -> f32 {
    at.max(0.0) * (PILL_H + gap::XS)
}

/// A navigation pill: the cover's `secondary-container` sits under it when active.
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
    let el = fx(b.style(button_flat(Color::TRANSPARENT, shape::FULL)))
        .layer(ink)
        .radius(shape::FULL);
    if wide { el.into() } else { tip(el, text) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pill_sits_on_the_entry_it_follows() {
        assert!(nav_pill_y(0.0).abs() < 1e-6);
        assert!((nav_pill_y(1.0) - (PILL_H + gap::XS)).abs() < 1e-6);
        assert!((nav_pill_y(3.0) - 3.0 * (PILL_H + gap::XS)).abs() < 1e-6);
        assert!(
            nav_pill_y(-0.2).abs() < 1e-6,
            "a spring tail never goes above the first"
        );
    }
}
