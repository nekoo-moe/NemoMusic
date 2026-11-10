//! Now Playing: art and lyrics side by side, or the compact column (`ui-reference.md`
//! section 4). The floating bar is hidden here; the transport lives under the art. No glass sits
//! behind lyric text: the backdrop is a blurred cover under a scrim (`design.md` sections 9, 10).

use iced::widget::{Space, button, column, container, image, mouse_area, row, scrollable, stack};
use iced::{Alignment, Color, Element, Length, Padding, mouse};

use super::super::app::{App, CompactTab, Message};
use super::super::assets::Icon;
use super::super::style::{animated, button_style, col, fit, gap, icon_button, label, label_w};
use super::super::widgets::Offset;
use super::bar::{badge, cover_art, seek_row, transport};
use crate::format;
use crate::keys::{KeyAction, LYRICS_OFFSET_STEP_MS};
use crate::layout;
use crate::lyrics_view::{self, LINE_GAP, LineRole};
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

/// The full-window Now Playing screen.
pub fn view(app: &App, w: u32, h: u32) -> Element<'_, Message> {
    let page = if layout::is_compact(w, h) {
        compact(app, w, h)
    } else {
        wide(app, w, h)
    };
    stack![backdrop(app), page].into()
}

/// Blurred cover under a scrim; plain `surface` when there is no cover.
fn backdrop(app: &App) -> Element<'_, Message> {
    match &app.cover {
        Some(c) => stack![
            image(c.backdrop.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(iced::ContentFit::Cover),
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style({
                    let scrim = col(color::SURFACE.with_alpha(c.scrim_alpha));
                    move |_| container::Style {
                        background: Some(scrim.into()),
                        ..container::Style::default()
                    }
                }),
        ]
        .into(),
        None => Space::new().into(),
    }
}

fn close_button<'a>() -> Element<'a, Message> {
    icon_button(
        Icon::ChevronDown,
        24.0,
        40.0,
        col(color::ON_SURFACE),
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
                col(color::ON_SURFACE),
            ))
            .padding(Padding::from([gap::SM, gap::LG])),
        )
        .padding(0)
        .on_press(message),
        col(color::ON_SURFACE.with_alpha(0.10)),
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
            col(color::ON_SURFACE),
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
            col(color::ON_SURFACE)
        ),
        label(
            fit(&artist, width, as_.size),
            as_,
            false,
            col(color::ON_SURFACE_VARIANT)
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
        cover_art(app, art, shape::XL),
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
        button(
            container(label(
                text,
                type_scale::BODY_MD,
                on,
                col(if on {
                    color::ON_SURFACE
                } else {
                    color::ON_SURFACE_VARIANT
                }),
            ))
            .width(Length::Fill)
            .height(Length::Fixed(40.0))
            .center_x(Length::Fill)
            .center_y(Length::Fixed(40.0)),
        )
        .padding(0)
        .width(Length::Fill)
        .style(button_style(
            if on {
                col(color::SURFACE_CONTAINER_HIGH)
            } else {
                Color::TRANSPARENT
            },
            shape::FULL,
        ))
        .on_press(Message::CompactTab(which))
    };
    let lower: Element<'_, Message> = match app.compact_tab {
        CompactTab::Lyrics => compact_lyrics(app, width),
        CompactTab::Queue => compact_queue(app, width),
    };
    let body = column![
        cover_art(app, art, shape::XL),
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
        return lyrics_status(app, width);
    };
    if doc.instrumental {
        return status_text("Instrumental", None);
    }
    let active = app.ui.active_lyric();
    let center = active.unwrap_or(0);
    let first = center
        .saturating_sub(1)
        .min(doc.lines.len().saturating_sub(3));
    let mut c = column![].align_x(Alignment::Center).width(Length::Fill);
    for (i, line) in doc.lines.iter().enumerate().skip(first).take(3) {
        let role = if active.is_some() {
            LineRole::of(i, active)
        } else {
            LineRole::Other
        };
        // One step smaller than the wide layout so three lines fit above the footer.
        let st = match role {
            LineRole::Active => type_scale::TITLE_MD,
            _ => type_scale::BODY_MD,
        };
        c = c.push(label(
            fit(&line.text, width, st.size),
            st,
            role.semibold(),
            lyric_ink(role),
        ));
    }
    column![
        c,
        Space::new().height(Length::Fill),
        footer(app, width),
        Space::new().height(gap::SM)
    ]
    .into()
}

fn compact_queue(app: &App, width: f32) -> Element<'_, Message> {
    let snap = &app.ui.snapshot;
    let mut rows = column![];
    for (i, t) in snap.queue.iter().enumerate() {
        let current = snap.queue_index == Some(i);
        rows = rows.push(
            button(
                container(
                    row![
                        label(
                            fit(&t.title, width * 0.55, type_scale::BODY_MD.size),
                            type_scale::BODY_MD,
                            current,
                            col(color::ON_SURFACE),
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
                            col(color::ON_SURFACE_VARIANT),
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
            .style(button_style(Color::TRANSPARENT, shape::MD))
            .on_press(Message::Ui(Event::Command(PlayerCommand::SetQueue {
                tracks: snap.queue.clone(),
                start: i,
            }))),
        );
    }
    scrollable(rows).height(Length::Fill).into()
}

/// Ink of a lyric line: bright when active, dimmed otherwise.
pub fn lyric_ink(role: LineRole) -> Color {
    match role {
        LineRole::Active => col(color::ON_SURFACE),
        _ => col(color::ON_SURFACE_VARIANT.with_alpha(crate::cover::LYRIC_DIM_ALPHA)),
    }
}

/// A centred status message in the lyrics area.
pub fn status_text(title: &str, detail: Option<String>) -> Element<'_, Message> {
    let mut c = column![label(
        title,
        type_scale::TITLE_MD,
        true,
        col(color::ON_SURFACE)
    )]
    .spacing(gap::XS);
    if let Some(d) = detail {
        c = c.push(label(
            d,
            type_scale::BODY_SM,
            false,
            col(color::ON_SURFACE_VARIANT),
        ));
    }
    container(c)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

/// Loading and missing states.
pub fn lyrics_status(app: &App, width: f32) -> Element<'_, Message> {
    let _ = width;
    match &app.ui.lyrics {
        LyricsState::Missing(tried) => {
            let names: Vec<&str> = tried.iter().map(|p| p.as_str()).collect();
            status_text(
                "No lyrics found",
                Some(format!("Tried: {}", names.join(", "))),
            )
        }
        LyricsState::Loading => status_text("Looking for lyrics\u{2026}", None),
        _ => status_text("No lyrics", None),
    }
}

/// Provider and sync state; never hidden (`design.md` section 10).
pub fn footer(app: &App, width: f32) -> Element<'_, Message> {
    let doc = match &app.ui.lyrics {
        LyricsState::Ready(d) => Some(d),
        _ => None,
    };
    container(label(
        fit(&lyrics_view::footer(doc), width, type_scale::LABEL_SM.size),
        type_scale::LABEL_SM,
        false,
        col(color::ON_SURFACE_VARIANT),
    ))
    .height(Length::Fixed(24.0))
    .align_y(Alignment::End)
    .into()
}

/// The lyrics column of the wide layout.
fn lyrics_panel(app: &App, width: f32, viewport_h: f32) -> Element<'_, Message> {
    let body: Element<'_, Message> = match &app.ui.lyrics {
        LyricsState::Ready(doc) if doc.instrumental => status_text("Instrumental", None),
        LyricsState::Ready(doc) if doc.granularity == Granularity::Unsynced => {
            let mut c = column![].spacing(gap::SM).width(Length::Fill);
            for line in &doc.lines {
                c = c.push(
                    label(
                        line.text.clone(),
                        type_scale::TITLE_SM,
                        false,
                        col(color::ON_SURFACE_VARIANT),
                    )
                    .wrapping(iced::widget::text::Wrapping::Word),
                );
            }
            scrollable(c).height(Length::Fill).into()
        }
        LyricsState::Ready(doc) => {
            let active = app.ui.active_lyric();
            let mut c = column![].width(Length::Fill);
            for (i, line) in doc.lines.iter().enumerate() {
                let role = LineRole::of(i, active);
                let st = role.style();
                c = c.push(
                    container(label(
                        fit(&line.text, width, st.size),
                        st,
                        role.semibold(),
                        lyric_ink(role),
                    ))
                    .height(Length::Fixed(f32::from(st.line) + LINE_GAP))
                    .width(Length::Fill),
                );
            }
            let list = mouse_area(Offset::new(c, app.ui.lyric_scroll.value)).on_scroll(|delta| {
                let px = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => -y * 48.0,
                    mouse::ScrollDelta::Pixels { y, .. } => -y,
                };
                Message::Ui(Event::LyricsScrolled(px))
            });
            let mut layers: Vec<Element<'_, Message>> = vec![list.into()];
            if !app.ui.lyrics_following() {
                layers.push(
                    container(pill_button(
                        "Back to current line",
                        Message::Ui(Event::LyricsFollow),
                    ))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_y(Alignment::End)
                    .center_x(Length::Fill)
                    .padding(gap::MD)
                    .into(),
                );
            }
            stack(layers).into()
        }
        _ => lyrics_status(app, width),
    };
    column![
        container(body)
            .width(Length::Fill)
            .height(Length::Fixed(viewport_h)),
        footer(app, width),
    ]
    .width(Length::Fixed(width))
    .into()
}
