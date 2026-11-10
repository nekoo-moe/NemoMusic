//! Search: a query box (300 ms debounce in the reducer), then results grouped as Songs, Albums,
//! Artists and Playlists. Enter plays the top song.

use iced::widget::{Space, column, container, stack, text_input};
use iced::{Alignment, Border, Element, Length, Padding};

use super::super::app::{App, ArtShape, ListSource, Message, SEARCH_ID};
use super::super::assets::Icon;
use super::super::style::{col, gap, icon, label};
use super::kit::{
    Card, RowCtx, card_grid, page, page_title, section_title, status, track_list, vspace,
};
use crate::format;
use crate::layout;
use crate::route::Route;
use crate::search::SONGS_SHOWN;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};
use nemo_core::track::Artwork;

/// The Search screen.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid(w, app.ui.queue_open);
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let content_w = grid.content as f32;
    let search = &app.ui.search;

    let mut c = column![
        page_title("Search"),
        vspace(gap::LG),
        query_box(app, content_w)
    ]
    .width(Length::Fill);
    c = c.push(vspace(gap::XL));

    if !app.ui.youtube_enabled() {
        c = c.push(status(
            Icon::Info,
            "YouTube Music is waiting",
            "Read the short notice to search YouTube Music. Your own files work without it.",
            Some(("Review notice", Message::Ui(Event::TosReview))),
        ));
        return page(grid.content, c);
    }

    let results = search.results.as_ref();
    if search.query().is_empty() {
        c = c.push(status(
            Icon::Search,
            "Find something to play",
            "Search songs, albums, artists and playlists. Press Enter to play the top song.",
            None,
        ));
    } else if let Some(r) = results {
        let any = !(r.songs.is_empty()
            && r.albums.is_empty()
            && r.artists.is_empty()
            && r.playlists.is_empty());
        if !any && !search.loading {
            c = c.push(status(
                Icon::Search,
                format!("No results for \u{201c}{}\u{201d}", search.query()),
                "Check the spelling or try fewer words.",
                None,
            ));
        }
        if !r.songs.is_empty() {
            let shown = r.songs.len().min(SONGS_SHOWN.max(1));
            let ctx = RowCtx {
                width: content_w,
                source: ListSource::Search,
                numbered: false,
                show_album: true,
            };
            c = c
                .push(section_title("Songs"))
                .push(vspace(gap::SM))
                .push(track_list(app, &r.songs, shown, &ctx))
                .push(vspace(gap::XXL));
        }
        if !r.albums.is_empty() {
            let cards = r
                .albums
                .iter()
                .map(|a| Card {
                    artwork: Artwork::Remote(a.thumbnails.clone()),
                    title: a.title.clone(),
                    subtitle: format::artists(&a.artists),
                    shape: ArtShape::Card,
                    on_press: Message::Ui(Event::Go(Route::Album(a.id.clone()))),
                })
                .collect();
            c = c
                .push(section_title("Albums"))
                .push(vspace(gap::MD))
                .push(card_grid(app, grid, cards, 1))
                .push(vspace(gap::XXL));
        }
        if !r.artists.is_empty() {
            let cards = r
                .artists
                .iter()
                .map(|a| Card {
                    artwork: Artwork::Remote(a.thumbnails.clone()),
                    title: a.name.clone(),
                    subtitle: "Artist".to_owned(),
                    shape: ArtShape::Circle,
                    on_press: Message::Ui(Event::Go(Route::Artist(a.id.clone()))),
                })
                .collect();
            c = c
                .push(section_title("Artists"))
                .push(vspace(gap::MD))
                .push(card_grid(app, grid, cards, 1))
                .push(vspace(gap::XXL));
        }
        if !r.playlists.is_empty() {
            let cards = r
                .playlists
                .iter()
                .map(|p| Card {
                    artwork: Artwork::Remote(p.thumbnails.clone()),
                    title: p.title.clone(),
                    subtitle: p.author.clone().unwrap_or_else(|| "Playlist".to_owned()),
                    shape: ArtShape::Card,
                    on_press: Message::Ui(Event::Go(Route::Playlist(p.id.clone()))),
                })
                .collect();
            c = c
                .push(section_title("Playlists"))
                .push(vspace(gap::MD))
                .push(card_grid(app, grid, cards, 1));
        }
    } else if let Some(e) = &search.error {
        c = c.push(status(
            Icon::Info,
            "Search did not work",
            format!("{e} Check your connection and try again."),
            Some(("Try again", Message::Ui(Event::SearchRetry))),
        ));
    } else {
        c = c.push(label(
            "Searching\u{2026}",
            type_scale::BODY_MD,
            false,
            col(color::ON_SURFACE_VARIANT),
        ));
    }
    if results.is_some()
        && let Some(e) = &search.error
    {
        // An older answer stays on screen; say why it is stale.
        c = c.push(vspace(gap::LG)).push(status(
            Icon::Info,
            "Search did not work",
            format!("{e} Showing the previous results."),
            Some(("Try again", Message::Ui(Event::SearchRetry))),
        ));
    }
    page(grid.content, c)
}

/// The query box: a pill, height 48, glyph on the left, "Searching" hint on the right. The pill
/// gets a 2 px accent ring while it has focus.
fn query_box(app: &App, width: f32) -> Element<'_, Message> {
    let dim = col(color::ON_SURFACE_VARIANT);
    let ring = col(app.accent().primary);
    let width = width.min(560.0);
    let input = text_input("Search songs, albums, artists", &app.ui.search.input)
        .id(SEARCH_ID)
        .on_input(|t| Message::Ui(Event::SearchInput(t)))
        .on_submit(Message::Ui(Event::SearchSubmit))
        .size(f32::from(type_scale::BODY_MD.size))
        .font(super::super::assets::font(false))
        .width(Length::Fixed(width))
        .padding(Padding {
            top: gap::MD,
            bottom: gap::MD,
            left: gap::XXXL,
            right: gap::XXXL,
        })
        .style(move |_, status| {
            let focused = matches!(status, text_input::Status::Focused { .. });
            text_input::Style {
                background: iced::Background::Color(col(color::SURFACE_CONTAINER_HIGH)),
                border: Border {
                    width: if focused { 2.0 } else { 0.0 },
                    radius: shape::FULL.into(),
                    color: ring,
                },
                icon: col(color::ON_SURFACE_VARIANT),
                placeholder: col(color::ON_SURFACE_VARIANT),
                value: col(color::ON_SURFACE),
                selection: col(color::PRIMARY_FALLBACK.with_alpha(0.4)),
            }
        });
    let hint: Element<'_, Message> = if app.ui.search.loading {
        label("Searching\u{2026}", type_scale::BODY_SM, false, dim).into()
    } else {
        Space::new().into()
    };
    stack![
        input,
        container(icon(Icon::Search, 20.0, dim))
            .height(Length::Fixed(48.0))
            .padding(Padding::from([0.0, gap::LG]))
            .align_y(Alignment::Center),
        container(hint)
            .width(Length::Fixed(width))
            .height(Length::Fixed(48.0))
            .padding(Padding::from([0.0, gap::LG]))
            .align_x(Alignment::End)
            .align_y(Alignment::Center),
    ]
    .width(Length::Fixed(width))
    .height(Length::Fixed(48.0))
    .into()
}
