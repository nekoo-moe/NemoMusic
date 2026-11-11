//! Library: tabs (Overview, Playlists, Songs, Albums, Artists, Local), sort, list or grid, and a
//! "Create playlist" button. Account sections come first and local files after them, so a
//! signed-in account never sees "empty" above its own data. Hook point: the parent can send the
//! account library with `Message::AccountLibrary`; until then the shelves of Home that come from
//! the account are used.

use iced::font::Weight;
use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Color, Element, Length, Padding};
use nemo_core::catalog::{AlbumSummary, ArtistSummary, PlaylistSummary, ShelfItem};
use nemo_core::track::{Artwork, Track};

use super::super::app::{
    AccountMsg, App, ArtShape, CreateMsg, DetailKey, DetailKind, ListSource, Message,
    SettingsSection,
};
use super::super::assets::Icon;
use super::super::style::{animated, col, fit, gap, icon, icon_button, label, label_w, tip};
use super::kit::{
    Card, RowCtx, art, card_grid, page, page_title, paragraph, section_title, text_button, tile,
    tonal_button, track_list, vspace,
};
use crate::explore::{LibLayout, LibSort, LibTab, sort_by_key};
use crate::format;
use crate::layout;
use crate::route::Route;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// A playlist, album or artist of the library.
#[derive(Clone)]
struct Coll {
    title: String,
    subtitle: String,
    artwork: Artwork,
    route: Route,
    circle: bool,
    play: Option<Message>,
}

/// What the account has: from the parent's hook, else from the account's shelves on Home.
#[derive(Default)]
struct AccountData {
    playlists: Vec<Coll>,
    albums: Vec<Coll>,
    artists: Vec<Coll>,
    songs: Vec<Track>,
}

fn playlist_coll(p: &PlaylistSummary) -> Coll {
    Coll {
        title: p.title.clone(),
        subtitle: format!(
            "Playlist \u{b7} {}",
            p.author
                .clone()
                .unwrap_or_else(|| "YouTube Music".to_owned())
        ),
        artwork: Artwork::Remote(p.thumbnails.clone()),
        route: Route::Playlist(p.id.clone()),
        circle: false,
        play: Some(Message::PlayCollection(DetailKey {
            kind: DetailKind::Playlist,
            id: p.id.clone(),
        })),
    }
}

fn album_coll(a: &AlbumSummary) -> Coll {
    Coll {
        title: a.title.clone(),
        subtitle: format!("Album \u{b7} {}", format::artists(&a.artists)),
        artwork: Artwork::Remote(a.thumbnails.clone()),
        route: Route::Album(a.id.clone()),
        circle: false,
        play: Some(Message::PlayCollection(DetailKey {
            kind: DetailKind::Album,
            id: a.id.clone(),
        })),
    }
}

fn artist_coll(a: &ArtistSummary) -> Coll {
    Coll {
        title: a.name.clone(),
        subtitle: "Artist".to_owned(),
        artwork: Artwork::Remote(a.thumbnails.clone()),
        route: Route::Artist(a.id.clone()),
        circle: true,
        play: None,
    }
}

fn account_data(app: &App) -> AccountData {
    let mut d = AccountData::default();
    if let Some(lib) = &app.account_library {
        d.playlists = lib.playlists.iter().map(playlist_coll).collect();
        d.albums = lib.albums.iter().map(album_coll).collect();
        d.artists = lib.artists.iter().map(artist_coll).collect();
        d.songs.clone_from(&lib.songs);
        return d;
    }
    if !app.signed_in() {
        return d;
    }
    for shelf in &app.home.shelves {
        for item in &shelf.items {
            match item {
                ShelfItem::Playlist(p) => d.playlists.push(playlist_coll(p)),
                ShelfItem::Album(a) => d.albums.push(album_coll(a)),
                ShelfItem::Artist(a) => d.artists.push(artist_coll(a)),
                ShelfItem::Song(t) if shelf.title == "Liked songs" => d.songs.push(t.clone()),
                ShelfItem::Song(_) => {}
            }
        }
    }
    dedupe(&mut d.playlists);
    dedupe(&mut d.albums);
    dedupe(&mut d.artists);
    d
}

fn dedupe(v: &mut Vec<Coll>) {
    let mut seen: Vec<Route> = Vec::new();
    v.retain(|c| {
        if seen.contains(&c.route) {
            false
        } else {
            seen.push(c.route.clone());
            true
        }
    });
}

fn sorted(mut v: Vec<Coll>, sort: LibSort) -> Vec<Coll> {
    sort_by_key(&mut v, sort, |c| c.title.clone(), |c| c.subtitle.clone());
    v
}

fn local_albums(app: &App) -> Vec<Coll> {
    sorted(
        app.local_albums.iter().map(album_coll).collect(),
        app.lib_sort,
    )
}

fn local_tracks(app: &App) -> Vec<Track> {
    let mut v = app.local_tracks.clone();
    sort_by_key(
        &mut v,
        app.lib_sort,
        |t| t.title.clone(),
        |t| format::artists(&t.artists),
    );
    v
}

/// The Library screen.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid_in(w);
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let content_w = grid.content as f32;
    let mut c = column![
        header(app, content_w),
        vspace(gap::MD),
        tabs(app, content_w)
    ]
    .width(Length::Fill)
    .push(vspace(gap::LG));

    let data = account_data(app);
    let signed_in = app.signed_in();
    match app.lib_tab {
        LibTab::Overview => {
            if signed_in {
                let any = !(data.playlists.is_empty()
                    && data.albums.is_empty()
                    && data.artists.is_empty()
                    && data.songs.is_empty());
                if any {
                    c = c.push(account_overview(app, &data, grid, content_w));
                } else {
                    c = c.push(note(
                        app,
                        "Your account library is loading or has nothing saved yet.",
                        None,
                    ));
                }
            } else {
                c = c.push(sign_in_note(app, content_w));
            }
            c = c.push(local_section(app, grid, content_w, 1, 6));
        }
        LibTab::Playlists => {
            if signed_in {
                c = c.push(collections(
                    app,
                    sorted(data.playlists, app.lib_sort),
                    grid,
                    content_w,
                    "No playlists in your account yet.",
                ));
            } else {
                c = c.push(sign_in_note(app, content_w));
            }
        }
        LibTab::Albums => {
            let mut all = data.albums;
            all.extend(app.local_albums.iter().map(album_coll));
            c = c.push(collections(
                app,
                sorted(all, app.lib_sort),
                grid,
                content_w,
                "No albums yet.",
            ));
        }
        LibTab::Artists => {
            if signed_in {
                c = c.push(collections(
                    app,
                    sorted(data.artists, app.lib_sort),
                    grid,
                    content_w,
                    "No followed artists yet.",
                ));
            } else {
                c = c.push(sign_in_note(app, content_w));
            }
        }
        LibTab::Songs => {
            if signed_in && !data.songs.is_empty() {
                let mut liked = data.songs;
                sort_by_key(
                    &mut liked,
                    app.lib_sort,
                    |t| t.title.clone(),
                    |t| format::artists(&t.artists),
                );
                c = c
                    .push(section_title("Liked songs"))
                    .push(vspace(gap::SM))
                    .push(song_block(app, liked, ListSource::Account, content_w, grid))
                    .push(vspace(gap::XL));
            } else if !signed_in {
                c = c.push(sign_in_note(app, content_w));
            }
            let local = local_tracks(app);
            if !local.is_empty() {
                c = c
                    .push(section_title("On this computer"))
                    .push(vspace(gap::SM))
                    .push(song_block(app, local, ListSource::Library, content_w, grid));
            } else if signed_in && app.local_tracks.is_empty() {
                c = c.push(note(app, "No songs yet.", None));
            }
        }
        LibTab::Local => {
            c = c.push(local_section(app, grid, content_w, 3, usize::MAX));
        }
    }
    page(grid.content, c)
}

/// Title row with the Create playlist button.
fn header(app: &App, _width: f32) -> Element<'_, Message> {
    let _ = app;
    row![
        page_title("Library"),
        Space::new().width(Length::Fill),
        tonal_button(
            "Create playlist",
            Some(Icon::Add),
            Some(Message::Create(CreateMsg::Open)),
        ),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// Tabs on the left; sort and list or grid on the right.
fn tabs(app: &App, width: f32) -> Element<'_, Message> {
    let accent = app.accent();
    let dim = col(color::on_surface_variant());
    let ink = col(color::on_surface());
    let mut r = row![].spacing(gap::SM).align_y(Alignment::Center);
    let labels: Vec<&str> = LibTab::ALL.iter().map(|t| t.label()).collect();
    let selected = LibTab::ALL
        .iter()
        .position(|t| *t == app.lib_tab)
        .unwrap_or(0);
    r = r.push(super::kit::sliding_chips(app, &labels, selected, 1, |i| {
        Message::LibTab(LibTab::ALL[i])
    }));
    r = r.push(Space::new().width(Length::Fill));
    let sort_label = format!("Sort: {}", app.lib_sort.label());
    let sort = button(
        row![
            icon(Icon::Sort, 18.0, ink),
            Space::new().width(gap::SM),
            label_w(sort_label, type_scale::LABEL_MD, Weight::Medium, ink),
        ]
        .align_y(Alignment::Center)
        .padding(Padding::from([0.0, gap::MD]))
        .height(Length::Fixed(36.0)),
    )
    .padding(0)
    .on_press(Message::LibSort(app.lib_sort.next()));
    r = r.push(tip(
        animated(sort, Color::TRANSPARENT, shape::FULL),
        "Change the sort order",
    ));
    for (layout_, glyph, tip_text) in [
        (LibLayout::List, Icon::List, "List"),
        (LibLayout::Grid, Icon::Grid, "Grid"),
    ] {
        let on = app.lib_layout == layout_;
        r = r.push(tip(
            icon_button(
                glyph,
                20.0,
                36.0,
                if on {
                    col(accent.on_secondary_container)
                } else {
                    dim
                },
                if on {
                    col(accent.secondary_container)
                } else {
                    Color::TRANSPARENT
                },
                Some(Message::LibLayout(layout_)),
            ),
            tip_text,
        ));
    }
    let _ = width;
    r.into()
}

/// A calm one-line note, with an optional action.
fn note<'a>(app: &App, text: &'a str, action: Option<(&'a str, Message)>) -> Element<'a, Message> {
    let _ = app;
    let mut r = row![
        icon(Icon::Info, 18.0, col(color::on_surface_variant())),
        Space::new().width(gap::MD),
        label(
            text,
            type_scale::BODY_MD,
            false,
            col(color::on_surface_variant())
        ),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([gap::MD, gap::LG]));
    if let Some((t, m)) = action {
        r = r.push(text_button(t, m));
    }
    column![tile(r, color::surface_container_high()), vspace(gap::LG)].into()
}

/// The signed-out prompt: the account sections need a sign-in.
fn sign_in_note(app: &App, _width: f32) -> Element<'_, Message> {
    note(
        app,
        "Sign in to see the playlists, albums, artists and liked songs of your YouTube Music account.",
        Some(("Sign in", Message::Account(AccountMsg::Open))),
    )
}

/// The account sections of the overview.
fn account_overview<'a>(
    app: &'a App,
    data: &AccountData,
    grid: layout::Grid,
    content_w: f32,
) -> Element<'a, Message> {
    let mut c = column![].width(Length::Fill);
    let sections = [
        ("Playlists", sorted(data.playlists.clone(), app.lib_sort)),
        ("Albums", sorted(data.albums.clone(), app.lib_sort)),
        ("Artists", sorted(data.artists.clone(), app.lib_sort)),
    ];
    for (i, (title, items)) in sections.into_iter().enumerate() {
        if items.is_empty() {
            continue;
        }
        c = c
            .push(section_title(title))
            .push(vspace(gap::MD))
            .push(collection_view(app, items, grid, content_w, 1, i, true))
            .push(vspace(gap::XL));
    }
    if !data.songs.is_empty() {
        let mut liked = data.songs.clone();
        sort_by_key(
            &mut liked,
            app.lib_sort,
            |t| t.title.clone(),
            |t| format::artists(&t.artists),
        );
        liked.truncate(6);
        c = c
            .push(section_title("Liked songs"))
            .push(vspace(gap::SM))
            .push(song_block(app, liked, ListSource::Account, content_w, grid))
            .push(vspace(gap::XL));
    }
    c.into()
}

/// "On this computer": the folders note, local albums and songs. An empty library is a small
/// note, never a full-page empty state.
fn local_section(
    app: &App,
    grid: layout::Grid,
    content_w: f32,
    album_rows: usize,
    song_limit: usize,
) -> Element<'_, Message> {
    let mut c = column![
        section_title("On this computer"),
        vspace(gap::SM),
        folders_note(app, content_w),
        vspace(gap::LG),
    ]
    .width(Length::Fill);
    let albums = local_albums(app);
    let tracks = local_tracks(app);
    if albums.is_empty() && tracks.is_empty() {
        let text = if app.library_loaded {
            "No music files found yet. Add a folder in Settings."
        } else {
            "Looking for your music\u{2026}"
        };
        return c.push(note(app, text, None)).into();
    }
    if !albums.is_empty() {
        c = c
            .push(collection_view(
                app, albums, grid, content_w, album_rows, 8, true,
            ))
            .push(vspace(gap::XL));
    }
    if !tracks.is_empty() {
        let shown: Vec<Track> = tracks.into_iter().take(song_limit).collect();
        c = c.push(song_block(app, shown, ListSource::Library, content_w, grid));
    }
    c.into()
}

/// A whole tab of collections.
fn collections<'a>(
    app: &'a App,
    items: Vec<Coll>,
    grid: layout::Grid,
    content_w: f32,
    empty: &'a str,
) -> Element<'a, Message> {
    if items.is_empty() {
        return note(app, empty, None);
    }
    collection_view(app, items, grid, content_w, 12, 0, false)
}

/// Collections as a card grid or as rows, by the layout toggle.
fn collection_view(
    app: &App,
    items: Vec<Coll>,
    grid: layout::Grid,
    content_w: f32,
    max_rows: usize,
    base: usize,
    force_grid: bool,
) -> Element<'_, Message> {
    if force_grid || app.lib_layout == LibLayout::Grid {
        let cards: Vec<Card> = items
            .into_iter()
            .map(|c| Card {
                artwork: c.artwork,
                title: c.title,
                subtitle: c.subtitle,
                shape: if c.circle {
                    ArtShape::Circle
                } else {
                    ArtShape::Tile
                },
                on_press: Message::Ui(Event::Go(c.route)),
                play: c.play,
            })
            .collect();
        return card_grid(app, grid, cards, max_rows, base);
    }
    let text_w = (content_w - 48.0 - gap::MD - 2.0 * gap::SM - 24.0).max(120.0);
    let mut list = column![].spacing(gap::XS);
    for c in items.into_iter().take(max_rows.saturating_mul(6).max(6)) {
        let shape_ = if c.circle {
            ArtShape::Circle
        } else {
            ArtShape::Thumb
        };
        let line = row![
            art(app, &c.artwork, 48, shape_),
            Space::new().width(gap::MD),
            column![
                label(
                    fit(&c.title, text_w, type_scale::BODY_MD.size),
                    type_scale::BODY_MD,
                    true,
                    col(color::on_surface())
                ),
                label(
                    fit(&c.subtitle, text_w, type_scale::BODY_SM.size),
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
                .height(Length::Fixed(60.0))
                .padding(Padding::from([0.0, gap::SM]))
                .align_y(Alignment::Center),
        )
        .padding(0)
        .width(Length::Fill)
        .on_press(Message::Ui(Event::Go(c.route)));
        list = list.push(animated(b, Color::TRANSPARENT, shape::lg()));
    }
    list.into()
}

/// Songs as rows or, in grid mode, as cards that play.
#[allow(clippy::needless_pass_by_value)] // callers build the list per frame and hand it over
fn song_block(
    app: &App,
    tracks: Vec<Track>,
    source: ListSource,
    content_w: f32,
    grid: layout::Grid,
) -> Element<'_, Message> {
    if app.lib_layout == LibLayout::Grid {
        let cards: Vec<Card> = tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let play = Message::PlayAt {
                    source: source.clone(),
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
        return card_grid(app, grid, cards, 8, 0);
    }
    let ctx = RowCtx {
        width: content_w,
        source,
        numbered: false,
        show_album: true,
    };
    track_list(app, &tracks, app.rows_limit, &ctx)
}

/// The "Music folders" note: where local music comes from, and where to change it.
fn folders_note(app: &App, width: f32) -> Element<'_, Message> {
    let dim = col(color::on_surface_variant());
    let list = if app.music_folders.is_empty() {
        "No folders added yet.".to_owned()
    } else {
        app.music_folders.join("   \u{b7}   ")
    };
    tile(
        row![
            icon(Icon::Folder, 20.0, dim),
            Space::new().width(gap::MD),
            column![
                label(
                    "Music folders",
                    type_scale::BODY_MD,
                    true,
                    col(color::on_surface())
                ),
                paragraph(list, type_scale::BODY_SM, dim, (width - 240.0).max(200.0)),
            ],
            Space::new().width(Length::Fill),
            text_button(
                "Manage in Settings",
                Message::SettingsSectionGo(SettingsSection::Library),
            ),
        ]
        .align_y(Alignment::Center)
        .padding(Padding::from([gap::MD, gap::LG])),
        color::surface_container_high(),
    )
}
