//! The library list of the left pane: header, kind chips and the rows of playlists, artists and albums.

use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Color, Element, Length, Padding};
use nemo_core::catalog::{AlbumSummary, ArtistSummary, PlaylistSummary, ShelfItem};
use nemo_core::track::Artwork;

use crate::format;
use crate::gui::app::{App, ArtShape, LibFilter, Message};
use crate::gui::assets::Icon;
use crate::gui::fx::entrance;
use crate::gui::style::{animated, col, fit, gap, icon, icon_button, label, scroll, tip};
use crate::gui::views::kit::{art, chip_padded, pane_bg};
use crate::route::Route;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// Height of a library row.
const LIB_ROW_H: f32 = 64.0;

pub(super) fn library_header<'a>() -> Element<'a, Message> {
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
                Some(Message::Create(crate::gui::app::CreateMsg::Open)),
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

pub(super) fn library_chips(app: &App) -> Element<'_, Message> {
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

pub(super) fn library_list<'a>(app: &'a App, width: f32, route: &Route) -> Element<'a, Message> {
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
        bottom: crate::gui::views::kit::PAGE_BOTTOM,
        ..Padding::ZERO
    });
    scroll(padded).height(Length::Fill).into()
}
