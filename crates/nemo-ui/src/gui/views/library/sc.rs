//! The SoundCloud Library (soundcloud-account criteria 11, 13, 14): Likes, Playlists, Albums,
//! Following and History of the signed-in account, in the layout of the YouTube Music Library
//! (the same rows, cards, skeleton and notes). Signed out, switched off, failed and empty each
//! say so in one plain sentence; a failure offers Retry.

use iced::widget::{Space, column, row};
use iced::{Alignment, Element, Length};
use nemo_core::catalog::{AlbumSummary, ArtistSummary, HistorySection, PlaylistSummary};
use nemo_core::track::Artwork;

use super::collection::{Coll, sorted};
use super::list::{collections, song_block};
use super::note;
use crate::gui::app::{
    App, DetailKey, DetailKind, ListSource, Message, ScMsg, ScPhase, ScTab, ScWriteMsg,
    SettingsSection,
};
use crate::gui::assets::Icon;
use crate::gui::style::gap;
use crate::gui::views::kit::{
    blank, page, page_title, section_title, sliding_chips, swap, tonal_button, vspace,
};
use crate::layout;
use crate::route::Route;
use crate::skeleton::Region;

/// The text of the sentence under a signed-out or switched-off Library.
const OFF: &str = "SoundCloud is off. Turn it on in Settings to see your SoundCloud library.";
/// Signed out.
const SIGNED_OUT: &str = "Sign in to SoundCloud in Settings to see your likes, playlists, the artists you follow and your history.";
/// The first read has not answered and nothing failed: only the skeleton shows.
const RETRY: &str = "Retry";

/// The SoundCloud Library screen.
pub(super) fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid_in(w);
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let content_w = grid.content as f32;
    let c = column![
        header(app),
        vspace(gap::MD),
        tabs(app),
        vspace(gap::LG),
        swap(
            app,
            Region::Library,
            || body(app, grid, content_w),
            |sk| skeleton(sk, app.sc_open_tab(), grid, content_w),
        ),
    ]
    .width(Length::Fill);
    page(grid.content, c)
}

fn header(app: &App) -> Element<'_, Message> {
    let mut r = row![page_title("Library"), Space::new().width(gap::LG)].align_y(Alignment::Center);
    if let Some(switch) = super::source_switch(app) {
        r = r.push(switch);
    }
    r.into()
}

fn tabs(app: &App) -> Element<'_, Message> {
    let labels: Vec<&str> = ScTab::ALL.iter().map(|t| t.label()).collect();
    let selected = ScTab::ALL
        .iter()
        .position(|t| *t == app.sc_open_tab())
        .unwrap_or(0);
    let chips = sliding_chips(app, &labels, selected, 3, |i| {
        Message::Sc(ScMsg::Tab(ScTab::ALL[i]))
    });
    // The history keeps SoundCloud's order (by day); every other list can be sorted and laid out
    // as a list or a grid, like the YouTube Music Library.
    if app.sc_open_tab() == ScTab::History {
        return chips;
    }
    row![
        chips,
        Space::new().width(Length::Fill),
        super::sort_and_layout(app)
    ]
    .align_y(Alignment::Center)
    .into()
}

/// What the open tab shows.
fn body(app: &App, grid: layout::Grid, content_w: f32) -> Element<'_, Message> {
    if !app.sc_open() {
        return note(app, OFF, Some(("Open settings", settings(app))));
    }
    if !app.sc_is_signed_in() {
        return note(app, SIGNED_OUT, Some(("Open settings", settings(app))));
    }
    // Wanted, but the skeleton waits 150 ms so a quick answer never flashes it.
    if app.sc_library_pending() {
        return blank();
    }
    let lib = &app.sc_lib;
    let mut c = column![].width(Length::Fill);
    if lib.phase == ScPhase::Failed {
        let text = lib.error.as_deref().unwrap_or(RETRY);
        c = c.push(note(app, text, Some((RETRY, Message::Sc(ScMsg::Reload)))));
    }
    if lib.phase == ScPhase::Failed && lib.data.likes.is_empty() && is_empty(app) {
        return c.into();
    }
    c.push(tab_body(app, grid, content_w)).into()
}

/// Nothing of any tab is held (a failed first read).
fn is_empty(app: &App) -> bool {
    let d = &app.sc_lib.data;
    d.likes.is_empty()
        && d.playlists.is_empty()
        && d.albums.is_empty()
        && d.artists.is_empty()
        && d.history.is_empty()
}

fn settings(_app: &App) -> Message {
    Message::SettingsSectionGo(SettingsSection::SoundCloud)
}

fn tab_body(app: &App, grid: layout::Grid, content_w: f32) -> Element<'_, Message> {
    let data = app.sc_data();
    match app.sc_open_tab() {
        ScTab::Likes => {
            if data.likes.is_empty() {
                return note(app, "No liked tracks yet.", None);
            }
            // The rows play in the order they are drawn in (`App::tracks_of`).
            song_block(
                app,
                app.tracks_of(&ListSource::ScLikes),
                ListSource::ScLikes,
                content_w,
                grid,
            )
        }
        ScTab::Playlists => {
            let list = collections(
                app,
                sorted(
                    data.playlists.iter().map(playlist_coll).collect(),
                    app.lib_sort,
                ),
                grid,
                content_w,
                "No playlists yet.",
            );
            if !app.sc_can_write() {
                return list;
            }
            column![
                tonal_button(
                    "New playlist",
                    Some(Icon::Add),
                    Some(Message::ScWrite(ScWriteMsg::NewPlaylist(None))),
                ),
                vspace(gap::LG),
                list,
            ]
            .into()
        }
        ScTab::Albums => collections(
            app,
            sorted(data.albums.iter().map(album_coll).collect(), app.lib_sort),
            grid,
            content_w,
            "No albums yet.",
        ),
        ScTab::Following => collections(
            app,
            sorted(data.artists.iter().map(artist_coll).collect(), app.lib_sort),
            grid,
            content_w,
            "You follow no artists yet.",
        ),
        ScTab::History => history(app, &data.history, grid, content_w),
    }
}

fn history<'a>(
    app: &'a App,
    sections: &[HistorySection],
    grid: layout::Grid,
    content_w: f32,
) -> Element<'a, Message> {
    if sections.is_empty() {
        return note(app, "Nothing played on SoundCloud yet.", None);
    }
    let mut c = column![].width(Length::Fill);
    for (i, s) in sections.iter().enumerate() {
        c = c
            .push(section_title(s.title.clone()))
            .push(vspace(gap::SM))
            .push(song_block(
                app,
                s.tracks.clone(),
                ListSource::ScHistory(i),
                content_w,
                grid,
            ))
            .push(vspace(gap::XL));
    }
    c.into()
}

fn playlist_coll(p: &PlaylistSummary) -> Coll {
    Coll {
        title: p.title.clone(),
        subtitle: format!(
            "Playlist \u{b7} {}",
            p.author.clone().unwrap_or_else(|| "SoundCloud".to_owned())
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
        subtitle: format!("Album \u{b7} {}", crate::format::artists(&a.artists)),
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

/// The loading skeleton of a tab, in the layout of the tab.
fn skeleton<'a>(
    sk: crate::gui::views::kit::Skeleton,
    tab: ScTab,
    grid: layout::Grid,
    content_w: f32,
) -> Element<'a, Message> {
    let mut c = column![sk.section_header(content_w * 0.2)];
    match tab {
        ScTab::Likes | ScTab::History => {
            for i in 0..7 {
                c = c.push(sk.row(content_w, i));
            }
        }
        ScTab::Playlists | ScTab::Albums => c = c.push(sk.cards(grid, 2, false)),
        ScTab::Following => c = c.push(sk.cards(grid, 2, true)),
    }
    c.push(vspace(gap::XXL)).into()
}
