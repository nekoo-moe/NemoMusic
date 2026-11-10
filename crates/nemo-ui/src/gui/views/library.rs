//! Library: local albums and local tracks, plus a note about the music folders.

use iced::widget::{Space, column, row};
use iced::{Alignment, Element, Length, Padding};
use nemo_core::track::Artwork;

use super::super::app::{App, ArtShape, ListSource, Message, SettingsSection};
use super::super::assets::Icon;
use super::super::style::{col, gap, icon, label};
use super::kit::{
    Card, RowCtx, card_grid, page, page_title, paragraph, section_title, status, text_button, tile,
    track_list, vspace,
};
use crate::copy;
use crate::format;
use crate::layout;
use crate::route::Route;
use crate::state::Event;
use crate::theme::{color, type_scale};

/// The Library screen.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid_in(w);
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let content_w = grid.content as f32;
    let mut c = column![page_title("Library"), vspace(gap::LG)].width(Length::Fill);
    c = c.push(folders_note(app, content_w)).push(vspace(gap::XL));

    if app.local_tracks.is_empty() && app.local_albums.is_empty() {
        c = c.push(if app.library_loaded {
            status(
                Icon::Library,
                copy::LIBRARY_EMPTY_TITLE,
                copy::LIBRARY_EMPTY_HELP,
                Some((
                    "Open Settings",
                    Message::SettingsSectionGo(SettingsSection::Library),
                )),
            )
        } else {
            label(
                "Looking for your music\u{2026}",
                type_scale::BODY_MD,
                false,
                col(color::ON_SURFACE_VARIANT),
            )
            .into()
        });
        return page(grid.content, c);
    }

    if !app.local_albums.is_empty() {
        let cards = app
            .local_albums
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
            .push(card_grid(app, grid, cards, 3, 0))
            .push(vspace(gap::XXL));
    }
    if !app.local_tracks.is_empty() {
        let ctx = RowCtx {
            width: content_w,
            source: ListSource::Library,
            numbered: false,
            show_album: true,
        };
        c = c
            .push(section_title("Songs"))
            .push(vspace(gap::SM))
            .push(track_list(app, &app.local_tracks, app.rows_limit, &ctx));
    }
    page(grid.content, c)
}

/// The "Music folders" note: where music comes from, and where to change it.
fn folders_note(app: &App, width: f32) -> Element<'_, Message> {
    let dim = col(color::ON_SURFACE_VARIANT);
    let list = if app.music_folders.is_empty() {
        "No folders added yet.".to_owned()
    } else {
        app.music_folders.join("   \u{b7}   ")
    };
    tile(
        row![
            icon(Icon::Library, 20.0, dim),
            Space::new().width(gap::MD),
            column![
                label(
                    "Music folders",
                    type_scale::BODY_MD,
                    true,
                    col(color::ON_SURFACE)
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
        color::SURFACE_CONTAINER,
    )
}
