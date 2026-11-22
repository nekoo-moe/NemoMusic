//! Lyric lines, shared by the right pane, the full-screen Now Playing and its compact layout
//! (`design.md` section 10, `lyrics-rebuild` spec). One [`LyricList`] draws every line in one
//! size and weight; lit lines are bright and full size, the others dim and a little smaller. Row
//! counts are measured with the real font advances, and the reducer anchors the scroll on the
//! same numbers.

use std::borrow::Cow;

use iced::widget::text::Wrapping;
use iced::widget::{Space, column, container, mouse_area, stack, text};
use iced::{Alignment, Color, Element, Length, Padding, Pixels, mouse};
use nemo_core::lyrics::{Granularity, LyricsDoc};

use super::super::app::{App, Message};
use super::super::assets::{Icon, display_font};
use super::super::lyric_list::LyricList;
use super::super::shimmer::Shimmer;
use super::super::style::{col, gap, icon, label};
use super::super::textfit;
use crate::lyrics_view::enter;
use crate::lyrics_view::layout::Rows;
use crate::lyrics_view::metrics::Metrics;
use crate::state::{Event, LyricsState};
use crate::theme::{TextStyle, color, type_scale};

mod footer;
mod note;
mod three;

pub use footer::{footer, footer_extra};
pub use three::lines as three_lines;

/// Room kept at the right of a lyric line for padding, px.
pub const GLOW_INSET: f32 = 4.0;

/// Width a lyric line wraps at in a column `width` px wide.
#[must_use]
pub fn wrap_width(width: f32) -> f32 {
    (width - GLOW_INSET).max(1.0)
}

/// Rows `text` wraps onto at `size` px in a column `width` px wide.
#[must_use]
pub fn rows_at(text: &str, width: f32, size: f32) -> Rows {
    Rows::try_from(textfit::rows_in(
        textfit::Face::Display,
        text,
        wrap_width(width),
        size,
    ))
    .unwrap_or(Rows::MAX)
}

/// One line of display text (`font.display` 600, lyrics-polish task 3): `s` cut with an
/// ellipsis to `width` px, measured with the display face.
pub fn display_title<'a>(s: &str, st: TextStyle, width: f32, ink: Color) -> text::Text<'a> {
    let size = f32::from(st.size);
    text(textfit::fit_in(textfit::Face::Display, s, width, size))
        .size(Pixels(size))
        .line_height(text::LineHeight::Absolute(Pixels(f32::from(st.line))))
        .font(display_font(true))
        .color(ink)
        .wrapping(Wrapping::None)
}

/// Wrapped row counts of every line of `doc` at `size` px, for the reducer's scroll anchor.
#[must_use]
pub fn line_rows(doc: &LyricsDoc, width: f32, size: f32) -> Vec<Rows> {
    doc.lines
        .iter()
        .map(|l| rows_at(&l.text, width, size))
        .collect()
}

/// The list of synced lines, with the intro or gap note and the `Back to current line` pill
/// while the user scrolled away.
pub fn synced_list<'a>(app: &'a App, doc: &'a LyricsDoc, width: f32) -> Element<'a, Message> {
    let focus = app.ui.lyric_focus();
    let note = focus.note.filter(|n| n.presence > 0.0);
    let m = app.ui.lyric_metrics;
    let scroll = app.ui.lyric_scroll.value;
    let Some(clocks) = app.ui.lyric_clocks() else {
        return Space::new().into();
    };
    let list = LyricList {
        doc,
        rev: app.ui.lyrics_rev,
        first: 0,
        tops: Cow::Borrowed(&app.ui.lyric_tops),
        metrics: m,
        wrap_w: wrap_width(width),
        scroll,
        timeline: &app.ui.lyric_timeline,
        clocks,
        note,
        ink: col(color::on_surface()),
        font: display_font(true),
        reduced: app.ui.reduced_motion,
        fade_edges: true,
        cascade: Some(&app.ui.lyric_scroll),
        enter: enter::enter_progress(app.ui.lyric_enter_ms),
    };
    let list = mouse_area(Element::from(list)).on_scroll(|delta| {
        let px = match delta {
            mouse::ScrollDelta::Lines { y, .. } => -y * 48.0,
            mouse::ScrollDelta::Pixels { y, .. } => -y,
        };
        Message::Ui(Event::LyricsScrolled(px))
    });
    let mut layers: Vec<Element<'a, Message>> = vec![list.into()];
    if let Some(n) = note {
        let (el, _) = note::row(n, &m, m.gap);
        let top = (app.ui.lyric_tops.get(n.before).copied().unwrap_or(0.0) - scroll
            + app.ui.lyric_scroll.offset(n.before))
        .max(0.0);
        layers.push(
            container(el)
                .padding(Padding {
                    top,
                    ..Padding::ZERO
                })
                .width(Length::Fill)
                .height(Length::Fill)
                .clip(true)
                .into(),
        );
    }
    if !app.ui.lyrics_following() {
        layers.push(
            container(super::now_playing::pill_button(
                crate::i18n::t(
                    app.ui.settings.language,
                    "Back to current line",
                    "Trở lại dòng hiện tại",
                ),
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

/// Unsynced lyrics: static wrapped text the user scrolls (`design.md` section 10).
pub fn unsynced(doc: &LyricsDoc, st: TextStyle) -> Element<'_, Message> {
    let mut c = column![].spacing(gap::SM).width(Length::Fill);
    for line in &doc.lines {
        c = c.push(
            text(line.text.clone())
                .size(Pixels(f32::from(st.size)))
                .line_height(text::LineHeight::Absolute(Pixels(f32::from(st.line))))
                .font(display_font(true))
                .color(col(color::on_surface_variant()))
                .wrapping(Wrapping::Word),
        );
    }
    super::super::style::scroll(c).height(Length::Fill).into()
}

/// The lyrics body for the right pane or the wide full-screen layout.
pub fn body(app: &App, width: f32) -> Element<'_, Message> {
    match &app.ui.lyrics {
        LyricsState::Ready(doc) if doc.instrumental => status_text(
            crate::i18n::t(app.ui.settings.language, "Instrumental", "Không lời"),
            None,
        ),
        LyricsState::Ready(doc) if doc.granularity == Granularity::Unsynced => {
            unsynced(doc, unsynced_style(app.ui.lyric_metrics))
        }
        LyricsState::Ready(doc) => synced_list(app, doc, width),
        _ => lyrics_status(app, width),
    }
}

/// Text style of unsynced lyrics: the list size and line height of the place they are shown.
fn unsynced_style(m: Metrics) -> TextStyle {
    // Sizes are small whole numbers, exact in u16.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    TextStyle {
        size: m.size as u16,
        line: m.line_h as u16,
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
        c = c.push(
            label(
                d,
                type_scale::BODY_SM,
                false,
                col(color::on_surface_variant()),
            )
            .wrapping(Wrapping::Word),
        );
    }
    container(c)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

/// The loading state: one calm shimmering line where the first lyric line will be, then the
/// list slides in. Without motion the line is still.
fn loading_line(app: &App) -> Element<'_, Message> {
    let m = app.ui.lyric_metrics;
    let top = (crate::lyrics_view::ANCHOR * app.ui.lyric_viewport_h - m.gap).max(0.0);
    let line = Shimmer {
        text: crate::i18n::t(
            app.ui.settings.language,
            "Looking for lyrics\u{2026}",
            "Đang tìm lời bài hát\u{2026}",
        )
        .to_owned(),
        metrics: m,
        phase: enter::shimmer_phase(app.ui.lyric_loading_ms),
        ink: col(color::on_surface()),
        font: display_font(true),
        animate: !app.ui.reduced_motion,
    };
    container(Element::from(line))
        .padding(Padding {
            top,
            ..Padding::ZERO
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Loading and missing states: one calm block, centred. A missing song gets an icon, the plain
/// sentence and the providers that were asked; nothing else is drawn under it.
pub fn lyrics_status(app: &App, width: f32) -> Element<'_, Message> {
    let dim = col(color::on_surface_variant());
    let lang = app.ui.settings.language;
    let (glyph, title, detail) = match &app.ui.lyrics {
        LyricsState::Missing(tried) => {
            let names: Vec<&str> = tried
                .iter()
                .map(|p| p.as_str())
                .filter(|n| !n.is_empty())
                .collect();
            (
                Icon::MusicOff,
                crate::i18n::t(
                    lang,
                    "No lyrics for this song",
                    "Không có lời bài hát cho bản nhạc này",
                ),
                (!names.is_empty()).then(|| match lang {
                    crate::settings::Language::Vietnamese => {
                        format!("Đã tìm tại {}", names.join(", "))
                    }
                    crate::settings::Language::English => format!("Looked at {}", names.join(", ")),
                }),
            )
        }
        LyricsState::Loading => return loading_line(app),
        _ => (
            Icon::MusicOff,
            crate::i18n::t(
                lang,
                "No lyrics for this song",
                "Không có lời bài hát cho bản nhạc này",
            ),
            None,
        ),
    };
    let mut c = column![
        icon(glyph, 32.0, dim),
        Space::new().height(gap::MD),
        label(title, type_scale::TITLE_SM, true, col(color::on_surface())),
    ]
    .align_x(Alignment::Center);
    if let Some(d) = detail {
        c = c.push(Space::new().height(gap::XS)).push(
            label(d, type_scale::BODY_SM, false, dim)
                .wrapping(Wrapping::Word)
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

#[cfg(test)]
mod tests;
