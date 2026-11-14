//! Library sections: the account overview, "On this computer" and the music folders note.

use iced::widget::{Space, column, row};
use iced::{Alignment, Element, Length, Padding};
use nemo_core::track::Track;

use super::collection::{AccountData, local_albums, local_tracks, sorted};
use super::list::{collection_view, song_block};
use super::note;
use crate::explore::sort_by_key;
use crate::format;
use crate::gui::app::{App, ListSource, Message, SettingsSection};
use crate::gui::assets::Icon;
use crate::gui::style::{col, gap, icon, label};
use crate::gui::views::kit::{paragraph, section_title, text_button, tile, vspace};
use crate::layout;
use crate::theme::{color, type_scale};

/// The account sections of the overview.
pub(super) fn account_overview<'a>(
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
pub(super) fn local_section(
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
