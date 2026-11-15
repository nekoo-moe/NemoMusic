//! Album, Artist and Playlist pages. Albums and playlists use the YouTube Music layout in
//! [`collection`] (collection-page-ytm spec); the Artist page keeps the 320 px hero with title,
//! Play and Shuffle, then its songs and albums (`ui-reference.md` section 4). Pressing a row plays
//! it with the whole list as the queue.

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length};
use nemo_core::catalog::{AlbumDetail, ArtistDetail, PlaylistDetail};
use nemo_core::track::{Artwork, Track};

use super::super::app::{
    App, ArtShape, DetailData, DetailKey, DetailKind, DetailState, ListSource, Message,
};
use super::super::assets::Icon;
use super::super::style::{col, fit, gap, label};
use super::kit::{
    Card, RowCtx, art, card_grid, filled_button, page, paragraph, section_title, status,
    tonal_button, track_list, vspace,
};
use crate::format;
use crate::layout;
use crate::route::Route;
use crate::state::Event;
use crate::theme::{color, type_scale};

pub mod collection;
mod song_row;

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
    let body: Element<'_, Message> = match app.details.get(&key) {
        None | Some(DetailState::Loading) if collection_page => {
            return collection::skeleton(app, grid.content);
        }
        None | Some(DetailState::Loading) => skeleton_page(app, grid),
        Some(DetailState::Failed(e)) => status(
            Icon::Info,
            "This page did not load",
            format!("{e} Check your connection and try again."),
            Some(("Try again", Message::RetryDetail(key.clone()))),
        ),
        Some(DetailState::Ready(data)) => match data.as_ref() {
            DetailData::Album(a) => return album(app, &key, a, grid),
            DetailData::Artist(a) => artist(app, &key, a, grid),
            DetailData::Playlist(p) => return playlist(app, &key, p, grid),
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
        owner: (!a.summary.artists.is_empty()).then(|| format::artists(&a.summary.artists)),
        year: a.summary.year,
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
        owner: p.summary.author.clone(),
        year: None,
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

/// A shimmering block: a soft highlight sweeps across a tonal rectangle.
pub fn skeleton_block<'a>(
    width: Length,
    height: f32,
    radius: f32,
    phase: f32,
) -> Element<'a, Message> {
    let base = col(color::surface_container_high());
    let glow = col(color::surface_container_highest());
    // The highlight centre moves from -0.3 to 1.3 across the block.
    let c = -0.3 + 1.6 * phase;
    let stop = |x: f32| x.clamp(0.0, 1.0);
    let grad = iced::gradient::Linear::new(iced::Radians(std::f32::consts::FRAC_PI_2))
        .add_stop(0.0, base)
        .add_stop(stop(c - 0.25), base)
        .add_stop(stop(c), glow)
        .add_stop(stop(c + 0.25), base)
        .add_stop(1.0, base);
    container(Space::new())
        .width(width)
        .height(Length::Fixed(height))
        .style(move |_| container::Style {
            background: Some(iced::Background::Gradient(grad.into())),
            border: iced::Border {
                radius: radius.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// The page while it loads: the hero and track rows as shimmering blocks, shown at once.
fn skeleton_page(app: &App, grid: layout::Grid) -> Element<'_, Message> {
    let p = app.shimmer;
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let content_w = grid.content as f32;
    let hero_px = if content_w >= 960.0 { 320.0 } else { 240.0 };
    let text_w = (content_w - hero_px - gap::XXL).max(240.0);
    let r = crate::theme::shape::lg();
    let info = column![
        skeleton_block(Length::Fixed(80.0), 14.0, 7.0, p),
        vspace(gap::MD),
        skeleton_block(Length::Fixed(text_w * 0.8), 40.0, 8.0, p),
        vspace(gap::MD),
        skeleton_block(Length::Fixed(text_w * 0.4), 18.0, 9.0, p),
        vspace(gap::SM),
        skeleton_block(Length::Fixed(text_w * 0.3), 14.0, 7.0, p),
        vspace(gap::LG),
        row![
            skeleton_block(Length::Fixed(120.0), 48.0, crate::theme::shape::FULL, p),
            Space::new().width(gap::MD),
            skeleton_block(Length::Fixed(120.0), 48.0, crate::theme::shape::FULL, p),
        ],
    ]
    .width(Length::Fixed(text_w));
    let mut c = column![
        row![
            skeleton_block(Length::Fixed(hero_px), hero_px, r, p),
            Space::new().width(gap::XXL),
            info
        ]
        .align_y(Alignment::End),
        vspace(gap::XL),
    ];
    for i in 0..7_u8 {
        c = c.push(
            row![
                skeleton_block(Length::Fixed(44.0), 44.0, crate::theme::shape::sm(), p),
                Space::new().width(gap::MD),
                column![
                    skeleton_block(
                        Length::Fixed(content_w * (0.32 + 0.04 * f32::from(i % 3))),
                        14.0,
                        7.0,
                        p
                    ),
                    vspace(gap::XS),
                    skeleton_block(Length::Fixed(content_w * 0.18), 11.0, 6.0, p),
                ],
            ]
            .align_y(Alignment::Center)
            .height(Length::Fixed(56.0)),
        );
    }
    c.into()
}
