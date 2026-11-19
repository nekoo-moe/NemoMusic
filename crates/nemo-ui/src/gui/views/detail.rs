//! Album, Artist and Playlist pages. Albums and playlists use the YouTube Music layout in
//! [`collection`] (collection-page-ytm spec); the Artist page keeps the 320 px hero with title,
//! Play and Shuffle, then its songs and albums (`ui-reference.md` section 4). Pressing a row plays
//! it with the whole list as the queue.

use iced::widget::{Space, column, row};
use iced::{Alignment, Element, Length};
use nemo_core::catalog::{AlbumDetail, ArtistDetail, PlaylistDetail};
use nemo_core::track::{Artwork, Track};

use super::super::app::{
    App, ArtShape, DetailData, DetailKey, DetailKind, DetailState, ListSource, Message,
};
use super::super::assets::Icon;
use super::super::style::{col, fit, gap, label};
use super::kit::{
    Card, RowCtx, Skeleton, art, blank, card_grid, filled_button, page, paragraph, section_title,
    status, swap, tonal_button, track_list, vspace,
};
use crate::format;
use crate::layout;
use crate::route::Route;
use crate::skeleton::Region;
use crate::state::Event;
use crate::theme::{color, type_scale};

pub mod collection;
mod meta_line;
mod song_row;
mod sort_menu;
#[cfg(test)]
mod sort_tests;

/// Hero art size.
const HERO: u32 = 320;

/// An Album, Artist or Playlist page.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid_in(w);
    let Some(key) = DetailKey::of(app.ui.route()) else {
        return page(grid.content, vspace(0.0));
    };
    if !app.ui.youtube_enabled() && !key.is_local() {
        return page(
            grid.content,
            status(
                Icon::Info,
                "YouTube Music is waiting",
                "Read the short notice to open this page. Your own files work without it.",
                Some(("Review notice", Message::Ui(Event::TosReview))),
            ),
        );
    }
    let collection_page = !matches!(key.kind, DetailKind::Artist);
    swap(
        app,
        Region::Detail,
        || loaded(app, &key, grid),
        |sk| {
            if collection_page {
                collection::skeleton(app, sk, grid.content)
            } else {
                page(grid.content, skeleton_page(sk, grid))
            }
        },
    )
}

/// The page as the state says: nothing yet, the failure, or the data.
fn loaded<'a>(app: &'a App, key: &DetailKey, grid: layout::Grid) -> Element<'a, Message> {
    let body: Element<'_, Message> = match app.details.get(key) {
        // Wanted, but the skeleton waits 150 ms so a quick answer never flashes it.
        None | Some(DetailState::Loading) => blank(),
        Some(DetailState::Failed(e)) => status(
            Icon::Info,
            "This page did not load",
            format!("{e} Check your connection and try again."),
            Some(("Try again", Message::RetryDetail(key.clone()))),
        ),
        Some(DetailState::Ready(data)) => match data.as_ref() {
            DetailData::Album(a) => return album(app, key, a, grid),
            DetailData::Artist(a) => artist(app, key, a, grid),
            DetailData::Playlist(p) => return playlist(app, key, p, grid),
        },
    };
    page(grid.content, body)
}

struct Hero<'a> {
    artwork: Artwork,
    shape: ArtShape,
    kind: &'a str,
    title: &'a str,
    subtitle: Option<String>,
    meta: Option<String>,
    note: Option<String>,
}

fn hero<'a>(app: &'a App, h: &Hero<'_>, key: &DetailKey, content_w: f32) -> Element<'a, Message> {
    // 320 px art in a wide pane, 240 in the 1366 px laptop layout, so the page is not all cover.
    let hero_px: u32 = if content_w >= 960.0 { HERO } else { 240 };
    let text_w =
        (content_w - f32::from(u16::try_from(hero_px).unwrap_or(320)) - gap::XXL).max(240.0);
    let dim = col(color::on_surface_variant());
    let source = ListSource::Detail(key.clone());
    let mut info = column![
        label(h.kind.to_uppercase(), type_scale::LABEL_SM, false, dim),
        vspace(gap::SM),
        paragraph(
            h.title.to_owned(),
            type_scale::DISPLAY_MD,
            col(color::on_surface()),
            text_w
        ),
    ];
    if let Some(s) = &h.subtitle {
        info = info.push(vspace(gap::SM)).push(label(
            fit(s, text_w, type_scale::TITLE_SM.size),
            type_scale::TITLE_SM,
            true,
            col(color::on_surface()),
        ));
    }
    if let Some(m) = &h.meta {
        info = info
            .push(vspace(gap::XS))
            .push(label(m.clone(), type_scale::BODY_MD, false, dim));
    }
    if let Some(n) = &h.note {
        info =
            info.push(vspace(gap::SM))
                .push(paragraph(n.clone(), type_scale::BODY_SM, dim, text_w));
    }
    info = info.push(vspace(gap::LG)).push(row![
        filled_button(
            "Play",
            Icon::Play,
            app.accent(),
            Message::PlayAt {
                source: source.clone(),
                index: 0,
            },
        ),
        Space::new().width(gap::MD),
        tonal_button(
            "Shuffle",
            Some(Icon::Shuffle),
            Some(Message::Shuffle(source))
        ),
    ]);
    row![
        Element::from(
            super::super::fx::Fx::new(art(app, &h.artwork, hero_px, h.shape))
                .passive()
                .scale(0.86 + 0.14 * app.anim.page())
        ),
        Space::new().width(gap::XXL),
        info.width(Length::Fixed(text_w)),
    ]
    .align_y(Alignment::End)
    .into()
}

fn rows<'a>(
    app: &'a App,
    key: &DetailKey,
    tracks: &'a [Track],
    content_w: f32,
    numbered: bool,
) -> Element<'a, Message> {
    let ctx = RowCtx {
        width: content_w,
        source: ListSource::Detail(key.clone()),
        numbered,
        show_album: !numbered,
    };
    track_list(app, tracks, app.rows_limit, &ctx)
}

fn album<'a>(
    app: &'a App,
    key: &DetailKey,
    a: &'a AlbumDetail,
    grid: layout::Grid,
) -> Element<'a, Message> {
    let c = collection::Collection {
        kind: "Album",
        artwork: Artwork::Remote(a.summary.thumbnails.clone()),
        title: &a.summary.title,
        owner: a.meta.owner.clone().or_else(|| {
            (!a.summary.artists.is_empty()).then(|| format::artists(&a.summary.artists))
        }),
        year: a.summary.year.or(a.meta.year),
        privacy: None,
        meta: &a.meta,
        tracks: &a.tracks,
        numbered: true,
    };
    collection::view(app, key, &c, grid.content)
}

fn playlist<'a>(
    app: &'a App,
    key: &DetailKey,
    p: &'a PlaylistDetail,
    grid: layout::Grid,
) -> Element<'a, Message> {
    let c = collection::Collection {
        kind: "Playlist",
        artwork: Artwork::Remote(p.summary.thumbnails.clone()),
        title: &p.summary.title,
        owner: p.meta.owner.clone().or_else(|| p.summary.author.clone()),
        year: p.meta.year,
        // One's own playlist knows its privacy from the edit data even when the header is quiet.
        privacy: p
            .meta
            .privacy
            .or_else(|| p.edit.as_ref().map(|e| e.privacy)),
        meta: &p.meta,
        tracks: &p.tracks,
        numbered: false,
    };
    collection::view(app, key, &c, grid.content)
}

fn artist<'a>(
    app: &'a App,
    key: &DetailKey,
    a: &'a ArtistDetail,
    grid: layout::Grid,
) -> Element<'a, Message> {
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let w = grid.content as f32;
    let note = a.description.as_ref().map(|d| {
        let short: String = d.chars().take(220).collect();
        if short.len() < d.len() {
            format!("{}\u{2026}", short.trim_end())
        } else {
            short
        }
    });
    let h = Hero {
        artwork: Artwork::Remote(a.summary.thumbnails.clone()),
        shape: ArtShape::Circle,
        kind: "Artist",
        title: &a.summary.name,
        subtitle: None,
        meta: None,
        note,
    };
    let mut c = column![hero(app, &h, key, w), vspace(gap::XXL)];
    if !a.top_songs.is_empty() {
        c = c
            .push(section_title("Popular"))
            .push(vspace(gap::SM))
            .push(rows(app, key, &a.top_songs, w, false))
            .push(vspace(gap::XXL));
    }
    if !a.albums.is_empty() {
        let cards = a
            .albums
            .iter()
            .map(|al| Card {
                artwork: Artwork::Remote(al.thumbnails.clone()),
                title: al.title.clone(),
                subtitle: al
                    .year
                    .map_or_else(|| "Album".to_owned(), |y| y.to_string()),
                shape: ArtShape::Tile,
                on_press: Message::Ui(Event::Go(Route::Album(al.id.clone()))),
                play: Some(Message::PlayCollection(DetailKey {
                    kind: DetailKind::Album,
                    id: al.id.clone(),
                })),
            })
            .collect();
        c = c
            .push(section_title("Albums"))
            .push(vspace(gap::MD))
            .push(card_grid(app, grid, cards, 2, 0));
    }
    c.into()
}

/// The artist page while it loads: the hero and track rows as blocks.
fn skeleton_page<'a>(sk: Skeleton, grid: layout::Grid) -> Element<'a, Message> {
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let content_w = grid.content as f32;
    let hero_px = if content_w >= 960.0 { 320.0 } else { 240.0 };
    let text_w = (content_w - hero_px - gap::XXL).max(240.0);
    let info = column![
        sk.line(80.0, type_scale::LABEL_SM),
        vspace(gap::SM),
        sk.line(text_w * 0.8, type_scale::DISPLAY_MD),
        vspace(gap::SM),
        sk.line(text_w * 0.4, type_scale::TITLE_SM),
        vspace(gap::XS),
        sk.line(text_w * 0.3, type_scale::BODY_MD),
        vspace(gap::LG),
        row![
            sk.block(Length::Fixed(120.0), 48.0, crate::theme::shape::FULL),
            Space::new().width(gap::MD),
            sk.block(Length::Fixed(120.0), 48.0, crate::theme::shape::FULL),
        ],
    ]
    .width(Length::Fixed(text_w));
    let mut c = column![
        row![
            sk.block(Length::Fixed(hero_px), hero_px, crate::theme::shape::lg()),
            Space::new().width(gap::XXL),
            info
        ]
        .align_y(Alignment::End),
        vspace(gap::XL),
    ];
    for i in 0..7_usize {
        c = c.push(sk.row(content_w, i));
    }
    c.into()
}
