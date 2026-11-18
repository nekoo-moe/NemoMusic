//! Now Playing: art and lyrics side by side, or the compact column (`ui-reference.md`
//! section 4). The floating bar is hidden here; the transport lives under the art. No glass sits
//! behind lyric text: the backdrop is the song-coloured canvas, toned so every text ink keeps
//! 4.5:1 (`design.md` sections 9, 10; `crate::canvas`).

use iced::widget::{Space, button, column, container, row, scrollable, stack};
use iced::{Alignment, Color, Element, Length, Padding};

use super::super::app::{App, CompactTab, Message};
use super::super::assets::Icon;
use super::super::fx::{Fx, entrance};
use super::super::style::{animated, col, fit, gap, icon_button, label, label_w};
use super::bar::cover_art;
use super::lyrics as lyr;
use crate::anim;
use crate::format;
use crate::layout;
use crate::settings::NowPlayingStyle;
use crate::state::{Event, LyricsState};
use crate::theme::{color, shape, type_scale};
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
    // Opening (or leaving for) this screen is a route change: it fades in like every page.
    entrance(styled(app, w, h), app.anim.page(), col(color::surface()))
}

fn styled(app: &App, w: u32, h: u32) -> Element<'_, Message> {
    match app.ui.settings.now_playing_style {
        // Classic draws its own plain `surface`, so it needs no cover backdrop.
        NowPlayingStyle::Classic => super::np_classic::view(app, w, h),
        // The Apple Music style draws today's look until its builder lands (task 6).
        NowPlayingStyle::M3Expressive | NowPlayingStyle::AppleMusic => stack![
            super::backdrop::full_backdrop(app),
            m3_expressive(app, w, h)
        ]
        .into(),
    }
}

/// The cover grows from 86% to 100% with the page fade: it seems to come out of the capsule.
fn grow<'a>(app: &App, cover: Element<'a, Message>) -> Element<'a, Message> {
    Fx::new(cover)
        .passive()
        .scale(anim::GROW_FROM + (1.0 - anim::GROW_FROM) * app.anim.page())
        .into()
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

/// The top bar: close, then the Timing icon (full window only) and the quiet quality label.
fn header(app: &App, height: f32, with_timing: bool) -> Element<'_, Message> {
    let mut r = row![close_button(), Space::new().width(Length::Fill)]
        .spacing(gap::SM)
        .align_y(Alignment::Center);
    if with_timing {
        r = r.push(controls::timing_button(app));
    }
    r.push(controls::quality_label(app, 300.0))
        .padding(Padding::from([0.0, gap::XL]))
        .height(Length::Fixed(height))
        .into()
}

/// `page` with the Timing popover over it while it is open.
fn with_timing<'a>(app: &'a App, page: Element<'a, Message>, top: f32) -> Element<'a, Message> {
    match controls::timing_layer(app, top) {
        Some(layer) => stack![page, layer].into(),
        None => page,
    }
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
        lyr::display_title(&title, ts, width, col(color::on_surface())),
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

    let left = column![
        grow(
            app,
            super::pane::breathing(app, cover_art(app, art, shape::xl()))
        ),
        Space::new().height(gap::XL),
        titles(app, art, false),
        Space::new().height(gap::LG),
        controls::seek(app),
        Space::new().height(gap::MD),
        container(controls::transport(app, 56.0)).center_x(Length::Fill),
    ]
    .width(Length::Fixed(art));

    let right = lyrics_panel(app, lyrics_w, lyrics_viewport_height(w, h));
    // The art keeps the left, the lyrics the right 58 to 66% of the window: the room that is
    // left over sits between them, never at the window edges.
    let body = row![left, Space::new().width(Length::Fill), right]
        .width(Length::Fill)
        .align_y(Alignment::Center);
    let page = column![
        header(app, 56.0, true),
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(Padding::from([gap::XL, gap::XL])),
    ];
    with_timing(app, page.into(), 56.0)
}

fn compact(app: &App, w: u32, h: u32) -> Element<'_, Message> {
    let art = layout::now_playing_art(w, h);
    // Pixel sizes are small, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let (art, width) = (art as f32, (w as f32 - 2.0 * gap::XL).min(480.0));

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
        grow(
            app,
            super::pane::breathing(app, cover_art(app, art, shape::xl()))
        ),
        Space::new().height(gap::MD),
        titles(app, width, true),
        Space::new().height(gap::MD),
        container(controls::seek(app)).width(Length::Fixed(width)),
        Space::new().height(gap::SM),
        controls::transport(app, 56.0),
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
    let page = column![header(app, 48.0, true), body];
    with_timing(app, page.into(), 48.0)
}

/// Three lyric lines (previous, current, next) for the compact layout.
fn compact_lyrics(app: &App, width: f32) -> Element<'_, Message> {
    let LyricsState::Ready(doc) = &app.ui.lyrics else {
        return lyr::lyrics_status(app, width);
    };
    if doc.instrumental {
        return lyr::status_text("Instrumental", None);
    }
    column![
        lyr::three_lines(app, doc, width),
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
        container(lyr::body(app, width))
            .width(Length::Fill)
            .height(Length::Fixed(viewport_h - lyr::footer_extra(app, width)))
            .clip(true),
        lyr::footer(app, width),
    ]
    .width(Length::Fixed(width))
    .into()
}

mod controls;

#[cfg(test)]
mod tests;
