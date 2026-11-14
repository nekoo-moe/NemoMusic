//! The Classic style of Now Playing (now-playing-styles, criterion 8): one calm column.
//!
//! A large still cover (no breathing), title, artist, seek bar, controls, the honest quality
//! badge (it opens the signal path) and the lyrics offset control, then lyrics one tap away on
//! a tab under the controls (three lines and the provider footer; the other tab is the queue).
//! Plain `surface` behind everything: no canvas gradient, no glass, no new colour roles. It only
//! arranges the shared builders (`cover_art`, `seek_row`, `transport`, `badge`, the lyric line
//! and footer, the queue tab), so it shows the same facts as every style (criterion 5).
//! [`view`] is the full window, [`pane`] the right pane.

use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Color, Element, Length, Padding};

use super::super::app::{App, CompactTab, Message};
use super::super::assets::Icon;
use super::super::style::{animated, col, fit, gap, icon_button, label};
use super::bar::{badge, cover_art, seek_row, transport};
use super::lyrics::{self as lyr, Look};
use super::now_playing::pill_button;
use crate::format;
use crate::keys::{KeyAction, LYRICS_OFFSET_STEP_MS};
use crate::layout;
use crate::lyrics_view::LineRole;
use crate::state::{Event, LyricsState};
use crate::theme::{color, shape, type_scale};
use nemo_core::lyrics::Granularity;

/// Widest the column grows on a large window.
const MAX_COLUMN: f32 = 560.0;
/// Height the column needs below the cover in the full window: header, titles, seek bar,
/// controls, badge row, tabs, three lyric lines and the footer.
const BELOW_COVER: f32 = 460.0;
/// Smallest cover.
const MIN_COVER: f32 = 120.0;

/// Which surface the column is drawn for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Surface {
    Full,
    Pane,
}

/// The full-window Classic Now Playing: a close button, then the column.
pub fn view(app: &App, w: u32, h: u32) -> Element<'_, Message> {
    // Window and art sizes are small positive numbers, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let (wf, hf, art) = (w as f32, h as f32, layout::now_playing_art(w, h) as f32);
    let width = (wf - 2.0 * gap::XL).clamp(MIN_COVER, MAX_COLUMN);
    let cover = art.min(width).min(hf - BELOW_COVER).max(MIN_COVER);
    let header = row![
        icon_button(
            Icon::ChevronDown,
            24.0,
            40.0,
            col(color::on_surface()),
            Color::TRANSPARENT,
            Some(Message::Ui(Event::Back)),
        ),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([0.0, gap::XL]))
    .height(Length::Fixed(48.0));
    let page = column![
        header,
        container(body(app, width, cover, Surface::Full))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill),
    ];
    plain(page.into())
}

/// The Classic right pane, `width` px wide.
pub fn pane(app: &App, width: f32) -> Element<'_, Message> {
    let inner = width - 2.0 * gap::MD;
    let (w, h) = app.window_px();
    // Pixel sizes are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let cover = (layout::pane_art(w, h) as f32).min(inner);
    plain(
        container(body(app, inner, cover, Surface::Pane))
            .padding(Padding {
                top: gap::MD,
                ..Padding::ZERO
            })
            .width(Length::Fixed(width))
            .height(Length::Fill)
            .center_x(Length::Fixed(width))
            .into(),
    )
}

/// `content` on the plain `surface` colour.
fn plain(content: Element<'_, Message>) -> Element<'_, Message> {
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(col(color::surface()).into()),
            ..container::Style::default()
        })
        .into()
}

/// The column: cover, titles, seek bar, controls, badge and offset, tabs, tab content.
fn body(app: &App, width: f32, cover: f32, surface: Surface) -> Element<'_, Message> {
    let accent = app.accent();
    let play = if surface == Surface::Full { 56.0 } else { 48.0 };
    column![
        cover_art(app, cover, shape::xl()),
        Space::new().height(gap::MD),
        titles(app, width, surface),
        Space::new().height(gap::SM),
        container(seek_row(app, accent)).width(Length::Fixed(width)),
        transport(app, accent, play),
        Space::new().height(gap::SM),
        meta(app, width),
        Space::new().height(gap::MD),
        tabs(app, width),
        Space::new().height(gap::SM),
        container(match app.compact_tab {
            CompactTab::Lyrics => three_lines(app, width),
            CompactTab::Queue => super::queue::tab(app, width),
        })
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .clip(true),
    ]
    .align_x(Alignment::Center)
    .width(Length::Fixed(width))
    .into()
}

/// Narrower than this, the offset control goes under the badge instead of beside it.
const META_ROW_MIN: f32 = 520.0;

/// The badge (signal-path entry) and the offset control: one row, or two on a narrow column.
fn meta(app: &App, width: f32) -> Element<'_, Message> {
    if width >= META_ROW_MIN {
        row![badge(app, width * 0.6), offset_controls(app)]
            .spacing(gap::SM)
            .align_y(Alignment::Center)
            .into()
    } else {
        column![badge(app, width), offset_controls(app)]
            .spacing(gap::SM)
            .align_x(Alignment::Center)
            .into()
    }
}

/// Title and artist, centred.
fn titles(app: &App, width: f32, surface: Surface) -> Element<'_, Message> {
    let (title, artist) = app.ui.snapshot.current.as_ref().map_or_else(
        || ("Nothing playing".to_owned(), String::new()),
        |t| (t.title.clone(), format::artists(&t.artists)),
    );
    let (ts, as_) = if surface == Surface::Full {
        (type_scale::HEADLINE_SM, type_scale::TITLE_SM)
    } else {
        (type_scale::TITLE_LG, type_scale::BODY_MD)
    };
    column![
        label(
            fit(&title, width, ts.size),
            ts,
            true,
            col(color::on_surface())
        ),
        label(
            fit(&artist, width, as_.size),
            as_,
            false,
            col(color::on_surface_variant())
        ),
    ]
    .width(Length::Fixed(width))
    .align_x(Alignment::Center)
    .into()
}

/// The lyrics offset control (`-50 ms`, value, `+50 ms`, reset), only for synced lyrics.
fn offset_controls(app: &App) -> Element<'_, Message> {
    let synced =
        matches!(&app.ui.lyrics, LyricsState::Ready(d) if d.granularity != Granularity::Unsynced);
    if !synced {
        return Space::new().into();
    }
    row![
        pill_button(
            "-50\u{2009}ms",
            Message::Ui(Event::Key(KeyAction::LyricsOffsetBy(
                -LYRICS_OFFSET_STEP_MS
            ))),
        ),
        label(
            format::offset_seconds(app.ui.lyrics_offset_ms),
            type_scale::LABEL_SM,
            false,
            col(color::on_surface()),
        ),
        pill_button(
            "+50\u{2009}ms",
            Message::Ui(Event::Key(KeyAction::LyricsOffsetBy(LYRICS_OFFSET_STEP_MS))),
        ),
        pill_button("Reset", Message::Ui(Event::LyricsOffsetReset)),
    ]
    .spacing(gap::XS)
    .align_y(Alignment::Center)
    .into()
}

/// The two tabs under the controls: Lyrics and Queue.
fn tabs(app: &App, width: f32) -> Element<'_, Message> {
    let tab = |text: &'static str, which: CompactTab| {
        let on = app.compact_tab == which;
        let ink = if on {
            color::on_surface()
        } else {
            color::on_surface_variant()
        };
        let idle = if on {
            col(color::surface_container_high())
        } else {
            Color::TRANSPARENT
        };
        let b = button(
            container(label(text, type_scale::BODY_MD, on, col(ink)))
                .width(Length::Fill)
                .height(Length::Fixed(40.0))
                .center_x(Length::Fill)
                .center_y(Length::Fixed(40.0)),
        )
        .padding(0)
        .width(Length::Fill)
        .on_press(Message::CompactTab(which));
        animated(b, idle, shape::FULL)
    };
    container(
        row![
            tab("Lyrics", CompactTab::Lyrics),
            tab("Queue", CompactTab::Queue)
        ]
        .spacing(gap::SM),
    )
    .width(Length::Fixed(width))
    .into()
}

/// Three lyric lines (previous, current, next) and the provider footer.
fn three_lines(app: &App, width: f32) -> Element<'_, Message> {
    let LyricsState::Ready(doc) = &app.ui.lyrics else {
        return lyr::lyrics_status(app, width);
    };
    if doc.instrumental {
        return lyr::status_text("Instrumental", None);
    }
    let active = app.ui.active_lyric();
    let first = active
        .unwrap_or(0)
        .saturating_sub(1)
        .min(doc.lines.len().saturating_sub(3));
    let style = |role: LineRole| match role {
        LineRole::Active => type_scale::TITLE_MD,
        _ => type_scale::BODY_MD,
    };
    let role_of = |i: usize, at: Option<usize>| {
        if at.is_some() {
            LineRole::of(i, at)
        } else {
            LineRole::Other
        }
    };
    let before = if app.anim.line() < 1.0 {
        app.prev_line
    } else {
        active
    };
    let mut lines = column![].align_x(Alignment::Center).width(Length::Fill);
    for (i, line) in doc.lines.iter().enumerate().skip(first).take(3) {
        let (from, to) = (role_of(i, before), role_of(i, active));
        let look = Look {
            from: (from, style(from)),
            to: (to, style(to)),
        };
        lines = lines.push(lyr::lyric_line(app, line, width, look).0);
    }
    column![
        lines,
        Space::new().height(Length::Fill),
        lyr::footer(app, width),
        Space::new().height(gap::SM)
    ]
    .into()
}

#[cfg(test)]
mod tests;
