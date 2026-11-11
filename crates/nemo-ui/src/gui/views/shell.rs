//! Shell: three rounded panes on `surface-container` (navigation and library, content, Now
//! Playing) with 8 px gaps, the floating capsule on top, and the right pane's overlay mode below
//! 1280 px. `docs/ui-redesign.md` section 2 has the numbers.

use iced::font::Weight;
use iced::widget::{Space, button, column, container, row, stack};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};
use nemo_core::account::AccountState;
use nemo_core::catalog::{AlbumSummary, ArtistSummary, PlaylistSummary, ShelfItem};
use nemo_core::track::Artwork;

use super::super::app::{AccountMsg, App, ArtShape, LibFilter, Message};
use super::super::assets::Icon;
use super::super::fx::{entrance, fx};
use super::super::style::{
    animated, button_flat, col, fit, gap, icon, icon_button, label, label_w, scroll, spinner, tip,
};
use super::kit::{art, chip_padded, pane_bg, vspace};
use super::{bar, detail, explore, home, library, pane, search, settings};
use crate::format;
use crate::layout;
use crate::route::{NavEntry, Route};
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// Height of a nav pill.
const PILL_H: f32 = 48.0;
/// Height of a library row.
const LIB_ROW_H: f32 = 64.0;

/// A rounded pane background.
pub fn pane_surface<'a>(
    content: impl Into<Element<'a, Message>>,
    width: Length,
    bg: Background,
) -> iced::widget::Container<'a, Message> {
    container(content)
        .width(width)
        .height(Length::Fill)
        .clip(true)
        .style(move |_| container::Style {
            background: Some(bg),
            border: Border {
                radius: shape::scaled(layout::PANE_RADIUS).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
}

/// The window below the notices: the panes and the capsule.
pub fn view(app: &App, w: u32, h: u32) -> Element<'_, Message> {
    // Pane numbers are small, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let (left_w, right_full) = (layout::left_width(w) as f32, layout::right_width(w) as f32);
    let pushes = layout::right_pushes(w);
    let open = app.anim.pane.value.max(0.0);
    let center_w = layout::center_width(w, open);

    let mut body = row![left_pane(app, left_w, h), center_pane(app, center_w, w, h)]
        .spacing(gap::SM)
        .width(Length::Fill)
        .height(Length::Fill);
    // The panes end above the capsule while it shows, so it never covers a row.
    let capsule_p = app.anim.capsule.value.clamp(0.0, 1.0);
    // A small constant, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let reserve = layout::PANE_BOTTOM_RESERVE as f32 * capsule_p;
    if pushes && open > 0.002 {
        body = body.push(pane::view(
            app,
            right_full,
            Length::Fixed((right_full * open.min(1.0)).round()),
        ));
    }
    let mut layers: Vec<Element<'_, Message>> = vec![
        container(body)
            .padding(Padding {
                bottom: gap::SM + reserve,
                ..Padding::new(gap::SM)
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    ];
    if !pushes && app.pane_open {
        layers.push(
            container(entrance(
                pane::view(app, right_full, Length::Fixed(right_full)),
                app.anim.pane.value.clamp(0.0, 1.0),
                pane_bg(),
            ))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::End)
            .padding(Padding {
                bottom: gap::SM + reserve,
                ..Padding::new(gap::SM)
            })
            .into(),
        );
    }
    if app.ui.snapshot.current.is_some() || app.anim.capsule.value > 0.01 {
        // Capsule widths are small, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let cap_w = layout::capsule_width(w) as f32;
        let p = app.anim.capsule.value.clamp(0.0, 1.0);
        // A small constant, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let capsule_margin = layout::CAPSULE_MARGIN as f32;
        layers.push(
            container(
                super::super::fx::Fx::new(bar::view(app, cap_w))
                    .passive()
                    .entrance(p, 28.0, Color::TRANSPARENT),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::End)
            .center_x(Length::Fill)
            .padding(Padding {
                bottom: capsule_margin,
                ..Padding::ZERO
            })
            .into(),
        );
    }
    stack(layers).into()
}

// ---------------------------------------------------------------------------------------------
// Left pane
// ---------------------------------------------------------------------------------------------

fn left_pane(app: &App, width: f32, _h: u32) -> Element<'_, Message> {
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
                bottom: super::kit::PAGE_BOTTOM - gap::SM,
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

fn library_header<'a>() -> Element<'a, Message> {
    row![
        container(icon(Icon::Library, 24.0, col(color::on_surface_variant())))
            .padding(Padding::from([0.0, gap::SM])),
        label(
            "Your library",
            type_scale::TITLE_SM,
            true,
            col(color::on_surface())
        ),
        Space::new().width(Length::Fill),
        tip(
            icon_button(
                Icon::Add,
                20.0,
                40.0,
                col(color::on_surface_variant()),
                Color::TRANSPARENT,
                Some(Message::Create(super::super::app::CreateMsg::Open)),
            ),
            "Create playlist",
        ),
        tip(
            icon_button(
                Icon::ArrowForward,
                20.0,
                40.0,
                col(color::on_surface_variant()),
                Color::TRANSPARENT,
                Some(Message::Ui(Event::Go(Route::Library))),
            ),
            "Open library",
        ),
    ]
    .align_y(Alignment::Center)
    .height(Length::Fixed(40.0))
    .into()
}

fn library_chips(app: &App) -> Element<'_, Message> {
    let accent = app.accent();
    let mut r = row![].spacing(gap::SM);
    for f in LibFilter::ALL {
        let on = app.lib_filter == Some(f);
        r = r.push(chip_padded(
            f.label(),
            on,
            accent,
            Message::LibFilter(if on { None } else { Some(f) }),
            gap::MD,
        ));
    }
    r.into()
}

/// One row of the library list.
struct LibRow {
    kind: LibFilter,
    title: String,
    subtitle: String,
    artwork: Artwork,
    route: Route,
}

fn library_rows(app: &App) -> Vec<LibRow> {
    let mut playlists: Vec<LibRow> = Vec::new();
    let mut artists: Vec<LibRow> = Vec::new();
    let mut albums: Vec<LibRow> = Vec::new();
    let album_row = |a: &AlbumSummary| LibRow {
        kind: LibFilter::Albums,
        title: a.title.clone(),
        subtitle: format!("Album \u{b7} {}", format::artists(&a.artists)),
        artwork: Artwork::Remote(a.thumbnails.clone()),
        route: Route::Album(a.id.clone()),
    };
    let artist_row = |a: &ArtistSummary| LibRow {
        kind: LibFilter::Artists,
        title: a.name.clone(),
        subtitle: "Artist".to_owned(),
        artwork: Artwork::Remote(a.thumbnails.clone()),
        route: Route::Artist(a.id.clone()),
    };
    let playlist_row = |p: &PlaylistSummary| LibRow {
        kind: LibFilter::Playlists,
        title: p.title.clone(),
        subtitle: format!(
            "Playlist \u{b7} {}",
            p.author
                .clone()
                .unwrap_or_else(|| "YouTube Music".to_owned())
        ),
        artwork: Artwork::Remote(p.thumbnails.clone()),
        route: Route::Playlist(p.id.clone()),
    };
    for a in &app.local_albums {
        albums.push(album_row(a));
    }
    for item in app.home.shelves.iter().flat_map(|s| s.items.iter()) {
        match item {
            ShelfItem::Playlist(p) => playlists.push(playlist_row(p)),
            ShelfItem::Artist(a) => artists.push(artist_row(a)),
            ShelfItem::Album(a) => albums.push(album_row(a)),
            ShelfItem::Song(_) => {}
        }
    }
    let mut rows = Vec::new();
    for (kind, list) in [
        (LibFilter::Playlists, playlists),
        (LibFilter::Artists, artists),
        (LibFilter::Albums, albums),
    ] {
        if app.lib_filter.is_none() || app.lib_filter == Some(kind) {
            rows.extend(list);
        }
    }
    let mut seen: Vec<Route> = Vec::new();
    rows.retain(|r| {
        if seen.contains(&r.route) {
            false
        } else {
            seen.push(r.route.clone());
            true
        }
    });
    rows.truncate(40);
    rows
}

fn library_list<'a>(app: &'a App, width: f32, route: &Route) -> Element<'a, Message> {
    let rows = library_rows(app);
    if rows.is_empty() {
        return container(
            label(
                "Search for music and it shows up here.",
                type_scale::BODY_SM,
                false,
                col(color::on_surface_variant()),
            )
            .wrapping(iced::widget::text::Wrapping::Word)
            .width(Length::Fixed(width - 2.0 * gap::MD)),
        )
        .padding(gap::MD)
        .into();
    }
    let text_w = width - 48.0 - gap::MD - 2.0 * gap::SM;
    let mut list = column![].spacing(gap::XS);
    for (i, r) in rows.into_iter().enumerate() {
        let on = *route == r.route;
        let shape_ = if r.kind == LibFilter::Artists {
            ArtShape::Circle
        } else {
            ArtShape::Thumb
        };
        let line = row![
            art(app, &r.artwork, 48, shape_),
            Space::new().width(gap::MD),
            column![
                label(
                    fit(&r.title, text_w, type_scale::BODY_MD.size),
                    type_scale::BODY_MD,
                    true,
                    col(color::on_surface())
                ),
                label(
                    fit(&r.subtitle, text_w, type_scale::BODY_SM.size),
                    type_scale::BODY_SM,
                    false,
                    col(color::on_surface_variant())
                ),
            ]
            .width(Length::Fixed(text_w)),
        ]
        .align_y(Alignment::Center);
        let b = button(
            container(line)
                .width(Length::Fill)
                .height(Length::Fixed(LIB_ROW_H))
                .padding(Padding::from([0.0, gap::SM]))
                .align_y(Alignment::Center),
        )
        .padding(0)
        .width(Length::Fill)
        .on_press(Message::Ui(Event::Go(r.route)));
        let idle = if on {
            col(color::surface_container_high())
        } else {
            Color::TRANSPARENT
        };
        list = list.push(entrance(
            animated(b, idle, shape::lg()),
            app.anim.reveal(i.min(6)),
            pane_bg(),
        ));
    }
    let padded = container(list).padding(Padding {
        bottom: super::kit::PAGE_BOTTOM,
        ..Padding::ZERO
    });
    scroll(padded).height(Length::Fill).into()
}

// ---------------------------------------------------------------------------------------------
// Centre pane
// ---------------------------------------------------------------------------------------------

fn center_pane(app: &App, center_w: u32, _w: u32, h: u32) -> Element<'_, Message> {
    let page = content(app, center_w, h);
    // Home staggers its own items; every other page fades and slides in as one.
    let page = if matches!(app.ui.route(), Route::Home) {
        page
    } else {
        entrance(page, app.anim.page(), pane_bg())
    };
    pane_surface(
        column![top_bar(app, center_w), page],
        Length::Fill,
        col(color::surface_container().with_alpha(crate::scheme::PANE_ALPHA)).into(),
    )
    .into()
}

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

fn top_bar(app: &App, center_w: u32) -> Element<'_, Message> {
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
        r = r.push(super::panels::content_chips(app));
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

/// The content router: one arm per screen. `cw` is the width of the centre pane.
fn content(app: &App, cw: u32, _h: u32) -> Element<'_, Message> {
    match app.ui.route() {
        Route::Search(_) => search::view(app, cw),
        Route::Explore => explore::view(app, cw),
        Route::Library => library::view(app, cw),
        Route::Settings => settings::view(app, cw),
        Route::Album(_) | Route::Artist(_) | Route::Playlist(_) => detail::view(app, cw),
        // The queue is a tab of the right pane and Now Playing is its own screen; both are
        // handled before this.
        Route::Home | Route::Queue | Route::NowPlaying => home::view(app, cw),
    }
}
