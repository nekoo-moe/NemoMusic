//! Search: a query box (300 ms debounce in the reducer), the filter chips (All, Songs, Videos,
//! Albums, Artists, Community playlists), then for All YouTube Music's Top result card and the
//! result sections in the order YouTube Music ranked them, or one kind in full for a chip
//! (search-like-ytm). Enter plays the top song.

use iced::widget::{Space, column, container, stack, text_input};
use iced::{Alignment, Border, Element, Length, Padding};

use super::super::app::{App, Message, SEARCH_ID};
use super::super::assets::Icon;
use super::super::style::{col, gap, icon, label};
use super::kit::{blank, page, page_title, status, swap, vspace};
use crate::layout;
use crate::skeleton::Region;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

mod filter;
mod section;
mod skeleton;
mod top;

pub(crate) use filter::{index as filter_index, labels as filter_labels};

/// The Search screen.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid_in(w);
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let content_w = grid.content as f32;
    let search = &app.ui.search;

    // The page searches the listening area it is in; nothing here chooses a service.
    let mut c = column![
        page_title("Search"),
        vspace(gap::LG),
        query_box(app, content_w)
    ]
    .width(Length::Fill);
    c = c.push(vspace(gap::XL));

    if !app.ui.youtube_enabled() && !app.search_is_soundcloud() {
        c = c.push(status(
            Icon::Info,
            "YouTube Music is waiting",
            "Read the short notice to search YouTube Music. Your own files work without it.",
            Some(("Review notice", Message::Ui(Event::TosReview))),
        ));
        return page(grid.content, c);
    }

    if search.query().is_empty() {
        c = c.push(status(
            Icon::Search,
            "Find something to play",
            if app.search_is_soundcloud() {
                "Search SoundCloud tracks, albums, artists and playlists. Press Enter to play the top track."
            } else {
                "Search songs, albums, artists and playlists. Press Enter to play the top song."
            },
            None,
        ));
        return page(grid.content, c);
    }
    c = c
        .push(filter::chips(app, search.filter))
        .push(vspace(gap::XL));
    let frame = section::Frame {
        grid,
        width: content_w,
        full: search.filter.is_some(),
    };
    c = c.push(swap(
        app,
        Region::Search,
        || match search.filter {
            Some(kind) => filter::filtered(app, kind, &frame),
            None => all(app, &frame),
        },
        |sk| skeleton::view(sk, &frame, search.filter),
    ));
    page(grid.content, c)
}

/// The "All" answer: the Top result card, then the sections in YouTube Music's order.
fn all<'a>(app: &'a App, frame: &section::Frame) -> Element<'a, Message> {
    let search = &app.ui.search;
    let mut c = column![];
    let results = search.results.as_ref();
    if let Some(r) = results {
        if r.is_empty() && !search.loading {
            c = c.push(no_results(search.query()));
        }
        if let Some(top) = &r.top {
            c = c.push(top::top_card(app, top, frame.width));
        }
        // YouTube Music's order of the kinds, then the rest.
        for (i, kind) in r.sections().into_iter().enumerate() {
            if let Some(s) = section::section(app, r, kind, frame, i) {
                c = c.push(s);
            }
        }
    } else if let Some(e) = &search.error {
        c = c.push(status(
            Icon::Info,
            "Search did not work",
            format!("{e} Check your connection and try again."),
            Some(("Try again", Message::Ui(Event::SearchRetry))),
        ));
    } else {
        // Pending: the skeleton comes after its 150 ms wait.
        c = c.push(blank());
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
    c.into()
}

/// The empty answer.
pub(super) fn no_results(query: &str) -> Element<'_, Message> {
    status(
        Icon::Search,
        format!("No results for \u{201c}{query}\u{201d}"),
        "Check the spelling or try fewer words.",
        None,
    )
}

/// The query box: a pill, height 48, glyph on the left, "Searching" hint on the right. The pill
/// gets a 2 px accent ring while it has focus.
fn query_box(app: &App, width: f32) -> Element<'_, Message> {
    let dim = col(color::on_surface_variant());
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
                background: iced::Background::Color(col(color::surface_container_high())),
                border: Border {
                    width: if focused { 2.0 } else { 0.0 },
                    radius: shape::FULL.into(),
                    color: ring,
                },
                icon: col(color::on_surface_variant()),
                placeholder: col(color::on_surface_variant()),
                value: col(color::on_surface()),
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
