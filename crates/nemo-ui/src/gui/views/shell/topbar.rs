//! The top bar of the centre pane: account button, history, chips, search and pane toggles.

use iced::font::Weight;
use iced::widget::{Space, button, container, row};
use iced::{Alignment, Border, Color, Element, Length, Padding};
use nemo_core::account::AccountState;
use nemo_core::track::Artwork;

use crate::gui::app::{AccountMsg, App, ArtShape, Message};
use crate::gui::assets::Icon;
use crate::gui::style::{animated, col, gap, icon, icon_button, label, label_w, spinner, tip};
use crate::gui::views::kit::art;
use crate::layout;
use crate::route::Route;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// The sign-in slot: an avatar when signed in, a "Sign in" pill otherwise.
fn account_button(app: &App) -> Element<'_, Message> {
    let open = Message::Account(AccountMsg::Open);
    match app.backend.account.state() {
        AccountState::SignedIn(info) => {
            let face: Element<'_, Message> = if info.avatar.is_empty() {
                initial_badge(app, &info.name)
            } else {
                art(
                    app,
                    &Artwork::Remote(info.avatar.clone()),
                    40,
                    ArtShape::Circle,
                )
            };
            tip(
                animated(
                    button(face).padding(0).on_press(open),
                    Color::TRANSPARENT,
                    shape::FULL,
                ),
                "Account",
            )
        }
        state => {
            let busy = matches!(state, AccountState::SigningIn);
            let ink = col(color::on_surface());
            let glyph: Element<'_, Message> = if busy {
                spinner(20.0, ink, app.spin).into()
            } else {
                icon(Icon::PersonFill, 20.0, ink).into()
            };
            let b = button(
                row![
                    glyph,
                    Space::new().width(gap::SM),
                    label_w(
                        if busy {
                            "Signing in\u{2026}"
                        } else {
                            "Sign in"
                        },
                        type_scale::BODY_MD,
                        Weight::Medium,
                        ink
                    )
                ]
                .align_y(Alignment::Center)
                .padding(Padding::from([0.0, gap::LG]))
                .height(Length::Fixed(40.0)),
            )
            .padding(0)
            .on_press(open);
            animated(b, col(color::surface_container_highest()), shape::FULL)
        }
    }
}

/// A circle with the first letter of a name (an account without a picture).
fn initial_badge<'a>(app: &App, name: &str) -> Element<'a, Message> {
    let accent = app.accent();
    let letter: String = name
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default();
    container(label_w(
        letter,
        type_scale::TITLE_SM,
        Weight::Semibold,
        col(accent.on_primary_container),
    ))
    .width(Length::Fixed(40.0))
    .height(Length::Fixed(40.0))
    .center_x(Length::Fixed(40.0))
    .center_y(Length::Fixed(40.0))
    .style(move |_| container::Style {
        background: Some(col(accent.primary_container).into()),
        border: Border {
            radius: shape::FULL.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

pub(super) fn top_bar(app: &App, center_w: u32) -> Element<'_, Message> {
    let ink = col(color::on_surface());
    let dim = col(color::on_surface_variant());
    let narrow = center_w < 820;
    let mut r = row![account_button(app)]
        .spacing(gap::SM)
        .align_y(Alignment::Center);
    if !narrow {
        r = r.push(icon_button(
            Icon::ArrowBack,
            20.0,
            36.0,
            if app.ui.nav.can_back() { ink } else { dim },
            Color::TRANSPARENT,
            app.ui.nav.can_back().then_some(Message::Ui(Event::Back)),
        ));
        r = r.push(icon_button(
            Icon::ArrowForward,
            20.0,
            36.0,
            if app.ui.nav.can_forward() { ink } else { dim },
            Color::TRANSPARENT,
            app.ui
                .nav
                .can_forward()
                .then_some(Message::Ui(Event::Forward)),
        ));
    }
    r = r.push(Space::new().width(gap::XS));
    if matches!(app.ui.route(), Route::Home) {
        r = r.push(crate::gui::views::panels::content_chips(app));
    } else if matches!(
        app.ui.route(),
        Route::Album(_) | Route::Artist(_) | Route::Playlist(_)
    ) {
        r = r.push(label(
            app.ui.route().title(),
            type_scale::TITLE_MD,
            true,
            ink,
        ));
    }
    r = r.push(Space::new().width(Length::Fill));
    let search_pill = {
        let wide = center_w >= 900;
        let inner: Element<'_, Message> = if wide {
            row![
                icon(Icon::Search, 20.0, dim),
                Space::new().width(gap::SM),
                label("Search", type_scale::BODY_MD, false, dim),
                Space::new().width(Length::Fixed(72.0)),
            ]
            .align_y(Alignment::Center)
            .padding(Padding::from([0.0, gap::LG]))
            .height(Length::Fixed(40.0))
            .into()
        } else {
            container(icon(Icon::Search, 20.0, dim))
                .width(Length::Fixed(40.0))
                .height(Length::Fixed(40.0))
                .center_x(Length::Fixed(40.0))
                .center_y(Length::Fixed(40.0))
                .into()
        };
        tip(
            animated(
                button(inner)
                    .padding(0)
                    .on_press(Message::Ui(Event::Go(Route::Search(String::new())))),
                col(color::surface_container_highest()),
                shape::FULL,
            ),
            "Search (Ctrl+K)",
        )
    };
    r = r.push(search_pill);
    r = r.push(tip(
        icon_button(
            Icon::Settings,
            22.0,
            40.0,
            ink,
            col(color::surface_container_highest()),
            Some(Message::Ui(Event::Go(Route::Settings))),
        ),
        "Settings",
    ));
    r = r.push(tip(
        icon_button(
            if app.pane_open {
                Icon::PanelClose
            } else {
                Icon::PanelOpen
            },
            22.0,
            40.0,
            ink,
            col(color::surface_container_highest()),
            Some(Message::TogglePane),
        ),
        if app.pane_open {
            "Hide Now Playing"
        } else {
            "Show Now Playing"
        },
    ));
    r.padding(Padding::from([0.0, gap::LG]))
        .height(Length::Fixed(f32::from(
            u16::try_from(layout::CENTER_BAR).unwrap_or(64),
        )))
        .into()
}
