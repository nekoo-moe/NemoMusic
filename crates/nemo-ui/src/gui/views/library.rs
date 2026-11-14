//! Library: tabs (Overview, Playlists, Songs, Albums, Artists, Local), sort, list or grid, and a
//! "Create playlist" button. Account sections come first and local files after them, so a
//! signed-in account never sees "empty" above its own data. Hook point: the parent can send the
//! account library with `Message::AccountLibrary`; until then the shelves of Home that come from
//! the account are used.

use iced::font::Weight;
use iced::widget::{Space, button, column, row};
use iced::{Alignment, Color, Element, Length, Padding};

use super::super::app::{AccountMsg, App, CreateMsg, ListSource, Message};
use super::super::assets::Icon;
use super::super::style::{animated, col, gap, icon, icon_button, label, label_w, tip};
use super::kit::{page, page_title, section_title, text_button, tile, tonal_button, vspace};
use crate::explore::{LibLayout, LibTab, sort_by_key};
use crate::format;
use crate::layout;
use crate::theme::{color, shape, type_scale};

mod collection;
mod list;
mod section;

use collection::{account_data, album_coll, local_tracks, sorted};
use list::{collections, song_block};
use section::{account_overview, local_section};

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
