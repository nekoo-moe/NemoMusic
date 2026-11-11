//! Home: "Quick picks" as a two-column song list, then shelves of large cards laid out with
//! [`crate::layout::grid_in`]. The shelves come from the catalog's home feed when there is one,
//! otherwise from a few calm seed searches (see [`crate::home`]); a shelf that fails is absent.
//! The chips in the top bar filter the shelves by kind.

use iced::widget::{Space, column, row};
use iced::{Alignment, Color, Element, Length, Padding};
use nemo_core::catalog::{Shelf, ShelfItem};
use nemo_core::track::{Artwork, Track};

use super::super::app::{
    App, ArtShape, DetailKey, DetailKind, HomeFilter, HomePhase, ListSource, Message,
};
use super::super::assets::Icon;
use super::super::style::{col, gap, label};
use super::kit::{
    Card, card_grid, eyebrow, page, quick_pick, section_title, shelf_header, status, text_button,
    vspace,
};
use crate::explore::ContentFilter;
use crate::format;
use crate::layout;
use crate::route::Route;
use crate::state::Event;
use crate::theme::{color, type_scale};

/// Songs in Quick picks (four rows of two).
const QUICK_COUNT: usize = 8;
/// Tiles in the quick access grid.
const QUICK_ACCESS: usize = 8;

/// The Home screen. `w` is the width of the centre pane.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid_in(w);
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let content_w = grid.content as f32;
    // The kind chips only apply under Music; Videos and Podcasts are hook points with no data yet.
    let filter = if app.content_filter == ContentFilter::Music {
        app.home_filter
    } else {
        HomeFilter::All
    };
    let mut c = column![].width(Length::Fill);
    let mut any = false;
    if matches!(
        app.content_filter,
        ContentFilter::Videos | ContentFilter::Podcasts
    ) {
        return feed_kind_page(app, grid);
    }
    if app.content_filter == ContentFilter::Music {
        c = c.push(kind_chips(app)).push(vspace(gap::MD));
    }
    if filter == HomeFilter::All {
        c = c.push(greeting(app, content_w));
    }
    if filter == HomeFilter::All {
        let qa = quick_access(app);
        if !qa.is_empty() {
            any = true;
            c = c
                .push(quick_access_grid(app, qa, content_w))
                .push(vspace(gap::XL));
        }
        for (i, sec) in app.daily_sections.iter().enumerate() {
            any = true;
            c = c.push(daily_section(app, i, sec, grid, content_w));
        }
    }

    let quick = quick_picks_source(app);
    let quick_shelf = match &quick {
        Some((ListSource::Home(si), _)) => Some(*si),
        _ => None,
    };
    if let Some((source, tracks)) = quick
        && matches!(filter, HomeFilter::All | HomeFilter::Songs)
    {
        any = true;
        c = c.push(quick_picks(app, source, &tracks, content_w));
    }

    if matches!(filter, HomeFilter::All | HomeFilter::Songs)
        && !app.ui.recent.is_empty()
        && !matches!(quick_source_kind(app), Some(ListSource::Recent))
    {
        any = true;
        let cards = app
            .ui
            .recent
            .iter()
            .enumerate()
            .map(|(i, t)| Card {
                artwork: t.artwork.clone(),
                title: t.title.clone(),
                subtitle: format::artists(&t.artists),
                shape: ArtShape::Tile,
                on_press: Message::PlayAt {
                    source: ListSource::Recent,
                    index: i,
                },
                play: Some(Message::PlayAt {
                    source: ListSource::Recent,
                    index: i,
                }),
            })
            .collect();
        c = c
            .push(shelf_header("Jump back in"))
            .push(card_grid(app, grid, cards, 1, 1))
            .push(vspace(gap::XXL));
    }

    if matches!(filter, HomeFilter::All | HomeFilter::Albums) && !app.local_albums.is_empty() {
        any = true;
        let cards = app
            .local_albums
            .iter()
            .map(|a| Card {
                artwork: Artwork::Remote(a.thumbnails.clone()),
                title: a.title.clone(),
                subtitle: format::artists(&a.artists),
                shape: ArtShape::Tile,
                on_press: Message::Ui(Event::Go(Route::Album(a.id.clone()))),
                play: Some(Message::PlayCollection(DetailKey {
                    kind: DetailKind::Album,
                    id: a.id.clone(),
                })),
            })
            .collect();
        c = c
            .push(shelf_header("Your albums"))
            .push(card_grid(app, grid, cards, 1, 2))
            .push(vspace(gap::XXL));
    }

    for (si, shelf) in app.home.shelves.iter().enumerate() {
        if quick_shelf == Some(si) {
            continue;
        }
        let cards = shelf_cards(si, shelf, filter);
        if cards.is_empty() {
            continue;
        }
        any = true;
        c = c
            .push(shelf_header(shelf.title.clone()))
            .push(card_grid(app, grid, cards, 1, 2 + si))
            .push(vspace(gap::XXL));
    }

    if !app.ui.youtube_enabled() {
        c = c.push(status(
            Icon::Info,
            "YouTube Music is waiting",
            "Read the short notice to browse and search YouTube Music. Your own files work without it.",
            Some(("Review notice", Message::Ui(Event::TosReview))),
        ));
    } else if app.home.phase != HomePhase::Done {
        c = c.push(label(
            "Finding something calm\u{2026}",
            type_scale::BODY_MD,
            false,
            col(color::on_surface_variant()),
        ));
    } else if !any {
        c = c.push(status(
            Icon::Note,
            "Nothing here yet",
            "Search for music, or add a music folder in Settings.",
            Some((
                "Search",
                Message::Ui(Event::Go(Route::Search(String::new()))),
            )),
        ));
    }
    page(grid.content, c)
}

/// Where Quick picks come from: the first shelf with songs, else the recent tracks, else the
/// local library. The list a row plays is that same source.
fn quick_picks_source(app: &App) -> Option<(ListSource, Vec<&Track>)> {
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

fn quick_source_kind(app: &App) -> Option<ListSource> {
    quick_picks_source(app).map(|(s, _)| s)
}

fn quick_picks<'a>(
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

fn shelf_cards(shelf_index: usize, shelf: &Shelf, filter: HomeFilter) -> Vec<Card> {
    let mut songs = 0_usize;
    let mut cards = Vec::new();
    for item in &shelf.items {
        // Song positions count every song of the shelf, filtered or not: that is the list a
        // press plays.
        let song_index = songs;
        if matches!(item, ShelfItem::Song(_)) {
            songs += 1;
        }
        if !filter.keeps(item) {
            continue;
        }
        cards.push(match item {
            ShelfItem::Song(t) => Card {
                artwork: t.artwork.clone(),
                title: t.title.clone(),
                subtitle: format::artists(&t.artists),
                shape: ArtShape::Tile,
                on_press: Message::PlayAt {
                    source: ListSource::Home(shelf_index),
                    index: song_index,
                },
                play: Some(Message::PlayAt {
                    source: ListSource::Home(shelf_index),
                    index: song_index,
                }),
            },
            ShelfItem::Album(a) => Card {
                artwork: Artwork::Remote(a.thumbnails.clone()),
                title: a.title.clone(),
                subtitle: format::artists(&a.artists),
                shape: ArtShape::Tile,
                on_press: Message::Ui(Event::Go(Route::Album(a.id.clone()))),
                play: Some(Message::PlayCollection(DetailKey {
                    kind: DetailKind::Album,
                    id: a.id.clone(),
                })),
            },
            ShelfItem::Artist(a) => Card {
                artwork: Artwork::Remote(a.thumbnails.clone()),
                title: a.name.clone(),
                subtitle: "Artist".to_owned(),
                shape: ArtShape::Circle,
                on_press: Message::Ui(Event::Go(Route::Artist(a.id.clone()))),
                play: None,
            },
            ShelfItem::Playlist(p) => Card {
                artwork: Artwork::Remote(p.thumbnails.clone()),
                title: p.title.clone(),
                subtitle: p.author.clone().unwrap_or_else(|| "Playlist".to_owned()),
                shape: ArtShape::Tile,
                on_press: Message::Ui(Event::Go(Route::Playlist(p.id.clone()))),
                play: Some(Message::PlayCollection(DetailKey {
                    kind: DetailKind::Playlist,
                    id: p.id.clone(),
                })),
            },
        });
    }
    cards
}

/// One tile of the quick access grid.
struct Access {
    title: String,
    artwork: Artwork,
    on_press: Message,
    play: Message,
}

/// Up to eight things to jump into: recent songs, then local albums, then playlists, albums and
/// songs of the Home shelves.
fn quick_access(app: &App) -> Vec<Access> {
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
fn quick_access_grid(app: &App, items: Vec<Access>, content_w: f32) -> Element<'_, Message> {
    use super::super::fx::{BadgeAnchor, entrance, fx};
    use super::super::style::{button_flat, fit};
    use super::kit::{art, pane_bg, play_badge};
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

/// Kind chips under Music: All kinds, Songs, Albums, Artists, Playlists.
fn kind_chips(app: &App) -> Element<'_, Message> {
    let accent = app.accent();
    let mut r = row![].spacing(gap::SM);
    for f in HomeFilter::ALL {
        r = r.push(super::kit::chip(
            f.label(),
            app.home_filter == f,
            accent,
            Message::HomeFilter(f),
        ));
    }
    r.into()
}

/// What the hero banner shows.
struct Hero {
    tag: String,
    title: String,
    subtitle: String,
    artwork: Artwork,
    play: Message,
}

fn hero_info(app: &App) -> Option<Hero> {
    use crate::daily::DailyKind;
    if let Some((i, sec)) = app
        .daily_sections
        .iter()
        .enumerate()
        .find(|(_, s)| matches!(s.kind, DailyKind::DailyMix) && !s.tracks.is_empty())
    {
        return Some(Hero {
            tag: sec.tag.to_owned(),
            title: sec.title.clone(),
            subtitle: format!(
                "{} \u{b7} {} and more",
                sec.tracks[0].title,
                format::artists(&sec.tracks[0].artists)
            ),
            artwork: sec.tracks[0].artwork.clone(),
            play: Message::PlayAt {
                source: ListSource::Daily(i),
                index: 0,
            },
        });
    }
    // Signed out, or no history yet: a plain suggestion, said so.
    for (si, shelf) in app.home.shelves.iter().enumerate() {
        if let Some((n, t)) = shelf
            .items
            .iter()
            .filter_map(|i| match i {
                ShelfItem::Song(t) => Some(t),
                _ => None,
            })
            .enumerate()
            .next()
        {
            return Some(Hero {
                tag: "A suggestion to start with".to_owned(),
                title: t.title.clone(),
                subtitle: format::artists(&t.artists),
                artwork: t.artwork.clone(),
                play: Message::PlayAt {
                    source: ListSource::Home(si),
                    index: n,
                },
            });
        }
    }
    None
}

/// The time-of-day greeting and the hero banner card.
fn greeting(app: &App, content_w: f32) -> Element<'_, Message> {
    use super::super::style::{button_flat, vertical_gradient};
    let mut c = column![
        label(
            app.day.greeting(),
            type_scale::HEADLINE_MD,
            true,
            col(color::on_surface())
        ),
        vspace(gap::MD),
    ];
    if let Some(h) = hero_info(app) {
        let accent = app.accent();
        let art_px = 176_u32;
        let text_w = (content_w - 176.0 - 2.0 * gap::XL - gap::XL).max(200.0);
        let info = column![
            label(
                super::super::style::fit(&h.tag, text_w, type_scale::LABEL_MD.size).to_uppercase(),
                type_scale::LABEL_SM,
                false,
                col(accent.on_primary_container.with_alpha(0.8))
            ),
            vspace(gap::XS),
            label(
                super::super::style::fit(&h.title, text_w, type_scale::HEADLINE_SM.size),
                type_scale::HEADLINE_SM,
                true,
                col(accent.on_primary_container)
            ),
            vspace(gap::XS),
            label(
                super::super::style::fit(&h.subtitle, text_w, type_scale::BODY_MD.size),
                type_scale::BODY_MD,
                false,
                col(accent.on_primary_container.with_alpha(0.85))
            ),
            vspace(gap::LG),
            super::kit::filled_button("Play", Icon::Play, accent, h.play.clone()),
        ]
        .width(Length::Fixed(text_w));
        let body = row![
            info,
            Space::new().width(Length::Fill),
            super::kit::art(app, &h.artwork, art_px, ArtShape::Tile)
        ]
        .align_y(Alignment::Center)
        .padding(gap::XL);
        let top = accent.primary_container;
        let bottom = accent
            .primary_container
            .with_alpha(0.55)
            .over(color::surface_container());
        let radius = crate::theme::shape::xl();
        let b = iced::widget::button(iced::widget::container(body).width(Length::Fill).style(
            move |_| iced::widget::container::Style {
                background: Some(vertical_gradient(col(top), col(bottom))),
                border: iced::Border {
                    radius: radius.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::container::Style::default()
            },
        ))
        .padding(0)
        .width(Length::Fill)
        .on_press(h.play);
        c = c
            .push(
                super::super::fx::fx(b.style(button_flat(Color::TRANSPARENT, radius)))
                    .radius(radius)
                    .lift(2.0),
            )
            .push(vspace(gap::XL));
    }
    c.into()
}

/// One daily section: header with its honest source line, then cards or the mood tile.
fn daily_section<'a>(
    app: &'a App,
    index: usize,
    sec: &'a crate::daily::DailySection,
    grid: layout::Grid,
    content_w: f32,
) -> Element<'a, Message> {
    use crate::daily::DailyKind;
    let dim = col(color::on_surface_variant());
    let mut c = column![
        section_title(sec.title.clone()),
        label(sec.tag, type_scale::LABEL_MD, false, dim),
        vspace(gap::MD),
    ];
    match &sec.kind {
        DailyKind::MoodOfDay(name) => {
            let mood = crate::explore::MOODS
                .iter()
                .find(|m| m.label.eq_ignore_ascii_case(name))
                .copied()
                .unwrap_or(crate::explore::MOODS[0]);
            let (fill, ink) = super::explore::tile_color(&mood);
            let radius = crate::theme::shape::lg();
            let b = iced::widget::button(
                iced::widget::container(
                    row![
                        label(name.clone(), type_scale::TITLE_MD, true, col(ink)),
                        Space::new().width(Length::Fill),
                        label("Search this mood", type_scale::BODY_SM, false, col(ink)),
                        Space::new().width(gap::SM),
                        super::super::style::icon(Icon::ArrowForward, 18.0, col(ink)),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding(Padding::from([0.0, gap::XL]))
                .width(Length::Fixed(content_w.min(520.0)))
                .height(Length::Fixed(72.0))
                .align_y(Alignment::Center),
            )
            .padding(0)
            .on_press(Message::SearchFor(mood_query(name)));
            c = c.push(
                super::super::fx::fx(b.style(move |_, _| iced::widget::button::Style {
                    background: Some(col(fill).into()),
                    text_color: col(ink),
                    border: iced::Border {
                        radius: radius.into(),
                        ..iced::Border::default()
                    },
                    shadow: iced::Shadow::default(),
                    snap: false,
                }))
                .layer(col(ink))
                .radius(radius)
                .lift(2.0),
            );
        }
        _ if !sec.albums.is_empty() => {
            let cards = sec
                .albums
                .iter()
                .map(|a| Card {
                    artwork: Artwork::Remote(a.thumbnails.clone()),
                    title: a.title.clone(),
                    subtitle: format::artists(&a.artists),
                    shape: ArtShape::Tile,
                    on_press: Message::Ui(Event::Go(Route::Album(a.id.clone()))),
                    play: Some(Message::PlayCollection(DetailKey {
                        kind: DetailKind::Album,
                        id: a.id.clone(),
                    })),
                })
                .collect();
            c = c.push(card_grid(app, grid, cards, 1, 3 + index));
        }
        _ => {
            let cards = sec
                .tracks
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let play = Message::PlayAt {
                        source: ListSource::Daily(index),
                        index: i,
                    };
                    Card {
                        artwork: t.artwork.clone(),
                        title: t.title.clone(),
                        subtitle: format::artists(&t.artists),
                        shape: ArtShape::Tile,
                        on_press: play.clone(),
                        play: Some(play),
                    }
                })
                .collect();
            c = c.push(card_grid(app, grid, cards, 1, 3 + index));
        }
    }
    c.push(vspace(gap::XXL)).into()
}

/// The search text of a mood label: the curated query when it is one of ours.
fn mood_query(name: &str) -> String {
    crate::explore::MOODS
        .iter()
        .find(|m| m.label.eq_ignore_ascii_case(name))
        .map_or_else(|| format!("{name} music"), |m| m.query.to_owned())
}

/// The Videos and Podcasts chips: real shelves when the home feed gave them.
fn feed_kind_page(app: &App, grid: layout::Grid) -> Element<'_, Message> {
    use crate::explore::ContentFilter;
    if app.content_filter == ContentFilter::Videos && !app.daily.videos.is_empty() {
        let cards = app
            .daily
            .videos
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let play = Message::PlayAt {
                    source: ListSource::Videos,
                    index: i,
                };
                Card {
                    artwork: t.artwork.clone(),
                    title: t.title.clone(),
                    subtitle: format::artists(&t.artists),
                    shape: ArtShape::Tile,
                    on_press: play.clone(),
                    play: Some(play),
                }
            })
            .collect();
        return page(
            grid.content,
            column![
                section_title("Music videos"),
                label(
                    "Plays the audio of the video.",
                    type_scale::LABEL_MD,
                    false,
                    col(color::on_surface_variant())
                ),
                vspace(gap::MD),
                card_grid(app, grid, cards, 4, 0),
            ],
        );
    }
    if app.content_filter == ContentFilter::Podcasts && !app.daily.podcasts.is_empty() {
        let cards = app
            .daily
            .podcasts
            .iter()
            .map(|p| Card {
                artwork: Artwork::Remote(p.thumbnails.clone()),
                title: p.title.clone(),
                subtitle: p.author.clone().unwrap_or_else(|| "Podcast".to_owned()),
                shape: ArtShape::Tile,
                on_press: Message::Noop,
                play: None,
            })
            .collect();
        return page(
            grid.content,
            column![
                section_title("Podcasts"),
                label(
                    "Show pages are not built yet.",
                    type_scale::LABEL_MD,
                    false,
                    col(color::on_surface_variant())
                ),
                vspace(gap::MD),
                card_grid(app, grid, cards, 4, 0),
            ],
        );
    }
    let (title, detail, glyph) = if app.content_filter == ContentFilter::Videos {
        (
            "No music videos yet",
            "The home feed has none right now, or you are offline.",
            Icon::Video,
        )
    } else {
        (
            "No podcasts yet",
            "The home feed has none right now, or you are offline.",
            Icon::Podcasts,
        )
    };
    page(
        grid.content,
        status(
            glyph,
            title,
            detail,
            Some(("Back to All", Message::ContentFilter(ContentFilter::All))),
        ),
    )
}
