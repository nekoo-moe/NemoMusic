//! Now Playing: art and lyrics side by side, or the compact column (`ui-reference.md`
//! section 4). The floating bar is hidden here; the transport lives under the art. No glass sits
//! behind lyric text: the backdrop is a blurred cover under a scrim (`design.md` sections 9, 10).

use iced::widget::{Space, button, column, container, image, mouse_area, row, scrollable, stack};
use iced::{Alignment, Color, Element, Length, Padding, mouse};

use super::super::app::{App, CompactTab, Message};
use super::super::assets::Icon;
use super::super::style::{animated, col, fit, gap, icon_button, label, label_w};
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
                    let scrim = col(color::surface().with_alpha(c.scrim_alpha));
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
        c = c.push(lyric_line(app, &line.text, width, st, role));
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

/// One lyric line. The active line eases from the dim ink to the bright ink and gets a soft
/// accent glow (four faint offset copies behind it) that fades in with the line change.
pub fn lyric_line<'a>(
    app: &App,
    text: &str,
    width: f32,
    st: crate::theme::TextStyle,
    role: LineRole,
) -> Element<'a, Message> {
    let shown = fit(text, width, st.size);
    if role != LineRole::Active {
        return label(shown, st, role.semibold(), lyric_ink(role)).into();
    }
    let p = app.anim.line().clamp(0.0, 1.0);
    let dim = lyric_ink(LineRole::Other);
    let bright = lyric_ink(LineRole::Active);
    let mix = |a: f32, b: f32| a + (b - a) * p;
    let ink = Color {
        r: mix(dim.r, bright.r),
        g: mix(dim.g, bright.g),
        b: mix(dim.b, bright.b),
        a: mix(dim.a, bright.a),
    };
    let glow = col(app.accent().primary.with_alpha(0.22 * p));
    let mut layers: Vec<Element<'a, Message>> = Vec::new();
    if p > 0.02 {
        for (dx, dy) in [(0.0, 0.0), (3.0, 0.0), (0.0, 3.0), (3.0, 3.0)] {
            layers.push(
                container(label(shown.clone(), st, true, glow))
                    .padding(Padding {
                        left: dx,
                        top: dy,
                        ..Padding::ZERO
                    })
                    .into(),
            );
        }
    }
    layers.push(
        container(label(shown, st, true, ink))
            .padding(Padding {
                left: 1.5,
                top: 1.5,
                ..Padding::ZERO
            })
            .into(),
    );
    stack(layers).width(Length::Fill).into()
}

/// Ink of a lyric line: bright when active, dimmed otherwise.
pub fn lyric_ink(role: LineRole) -> Color {
    match role {
        LineRole::Active => col(color::on_surface()),
        _ => col(color::on_surface_variant().with_alpha(crate::cover::LYRIC_DIM_ALPHA)),
    }
}

/// A centred status message in the lyrics area.
pub fn status_text(title: &str, detail: Option<String>) -> Element<'_, Message> {
    let mut c = column![label(
        title,
        type_scale::TITLE_MD,
        true,
        col(color::on_surface())
    )]
    .spacing(gap::XS);
    if let Some(d) = detail {
        c = c.push(label(
            d,
            type_scale::BODY_SM,
            false,
            col(color::on_surface_variant()),
        ));
    }
    container(c)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

/// Loading and missing states: one calm block, centred. A missing song gets an icon, the plain
/// sentence and the providers that were asked; nothing else is drawn under it.
pub fn lyrics_status(app: &App, width: f32) -> Element<'_, Message> {
    let dim = col(color::on_surface_variant());
    let (glyph, title, detail) = match &app.ui.lyrics {
        LyricsState::Missing(tried) => {
            let names: Vec<&str> = tried
                .iter()
                .map(|p| p.as_str())
                .filter(|n| !n.is_empty())
                .collect();
            (
                Icon::MusicOff,
                "No lyrics for this song",
                (!names.is_empty()).then(|| format!("Looked at {}", names.join(", "))),
            )
        }
        LyricsState::Loading => (Icon::Lyrics, "Looking for lyrics\u{2026}", None),
        _ => (Icon::MusicOff, "No lyrics for this song", None),
    };
    let mut c = column![
        super::super::style::icon(glyph, 32.0, dim),
        Space::new().height(gap::MD),
        label(title, type_scale::TITLE_SM, true, col(color::on_surface())),
    ]
    .align_x(Alignment::Center);
    if let Some(d) = detail {
        c = c.push(Space::new().height(gap::XS)).push(
            label(d, type_scale::BODY_SM, false, dim)
                .wrapping(iced::widget::text::Wrapping::Word)
                .width(Length::Fixed((width - 2.0 * gap::LG).max(120.0)))
                .align_x(iced::alignment::Horizontal::Center),
        );
    }
    container(c)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

/// Extra height the lyrics footer takes for the provider switcher, px.
pub fn footer_extra(app: &App) -> f32 {
    if !matches!(app.ui.lyrics, LyricsState::Ready(_)) {
        return 0.0;
    }
    let switcher = if app.lyric_candidates.len() > 1 {
        38.0
    } else {
        0.0
    };
    let credit = if app.lyric_credit.is_some() {
        18.0
    } else {
        0.0
    };
    switcher + credit
}

/// The credit line a source asks for, for example "Lyrics from Unison \u{b7} by Someone".
fn credit_text(app: &App) -> Option<String> {
    let c = app.lyric_credit.as_ref()?;
    Some(match &c.by {
        Some(by) => format!("{} \u{b7} by {by}", c.label),
        None => c.label.clone(),
    })
}

/// Provider switcher, provider and sync state, and the credit; never hidden (`design.md`
/// section 10).
pub fn footer(app: &App, width: f32) -> Element<'_, Message> {
    // Without lyrics there is nothing to credit; the empty state names the providers instead.
    let LyricsState::Ready(doc) = &app.ui.lyrics else {
        return Space::new().height(gap::SM).into();
    };
    let dim = col(color::on_surface_variant());
    let text = lyrics_view::footer(Some(doc));
    let mut c = column![].width(Length::Fill);
    if app.lyric_candidates.len() > 1 {
        c = c.push(
            container(super::panels::lyrics_switcher(
                app,
                Message::PickLyricsProvider,
            ))
            .height(Length::Fixed(38.0))
            .align_y(Alignment::Center)
            .clip(true),
        );
    }
    if let Some(credit) = credit_text(app) {
        c = c.push(
            container(label(
                fit(&credit, width, type_scale::LABEL_SM.size),
                type_scale::LABEL_SM,
                false,
                col(color::on_surface()),
            ))
            .height(Length::Fixed(18.0)),
        );
    }
    c.push(
        container(label(
            fit(&text, width, type_scale::LABEL_SM.size),
            type_scale::LABEL_SM,
            false,
            dim,
        ))
        .height(Length::Fixed(24.0))
        .align_y(Alignment::End),
    )
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
                        col(color::on_surface_variant()),
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
                    container(lyric_line(app, &line.text, width, st, role))
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
            .height(Length::Fixed(viewport_h - footer_extra(app))),
        footer(app, width),
    ]
    .width(Length::Fixed(width))
    .into()
}
