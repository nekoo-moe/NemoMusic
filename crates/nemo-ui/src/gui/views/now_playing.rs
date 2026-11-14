//! Now Playing: art and lyrics side by side, or the compact column (`ui-reference.md`
//! section 4). The floating bar is hidden here; the transport lives under the art. No glass sits
//! behind lyric text: the backdrop is the song-coloured canvas, toned so every text ink keeps
//! 4.5:1 (`design.md` sections 9, 10; `crate::canvas`).

use iced::widget::{Space, button, column, container, row, scrollable, stack};
use iced::{Alignment, Color, Element, Length, Padding};

use super::super::app::{App, CompactTab, Message};
use super::super::assets::Icon;
use super::super::style::{animated, col, fit, gap, icon_button, label, label_w};
use super::bar::{badge, cover_art, seek_row, transport};
use super::lyrics::{self as lyr, Look};
use crate::format;
use crate::keys::{KeyAction, LYRICS_OFFSET_STEP_MS};
use crate::layout;
use crate::lyrics_view::LineRole;
use crate::settings::NowPlayingStyle;
use crate::state::{Event, LyricsState};
use crate::theme::{color, shape, type_scale};
use nemo_core::lyrics::Granularity;
use nemo_core::player::PlayerCommand;

/// Height of the lyrics viewport in the wide layout: window minus header (56), padding (2 x 24)
/// and the provider footer (24).
#[must_use]
pub fn lyrics_viewport_height(_w: u32, h: u32) -> f32 {
    // Window heights are small positive numbers.
    #[allow(clippy::cast_precision_loss)]
    let h = h as f32;
    (h - 56.0 - 2.0 * gap::XL - 24.0).max(120.0)
}

/// The full-window Now Playing screen, drawn by the builder of the chosen style. Every style
/// reads the same `App` state (position, lyrics offset and scroll, `compact_tab`), so a style
/// change restarts nothing.
pub fn view(app: &App, w: u32, h: u32) -> Element<'_, Message> {
    #[expect(
        clippy::match_same_arms,
        reason = "Classic and Apple Music get their own builders in now-playing-styles tasks 5 and 6"
    )]
    let page = match app.ui.settings.now_playing_style {
        NowPlayingStyle::M3Expressive => m3_expressive(app, w, h),
        // Until their builders land, these styles draw today's look.
        NowPlayingStyle::Classic | NowPlayingStyle::AppleMusic => m3_expressive(app, w, h),
    };
    stack![super::backdrop::full_backdrop(app), page].into()
}

/// The Material 3 Expressive look: art and lyrics side by side, or the compact column.
fn m3_expressive(app: &App, w: u32, h: u32) -> Element<'_, Message> {
    if layout::is_compact(w, h) {
        compact(app, w, h)
    } else {
        wide(app, w, h)
    }
}

fn close_button<'a>() -> Element<'a, Message> {
    icon_button(
        Icon::ChevronDown,
        24.0,
        40.0,
        col(color::on_surface()),
        Color::TRANSPARENT,
        Some(Message::Ui(Event::Back)),
    )
}

/// A small pill button (`label-md`, medium).
pub fn pill_button(text: &str, message: Message) -> Element<'_, Message> {
    animated(
        button(
            container(label_w(
                text,
                type_scale::LABEL_MD,
                iced::font::Weight::Medium,
                col(color::on_surface()),
            ))
            .padding(Padding::from([gap::SM, gap::LG])),
        )
        .padding(0)
        .on_press(message),
        col(color::on_surface().with_alpha(0.10)),
        shape::FULL,
    )
}

/// Lyrics offset control: `-50 ms`, value, `+50 ms`, reset (ADR 0002). Only for synced lyrics.
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
        container(label(
            format::offset_seconds(app.ui.lyrics_offset_ms),
            type_scale::LABEL_SM,
            false,
            col(color::on_surface()),
        ))
        .width(Length::Fixed(64.0))
        .center_x(Length::Fixed(64.0)),
        pill_button(
            "+50\u{2009}ms",
            Message::Ui(Event::Key(KeyAction::LyricsOffsetBy(LYRICS_OFFSET_STEP_MS))),
        ),
        Space::new().width(gap::SM),
        pill_button("Reset", Message::Ui(Event::LyricsOffsetReset)),
    ]
    .spacing(gap::XS)
    .align_y(Alignment::Center)
    .into()
}

fn header(app: &App, height: f32, with_offset: bool) -> Element<'_, Message> {
    let mut r = row![close_button(), Space::new().width(Length::Fill)].align_y(Alignment::Center);
    if with_offset {
        r = r
            .push(offset_controls(app))
            .push(Space::new().width(gap::LG));
    }
    r.push(badge(app, 300.0))
        .padding(Padding::from([0.0, gap::XL]))
        .height(Length::Fixed(height))
        .into()
}

fn titles(app: &App, width: f32, centered: bool) -> Element<'_, Message> {
    let (title, artist) = app.ui.snapshot.current.as_ref().map_or_else(
        || ("Nothing playing".to_owned(), String::new()),
        |t| (t.title.clone(), format::artists(&t.artists)),
    );
    let (ts, as_) = if centered {
        (type_scale::TITLE_MD, type_scale::BODY_MD)
    } else {
        (type_scale::HEADLINE_MD, type_scale::TITLE_SM)
    };
    let align = if centered {
        Alignment::Center
    } else {
        Alignment::Start
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
    .align_x(align)
    .into()
}

fn wide(app: &App, w: u32, h: u32) -> Element<'_, Message> {
    let art = layout::now_playing_art(w, h);
    let lyrics_w = layout::lyrics_column_width(w, art);
    // Pixel sizes are small, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let (art, lyrics_w) = (art as f32, lyrics_w as f32);
    let accent = app.accent();

    let left = column![
        super::pane::breathing(app, cover_art(app, art, shape::xl())),
        Space::new().height(gap::XL),
        titles(app, art, false),
        Space::new().height(gap::LG),
        seek_row(app, accent),
        Space::new().height(gap::MD),
        container(transport(app, accent, 56.0)).center_x(Length::Fill),
    ]
    .width(Length::Fixed(art));

    let right = lyrics_panel(app, lyrics_w, lyrics_viewport_height(w, h));
    let body = row![left, Space::new().width(gap::HUGE), right].align_y(Alignment::Center);
    column![
        header(app, 56.0, true),
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(Padding::from([gap::XL, gap::XL])),
    ]
    .into()
}

fn compact(app: &App, w: u32, h: u32) -> Element<'_, Message> {
    let art = layout::now_playing_art(w, h);
    // Pixel sizes are small, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let (art, width) = (art as f32, (w as f32 - 2.0 * gap::XL).min(480.0));
    let accent = app.accent();

    let tab = |text: &'static str, which: CompactTab| {
        let on = app.compact_tab == which;
        let idle = if on {
            col(color::surface_container_high())
        } else {
            Color::TRANSPARENT
        };
        let b = button(
            container(label(
                text,
                type_scale::BODY_MD,
                on,
                col(if on {
                    color::on_surface()
                } else {
                    color::on_surface_variant()
                }),
            ))
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
    let lower: Element<'_, Message> = match app.compact_tab {
        CompactTab::Lyrics => compact_lyrics(app, width),
        CompactTab::Queue => compact_queue(app, width),
    };
    let body = column![
        super::pane::breathing(app, cover_art(app, art, shape::xl())),
        Space::new().height(gap::MD),
        titles(app, width, true),
        Space::new().height(gap::MD),
        container(seek_row(app, accent)).width(Length::Fixed(width)),
        Space::new().height(gap::SM),
        transport(app, accent, 56.0),
        Space::new().height(gap::SM),
        container(
            row![
                tab("Lyrics", CompactTab::Lyrics),
                tab("Queue", CompactTab::Queue)
            ]
            .spacing(gap::SM)
        )
        .width(Length::Fixed(width)),
        Space::new().height(gap::SM),
        container(lower)
            .width(Length::Fixed(width))
            .height(Length::Fill),
    ]
    .align_x(Alignment::Center)
    .width(Length::Fill);
    column![header(app, 48.0, false), body].into()
}

/// Three lyric lines (previous, current, next) for the compact layout.
fn compact_lyrics(app: &App, width: f32) -> Element<'_, Message> {
    let LyricsState::Ready(doc) = &app.ui.lyrics else {
        return lyr::lyrics_status(app, width);
    };
    if doc.instrumental {
        return lyr::status_text("Instrumental", None);
    }
    let active = app.ui.active_lyric();
    let center = active.unwrap_or(0);
    let first = center
        .saturating_sub(1)
        .min(doc.lines.len().saturating_sub(3));
    // One step smaller than the wide layout so three lines fit above the footer.
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
    let mut c = column![].align_x(Alignment::Center).width(Length::Fill);
    for (i, line) in doc.lines.iter().enumerate().skip(first).take(3) {
        let (from, to) = (role_of(i, before), role_of(i, active));
        let look = Look {
            from: (from, style(from)),
            to: (to, style(to)),
        };
        c = c.push(lyr::lyric_line(app, line, width, look).0);
    }
    column![
        c,
        Space::new().height(Length::Fill),
        lyr::footer(app, width),
        Space::new().height(gap::SM)
    ]
    .into()
}

fn compact_queue(app: &App, width: f32) -> Element<'_, Message> {
    let snap = &app.ui.snapshot;
    let mut rows = column![];
    for (i, t) in snap.queue.iter().enumerate() {
        let current = snap.queue_index == Some(i);
        rows = rows.push(animated(
            button(
                container(
                    row![
                        label(
                            fit(&t.title, width * 0.55, type_scale::BODY_MD.size),
                            type_scale::BODY_MD,
                            current,
                            col(color::on_surface()),
                        ),
                        Space::new().width(Length::Fill),
                        label(
                            fit(
                                &format::artists(&t.artists),
                                width * 0.35,
                                type_scale::BODY_SM.size
                            ),
                            type_scale::BODY_SM,
                            false,
                            col(color::on_surface_variant()),
                        ),
                    ]
                    .align_y(Alignment::Center),
                )
                .height(Length::Fixed(40.0))
                .width(Length::Fill)
                .padding(Padding::from([0.0, gap::MD]))
                .align_y(Alignment::Center),
            )
            .padding(0)
            .width(Length::Fill)
            .on_press(Message::Ui(Event::Command(PlayerCommand::SetQueue {
                tracks: snap.queue.clone(),
                start: i,
            }))),
            Color::TRANSPARENT,
            shape::md(),
        ));
    }
    scrollable(rows).height(Length::Fill).into()
}

/// The lyrics column of the wide layout.
fn lyrics_panel(app: &App, width: f32, viewport_h: f32) -> Element<'_, Message> {
    column![
        container(lyr::body(app, width, false))
            .width(Length::Fill)
            .height(Length::Fixed(viewport_h - lyr::footer_extra(app, width)))
            .clip(true),
        lyr::footer(app, width),
    ]
    .width(Length::Fixed(width))
    .into()
}

#[cfg(test)]
mod tests;
