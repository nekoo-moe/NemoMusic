//! The library list of the left pane: header, kind chips and the rows of playlists, artists and albums.

use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Color, Element, Length, Padding};
use nemo_core::catalog::{AlbumSummary, ArtistSummary, PlaylistSummary};
use nemo_core::track::Artwork;

use crate::format;
use crate::gui::app::{App, ArtShape, LibFilter, Message, Source};
use crate::gui::assets::Icon;
use crate::gui::fx::entrance;
use crate::gui::style::{animated, col, fit, gap, icon, icon_button, label, scroll, tip};
use crate::gui::views::kit::{LIST_ITEM_H, Skeleton, art, blank, chip_padded, pane_bg, swap};
use crate::route::Route;
use crate::skeleton::Region;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// Height of a library row.
const LIB_ROW_H: f32 = LIST_ITEM_H;

pub(super) fn library_header(app: &App) -> Element<'_, Message> {
    let lang = app.ui.settings.language;
    row![
        container(icon(Icon::Library, 24.0, col(color::on_surface_variant())))
            .padding(Padding::from([0.0, gap::SM])),
        label(
            crate::i18n::t(lang, "Your library", "Thư viện của bạn"),
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
            crate::i18n::t(lang, "Create playlist", "Tạo danh sách phát"),
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
            crate::i18n::t(lang, "Open library", "Mở thư viện"),
        ),
    ]
    .align_y(Alignment::Center)
    .height(Length::Fixed(40.0))
    .into()
}

pub(super) fn library_chips(app: &App) -> Element<'_, Message> {
    let accent = app.accent();
    let lang = app.ui.settings.language;
    let mut r = row![].spacing(gap::SM);
    for f in LibFilter::ALL {
        let on = app.lib_filter == Some(f);
        r = r.push(chip_padded(
            f.label_localized(lang),
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

/// Most rows the list draws (a long library scrolls; the Library page has the rest).
const LIB_ROWS_MAX: usize = 200;

/// The rows of the list: the signed-in account's own playlists (Liked music first), followed
/// artists and saved albums, then the albums of local files. Never the suggestions on Home.
fn library_rows(app: &App) -> Vec<LibRow> {
    let mut playlists: Vec<LibRow> = Vec::new();
    let mut artists: Vec<LibRow> = Vec::new();
    let mut albums: Vec<LibRow> = Vec::new();
    let lang = app.ui.settings.language;
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
        subtitle: crate::i18n::t(lang, "Artist", "Nghệ sĩ").to_owned(),
        artwork: Artwork::Remote(a.thumbnails.clone()),
        route: Route::Artist(a.id.clone()),
    };
    let playlist_row = |p: &PlaylistSummary| LibRow {
        kind: LibFilter::Playlists,
        title: p.title.clone(),
        subtitle: format!(
            "{} \u{b7} {}",
            crate::i18n::t(lang, "Playlist", "Danh sách phát"),
            p.author
                .clone()
                .unwrap_or_else(|| "YouTube Music".to_owned())
        ),
        artwork: Artwork::Remote(p.thumbnails.clone()),
        route: Route::Playlist(p.id.clone()),
    };
    if app.source == Source::YouTube {
        if let Some(lib) = app.account_lists() {
            playlists.extend(lib.playlists.iter().map(playlist_row));
            artists.extend(lib.artists.iter().map(artist_row));
            albums.extend(lib.albums.iter().map(album_row));
        }
        albums.extend(app.local_albums.iter().map(album_row));
    } else if app.sc_is_signed_in() {
        let data = app.sc_data();
        let pl_str = crate::i18n::t(lang, "Playlist", "Danh sách phát");
        let ar_str = crate::i18n::t(lang, "Artist", "Nghệ sĩ");
        playlists.extend(data.playlists.iter().map(|p| {
            let mut row = playlist_row(p);
            row.subtitle = sc_subtitle(pl_str, p.author.as_deref());
            row
        }));
        artists.extend(data.artists.iter().map(|a| {
            let mut row = artist_row(a);
            row.subtitle = sc_subtitle(ar_str, None);
            row
        }));
        albums.extend(data.albums.iter().map(|a| {
            let mut row = album_row(a);
            row.subtitle = sc_subtitle("Album", Some(&format::artists(&a.artists)));
            row
        }));
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
    rows.truncate(LIB_ROWS_MAX);
    rows
}

/// The second line of a SoundCloud row: what it is and whose.
fn sc_subtitle(kind: &str, whose: Option<&str>) -> String {
    whose.map_or_else(|| kind.to_owned(), |w| format!("{kind} \u{b7} {w}"))
}

/// What the list says when it has no rows.
fn empty_hint(app: &App) -> &'static str {
    let lang = app.ui.settings.language;
    if app.source == Source::SoundCloud {
        return if !app.sc_is_signed_in() {
            crate::i18n::t(
                lang,
                "Sign in to SoundCloud in Settings to see your playlists, artists and albums here.",
                "Đăng nhập SoundCloud trong Cài đặt để xem danh sách phát, nghệ sĩ và album tại đây.",
            )
        } else if app.sc_lib.phase == crate::gui::app::ScPhase::Failed {
            crate::i18n::t(
                lang,
                "Could not load your SoundCloud library. Press Refresh (F5) to try again.",
                "Không thể tải thư viện SoundCloud của bạn. Nhấn Làm mới (F5) để thử lại.",
            )
        } else if app.sc_lib.phase == crate::gui::app::ScPhase::Done {
            crate::i18n::t(
                lang,
                "Nothing saved on SoundCloud yet.",
                "Chưa có nội dung nào được lưu trên SoundCloud.",
            )
        } else {
            crate::i18n::t(
                lang,
                "Loading your SoundCloud library\u{2026}",
                "Đang tải thư viện SoundCloud của bạn\u{2026}",
            )
        };
    }
    if !app.signed_in() {
        crate::i18n::t(
            lang,
            "Sign in to YouTube Music to see your playlists, albums and artists here.",
            "Đăng nhập YouTube Music để xem danh sách phát, album và nghệ sĩ tại đây.",
        )
    } else if app.account_lists().is_some() {
        crate::i18n::t(
            lang,
            "Nothing saved here yet.",
            "Chưa có nội dung nào được lưu tại đây.",
        )
    } else if app.library_failed {
        crate::i18n::t(
            lang,
            "Could not load your library. Press Refresh (F5) to try again.",
            "Không thể tải thư viện của bạn. Nhấn Làm mới (F5) để thử lại.",
        )
    } else {
        crate::i18n::t(
            lang,
            "Loading your library\u{2026}",
            "Đang tải thư viện của bạn\u{2026}",
        )
    }
}

/// Titles of the rows the list shows now (tests).
#[cfg(test)]
pub(crate) fn row_titles(app: &App) -> Vec<String> {
    library_rows(app).into_iter().map(|r| r.title).collect()
}

/// The text of the list when it is empty (tests).
#[cfg(test)]
pub(crate) fn hint(app: &App) -> &'static str {
    empty_hint(app)
}

/// What the list says when it has no rows. While the library loads it says nothing: the
/// skeleton stands in after its 150 ms wait.
fn empty_list(app: &App, width: f32) -> Element<'_, Message> {
    if app.sidebar_loading() {
        return blank();
    }
    container(
        label(
            empty_hint(app),
            type_scale::BODY_SM,
            false,
            col(color::on_surface_variant()),
        )
        .wrapping(iced::widget::text::Wrapping::Word)
        .width(Length::Fixed(width - 2.0 * gap::MD)),
    )
    .padding(gap::MD)
    .into()
}

/// Rows of the skeleton list.
const LOADING_ROWS: usize = 6;

/// The list while the account library loads: skeleton rows with the spacing of the real ones.
fn loading_list<'a>(sk: Skeleton, width: f32) -> Element<'a, Message> {
    let mut list = column![].spacing(gap::XS);
    for _ in 0..LOADING_ROWS {
        list = list.push(sk.list_item(width));
    }
    list.into()
}

pub(super) fn library_list<'a>(app: &'a App, width: f32, route: &Route) -> Element<'a, Message> {
    let rows = library_rows(app);
    if rows.is_empty() {
        return swap(
            app,
            Region::Sidebar,
            || empty_list(app, width),
            |sk| loading_list(sk, width),
        );
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
