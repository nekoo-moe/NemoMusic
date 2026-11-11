//! Album, Artist and Playlist pages (`ui-reference.md` section 4): a 320 px hero with title, meta,
//! Play and Shuffle, then the numbered track list. Pressing a row plays it with the whole list as
//! the queue.

use std::time::Duration;

use iced::widget::{Space, column, row};
use iced::{Alignment, Element, Length};
use nemo_core::catalog::{AlbumDetail, ArtistDetail, PlaylistDetail};
use nemo_core::track::{Artwork, Track};

use super::super::app::{App, ArtShape, DetailData, DetailKey, DetailState, ListSource, Message};
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
    let body: Element<'_, Message> = match app.details.get(&key) {
        None | Some(DetailState::Loading) => label(
            "Loading\u{2026}",
            type_scale::BODY_MD,
            false,
            col(color::ON_SURFACE_VARIANT),
        )
        .into(),
        Some(DetailState::Failed(e)) => status(
            Icon::Info,
            "This page did not load",
            format!("{e} Check your connection and try again."),
            Some(("Try again", Message::RetryDetail(key.clone()))),
        ),
        Some(DetailState::Ready(data)) => match data.as_ref() {
            DetailData::Album(a) => album(app, &key, a, grid),
            DetailData::Artist(a) => artist(app, &key, a, grid),
            DetailData::Playlist(p) => playlist(app, &key, p, grid),
        },
    };
    page(grid.content, body)
}

/// `2019 · 12 songs · 45 min`, skipping what is unknown.
fn meta_line(year: Option<u16>, tracks: &[Track]) -> String {
    let mut parts = Vec::new();
    if let Some(y) = year {
        parts.push(y.to_string());
    }
    parts.push(if tracks.len() == 1 {
        "1 song".to_owned()
    } else {
        format!("{} songs", tracks.len())
    });
    let total: Duration = tracks.iter().filter_map(|t| t.duration).sum();
    if !total.is_zero() {
        let mins = total.as_secs().div_ceil(60);
        parts.push(if mins >= 60 {
            format!("{} h {} min", mins / 60, mins % 60)
        } else {
            format!("{mins} min")
        });
    }
    parts.join(" \u{b7} ")
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
    let dim = col(color::ON_SURFACE_VARIANT);
    let source = ListSource::Detail(key.clone());
    let mut info = column![
        label(h.kind.to_uppercase(), type_scale::LABEL_SM, false, dim),
        vspace(gap::SM),
        paragraph(
            h.title.to_owned(),
            type_scale::DISPLAY_MD,
            col(color::ON_SURFACE),
            text_w
        ),
    ];
    if let Some(s) = &h.subtitle {
        info = info.push(vspace(gap::SM)).push(label(
            fit(s, text_w, type_scale::TITLE_SM.size),
            type_scale::TITLE_SM,
            true,
            col(color::ON_SURFACE),
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
        art(app, &h.artwork, hero_px, h.shape),
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
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let w = grid.content as f32;
    let h = Hero {
        artwork: Artwork::Remote(a.summary.thumbnails.clone()),
        shape: ArtShape::Hero,
        kind: "Album",
        title: &a.summary.title,
        subtitle: Some(format::artists(&a.summary.artists)),
        meta: Some(meta_line(a.summary.year, &a.tracks)),
        note: None,
    };
    column![
        hero(app, &h, key, w),
        vspace(gap::XXL),
        rows(app, key, &a.tracks, w, true)
    ]
    .into()
}

fn playlist<'a>(
    app: &'a App,
    key: &DetailKey,
    p: &'a PlaylistDetail,
    grid: layout::Grid,
) -> Element<'a, Message> {
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let w = grid.content as f32;
    let h = Hero {
        artwork: Artwork::Remote(p.summary.thumbnails.clone()),
        shape: ArtShape::Hero,
        kind: "Playlist",
        title: &p.summary.title,
        subtitle: p.summary.author.clone(),
        meta: Some(meta_line(None, &p.tracks)),
        note: None,
    };
    column![
        hero(app, &h, key, w),
        vspace(gap::XXL),
        rows(app, key, &p.tracks, w, false)
    ]
    .into()
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
            })
            .collect();
        c = c
            .push(section_title("Albums"))
            .push(vspace(gap::MD))
            .push(card_grid(app, grid, cards, 2, 0));
    }
    c.into()
}

#[cfg(test)]
mod tests {
    use super::meta_line;
    use crate::mock;

    #[test]
    fn meta_line_counts_songs_and_minutes() {
        let tracks = mock::tracks().into_iter().take(4).collect::<Vec<_>>();
        let m = meta_line(Some(2019), &tracks);
        assert!(m.starts_with("2019 \u{b7} 4 songs \u{b7} "), "{m}");
        assert!(m.ends_with(" min"), "{m}");
        assert_eq!(
            meta_line(None, &tracks[..1]).split(" \u{b7} ").next(),
            Some("1 song")
        );
    }
}
