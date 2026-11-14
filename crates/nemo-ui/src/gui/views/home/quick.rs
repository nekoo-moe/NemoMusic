//! Home: the Quick picks song list and the quick access tile grid.

use iced::widget::{Space, column, row};
use iced::{Alignment, Element, Length};
use nemo_core::catalog::ShelfItem;
use nemo_core::track::{Artwork, Track};

use crate::gui::app::{App, ArtShape, DetailKey, DetailKind, ListSource, Message};
use crate::gui::style::{col, gap, label};
use crate::gui::views::kit::{eyebrow, quick_pick, section_title, text_button, vspace};
use crate::route::Route;
use crate::state::Event;
use crate::theme::{color, type_scale};

/// Songs in Quick picks (four rows of two).
const QUICK_COUNT: usize = 8;
/// Tiles in the quick access grid.
const QUICK_ACCESS: usize = 8;

/// Where Quick picks come from: the first shelf with songs, else the recent tracks, else the
/// local library. The list a row plays is that same source.
pub(super) fn quick_picks_source(app: &App) -> Option<(ListSource, Vec<&Track>)> {
    for (si, shelf) in app.home.shelves.iter().enumerate() {
        let songs: Vec<&Track> = shelf
            .items
            .iter()
            .filter_map(|i| match i {
                ShelfItem::Song(t) => Some(t),
                _ => None,
            })
            .collect();
        if !songs.is_empty() {
            return Some((
                ListSource::Home(si),
                songs.into_iter().take(QUICK_COUNT).collect(),
            ));
        }
    }
    if !app.ui.recent.is_empty() {
        return Some((
            ListSource::Recent,
            app.ui.recent.iter().take(QUICK_COUNT).collect(),
        ));
    }
    if !app.local_tracks.is_empty() {
        return Some((
            ListSource::Library,
            app.local_tracks.iter().take(QUICK_COUNT).collect(),
        ));
    }
    None
}

pub(super) fn quick_source_kind(app: &App) -> Option<ListSource> {
    quick_picks_source(app).map(|(s, _)| s)
}

pub(super) fn quick_picks<'a>(
    app: &'a App,
    source: ListSource,
    tracks: &[&'a Track],
    content_w: f32,
) -> Element<'a, Message> {
    let col_w = ((content_w - gap::LG) / 2.0).floor();
    let mut rows = column![].spacing(gap::XS);
    for (r, pair) in tracks.chunks(2).enumerate() {
        let mut line = row![].spacing(gap::LG);
        for (k, t) in pair.iter().enumerate() {
            line = line.push(quick_pick(
                app,
                t,
                source.clone(),
                r * 2 + k,
                col_w,
                r * 2 + k,
            ));
        }
        rows = rows.push(line);
    }
    column![
        eyebrow("Start listening"),
        row![
            section_title("Quick picks"),
            Space::new().width(Length::Fill),
            text_button("Play all", Message::PlayAt { source, index: 0 }),
        ]
        .align_y(Alignment::Center),
        vspace(gap::SM),
        rows,
        vspace(gap::XXL),
    ]
    .into()
}

/// One tile of the quick access grid.
pub(super) struct Access {
    title: String,
    artwork: Artwork,
    on_press: Message,
    play: Message,
}

/// Up to eight things to jump into: recent songs, then local albums, then playlists, albums and
/// songs of the Home shelves.
pub(super) fn quick_access(app: &App) -> Vec<Access> {
    let mut out: Vec<Access> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let mut add = |out: &mut Vec<Access>, id: String, a: Access| {
        if out.len() < QUICK_ACCESS && !seen.contains(&id) {
            seen.push(id);
            out.push(a);
        }
    };
    for (i, t) in app.ui.recent.iter().enumerate() {
        let play = Message::PlayAt {
            source: ListSource::Recent,
            index: i,
        };
        add(
            &mut out,
            format!("t{}", t.id.0),
            Access {
                title: t.title.clone(),
                artwork: t.artwork.clone(),
                on_press: play.clone(),
                play,
            },
        );
    }
    for a in &app.local_albums {
        add(&mut out, format!("a{}", a.id.0), album_access(a));
    }
    for item in app.home.shelves.iter().flat_map(|s| s.items.iter()) {
        match item {
            ShelfItem::Playlist(p) => add(
                &mut out,
                format!("p{}", p.id.0),
                Access {
                    title: p.title.clone(),
                    artwork: Artwork::Remote(p.thumbnails.clone()),
                    on_press: Message::Ui(Event::Go(Route::Playlist(p.id.clone()))),
                    play: Message::PlayCollection(DetailKey {
                        kind: DetailKind::Playlist,
                        id: p.id.clone(),
                    }),
                },
            ),
            ShelfItem::Album(a) => add(&mut out, format!("a{}", a.id.0), album_access(a)),
            _ => {}
        }
    }
    out
}

fn album_access(a: &nemo_core::catalog::AlbumSummary) -> Access {
    Access {
        title: a.title.clone(),
        artwork: Artwork::Remote(a.thumbnails.clone()),
        on_press: Message::Ui(Event::Go(Route::Album(a.id.clone()))),
        play: Message::PlayCollection(DetailKey {
            kind: DetailKind::Album,
            id: a.id.clone(),
        }),
    }
}

/// The Spotify-style grid: two or four columns of 56 px tiles with a hover play button.
pub(super) fn quick_access_grid(
    app: &App,
    items: Vec<Access>,
    content_w: f32,
) -> Element<'_, Message> {
    use crate::gui::fx::{BadgeAnchor, entrance, fx};
    use crate::gui::style::{button_flat, fit};
    use crate::gui::views::kit::{art, pane_bg, play_badge};
    use iced::widget::button;
    let cols: u8 = if content_w >= 880.0 { 4 } else { 2 };
    let cols_f = f32::from(cols);
    let gap_px = gap::SM;
    let tile_w = ((content_w - gap_px * (cols_f - 1.0)) / cols_f).floor();
    let text_w = tile_w - 56.0 - gap::MD - gap::SM - 44.0;
    let mut rows = column![].spacing(gap_px);
    let mut it = items.into_iter().enumerate().peekable();
    while it.peek().is_some() {
        let mut line = row![].spacing(gap_px);
        for (i, a) in it.by_ref().take(usize::from(cols)) {
            let body = row![
                art(app, &a.artwork, 56, ArtShape::Thumb),
                Space::new().width(gap::MD),
                label(
                    fit(&a.title, text_w, type_scale::BODY_MD.size),
                    type_scale::BODY_MD,
                    true,
                    col(color::on_surface())
                ),
            ]
            .align_y(Alignment::Center);
            let b = button(
                iced::widget::container(body)
                    .width(Length::Fixed(tile_w))
                    .height(Length::Fixed(56.0))
                    .align_y(Alignment::Center),
            )
            .padding(0)
            .on_press(a.on_press);
            let radius = crate::theme::shape::md();
            let host = fx(b.style(button_flat(col(color::surface_container_high()), radius)))
                .radius(radius)
                .badge(play_badge(
                    app,
                    36.0,
                    BadgeAnchor::CenterRight(10.0),
                    a.play,
                ));
            line = line.push(entrance(host, app.anim.reveal(i), pane_bg()));
        }
        rows = rows.push(line);
    }
    rows.into()
}
