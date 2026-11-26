//! The left pane: brand, navigation pills and the library list below them.

use iced::font::Weight;
use iced::widget::{Space, button, column, container, image, row, stack};
use iced::{Alignment, Border, Color, Element, Length, Padding};

use super::pane_surface;
use super::sidebar::{library_chips, library_header, library_list};
use crate::copy::soundcloud as text;
use crate::gui::app::{App, Message, Source};
use crate::gui::assets::{Icon, app_logo};
use crate::gui::fx::{entrance, fx};
use crate::gui::style::{button_flat, col, gap, icon, icon_button, label, label_w, tip};
use crate::gui::views::kit::{pane_bg, vspace};
use crate::route::{NavEntry, Route};
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// Height of a nav pill.
const PILL_H: f32 = 48.0;

/// Side of the brand mark in the sidebar, in px.
const MARK_SIDE: f32 = 36.0;

/// The name of the app and, while SoundCloud is the listening area, one quiet line under it.
/// Nothing under it while YouTube Music is the area. Both lines fit the 48 px of the brand row.
fn brand_text(app: &App) -> Element<'_, Message> {
    let name = label(
        "NemoMusic",
        type_scale::TITLE_SM,
        true,
        col(color::on_surface()),
    );
    if app.source != Source::SoundCloud {
        return name.into();
    }
    column![
        name,
        label(
            text::AREA_CAPTION,
            type_scale::LABEL_SM,
            false,
            col(color::on_surface_variant())
        )
    ]
    .into()
}

pub(super) fn left_pane(app: &App, width: f32, _h: u32) -> Element<'_, Message> {
    let wide = width >= 200.0;
    let route = app.ui.route();
    let selected = app.nav_selected();

    let brand: Element<'_, Message> = {
        let mark = container(
            image(app_logo())
                .width(Length::Fixed(MARK_SIDE))
                .height(Length::Fixed(MARK_SIDE)),
        )
        .width(Length::Fixed(MARK_SIDE))
        .height(Length::Fixed(MARK_SIDE));
        if wide {
            row![mark, Space::new().width(gap::MD), brand_text(app)]
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

    let lang = app.ui.settings.language;
    let mut entries = column![].spacing(gap::XS);
    for &entry in app.source.nav() {
        let on = selected == Some(entry);
        let glyph = match entry {
            NavEntry::Home => {
                if on {
                    Icon::HomeFill
                } else {
                    Icon::Home
                }
            }
            NavEntry::Explore => Icon::Explore,
            NavEntry::Search => {
                if on {
                    Icon::SearchFill
                } else {
                    Icon::Search
                }
            }
            _ => {
                if on {
                    Icon::LibraryFill
                } else {
                    Icon::Library
                }
            }
        };
        let text = entry.label_localized(lang);
        entries = entries.push(nav_pill(app, glyph, text, on, entry.route(), wide));
    }
    let nav = column![brand, vspace(gap::SM), nav_stack(app, entries.into())].spacing(gap::XS);

    let mut c = column![nav].width(Length::Fill);
    if wide {
        c = c
            .push(vspace(gap::LG))
            .push(library_header(app))
            .push(vspace(gap::SM));
        c = c
            .push(library_chips(app))
            .push(vspace(gap::SM))
            .push(entrance(
                library_list(app, width - 2.0 * gap::MD, route),
                app.anim.list(),
                pane_bg(),
            ));
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
                crate::i18n::t(lang, "Settings", "Cài đặt"),
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
        col(color::surface_container().with_alpha(crate::scheme::pane_alpha())).into(),
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
    let count = f32::from(u8::try_from(app.source.nav().len()).unwrap_or(4));
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

#[cfg(test)]
mod mark_tests {
    use super::*;

    #[test]
    fn the_sidebar_mark_has_a_positive_size_and_loads_the_app_logo() {
        const { assert!(MARK_SIDE > 0.0) };
        let _ = app_logo();
    }
}
