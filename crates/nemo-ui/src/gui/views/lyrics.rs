//! Lyric lines, shared by the right pane, the full-screen Now Playing and its compact layout
//! (`design.md` section 10, wave-lyrics criteria 6 to 9). Lines wrap and are never cut; each
//! row is as tall as its wrapped text, measured with the real font advances, and the reducer
//! anchors the scroll on the same numbers. When the active line changes, the old and the new
//! active lines cross-fade size and ink while the list glides on the `spatial.default` spring.
//! Word- and syllable-timed lines colour every piece by its own timing.

use iced::widget::text::Wrapping;
use iced::widget::{Space, column, container, mouse_area, stack, text};
use iced::{Alignment, Color, Element, Length, Pixels, mouse};
use nemo_core::lyrics::{Granularity, LyricLine, LyricsDoc};

use super::super::app::{App, Message};
use super::super::assets::{Icon, display_font};
use super::super::style::{col, gap, icon, label};
use super::super::syllable_text::{Style as SyllableStyle, SyllableText};
use super::super::textfit;
use super::super::widgets::Offset;
use crate::lyric_pieces::{self, UNSUNG_ALPHA};
use crate::lyrics_view::{self, LINE_GAP, LineRole, LineRows};
use crate::state::{Event, LyricsState};
use crate::theme::{TextStyle, color, type_scale};

mod footer;
mod note;
mod three;

pub use footer::{credit_text, footer, footer_extra};
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
pub fn rows_at(text: &str, width: f32, size: f32) -> u8 {
    u8::try_from(textfit::rows_in(
        textfit::Face::Display,
        text,
        wrap_width(width),
        size,
    ))
    .unwrap_or(u8::MAX)
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

/// Wrapped row counts of every line of `doc` for each role, for the reducer's scroll anchor.
#[must_use]
pub fn line_rows(doc: &LyricsDoc, width: f32, dense: bool) -> Vec<LineRows> {
    let roles = [LineRole::Active, LineRole::Neighbor, LineRole::Other];
    doc.lines
        .iter()
        .map(|l| roles.map(|r| rows_at(&l.text, width, f32::from(r.style_d(dense).size))))
        .collect()
}

/// Ink of a lyric line in a role: pure white (`Color::WHITE`) when active, clean neutral grey
/// (`on-surface-variant`) for inactive/upcoming lines.
#[must_use]
pub fn line_ink(role: LineRole) -> Color {
    match role {
        LineRole::Active => Color::WHITE,
        LineRole::Neighbor => col(crate::theme::color::on_surface_variant().with_alpha(0.50)),
        LineRole::Other => col(crate::theme::color::on_surface_variant().with_alpha(0.35)),
    }
}

/// Ink of a line of word-timed lyrics `dist` lines from the active one: the active line is
/// `on-surface`, every other line `on-surface-variant` at [`lyrics_view::line_alpha`], always
/// dimmer than a word of the active line that is still to be sung.
fn timed_line_ink(dist: i32) -> Color {
    if dist == 0 {
        return col(color::on_surface());
    }
    col(color::on_surface_variant().with_alpha(lyrics_view::line_alpha(dist)))
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn mix_color(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: mix(a.r, b.r, t),
        g: mix(a.g, b.g, t),
        b: mix(a.b, b.b, t),
        a: mix(a.a, b.a, t),
    }
}

/// How one line looks: its role and type style before the last line change and now.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    /// Role and style before the change.
    pub from: (LineRole, TextStyle),
    /// Role and style now.
    pub to: (LineRole, TextStyle),
    /// Space under the line, px.
    pub gap: f32,
    /// Opacity from the soft edge mask of the full-screen lyrics (1 elsewhere).
    pub fade: f32,
    /// Signed distance from the active line before the change and now (positive: still to
    /// come, negative: already sung), for the opacity falloff of word-timed lyrics.
    pub dist: (i32, i32),
}

/// Distance a role stands for when the caller does not know the real one.
fn role_dist(role: LineRole) -> i32 {
    match role {
        LineRole::Active => 0,
        LineRole::Neighbor => 1,
        LineRole::Other => 3,
    }
}

impl Look {
    /// A line that did not change, with the dense line gap and no edge fade.
    #[must_use]
    pub fn settled(role: LineRole, st: TextStyle) -> Self {
        Self {
            from: (role, st),
            to: (role, st),
            gap: LINE_GAP,
            fade: 1.0,
            dist: (role_dist(role), role_dist(role)),
        }
    }
}

/// `c` with its alpha scaled by `k`.
fn faded(c: Color, k: f32) -> Color {
    Color { a: c.a * k, ..c }
}

fn text_run<'a>(content: String, size: f32, line_h: f32, bold: bool, ink: Color) -> text::Text<'a> {
    text(content)
        .size(Pixels(size))
        .line_height(text::LineHeight::Absolute(Pixels(line_h)))
        .font(display_font(bold))
        .color(ink)
        .wrapping(Wrapping::WordOrGlyph)
}

/// Size, row height and full height (wrapped rows plus the gap) of a line, px, mid-change.
fn metrics(app: &App, line: &LyricLine, width: f32, look: Look) -> (f32, f32, f32) {
    let p = app.anim.line().clamp(0.0, 1.0);
    let (from_st, st) = (look.from.1, look.to.1);
    let size = mix(f32::from(from_st.size), f32::from(st.size), p);
    let line_h = mix(f32::from(from_st.line), f32::from(st.line), p);
    let rows = rows_at(&line.text, width, size);
    (size, line_h, f32::from(rows) * line_h + look.gap)
}

/// One lyric line `width` px wide and its height in px (wrapped rows plus the line gap).
pub fn lyric_line<'a>(
    app: &App,
    line: &LyricLine,
    width: f32,
    look: Look,
) -> (Element<'a, Message>, f32) {
    let p = app.anim.line().clamp(0.0, 1.0);
    let (from_role, _) = look.from;
    let (role, _) = look.to;
    let (size, line_h, height) = metrics(app, line, width, look);
    let wrap = wrap_width(width);
    let doc_timed =
        matches!(&app.ui.lyrics, LyricsState::Ready(d) if d.granularity >= Granularity::Word);
    let ink = if doc_timed {
        mix_color(timed_line_ink(look.dist.0), timed_line_ink(look.dist.1), p)
    } else {
        mix_color(line_ink(from_role), line_ink(role), p)
    };
    let ink = faded(ink, look.fade);
    let bold = false;
    if role != LineRole::Active {
        let body = text_run(line.text.clone(), size, line_h, bold, ink).width(Length::Fixed(wrap));
        return (body.into(), height);
    }
    let timed = !line.words.is_empty()
        && matches!(&app.ui.lyrics, LyricsState::Ready(d) if d.granularity >= Granularity::Word);
    if timed {
        return (
            pieces_text(app, line, (size, line_h), wrap, look.fade),
            height,
        );
    }
    let body = text_run(line.text.clone(), size, line_h, false, ink).width(Length::Fixed(wrap));
    (body.into(), height)
}

/// The active line, one piece per timed word or syllable: grey ahead, a fill while sung, bright
/// after (`syllable-motion` spec).
fn pieces_text<'a>(
    app: &App,
    line: &LyricLine,
    (size, line_h): (f32, f32),
    wrap: f32,
    fade: f32,
) -> Element<'a, Message> {
    let t = i64::try_from(app.ui.position.as_millis())
        .unwrap_or(i64::MAX)
        .saturating_add(app.ui.lyrics_offset_ms);
    SyllableText::new(
        lyric_pieces::texts(line),
        lyric_pieces::timings(line),
        t,
        SyllableStyle {
            size,
            line_h,
            width: wrap,
            font: display_font(true),
            bright: faded(col(color::on_surface()), fade),
            grey: faded(
                col(color::on_surface_variant().with_alpha(UNSUNG_ALPHA)),
                fade,
            ),
            glow: faded(col(app.accent().primary), fade),
            rtl: lyric_pieces::is_rtl(&line.text),
            reduced: app.ui.reduced_motion,
        },
    )
    .into()
}

/// Signed distance of line `i` from the active line (`None`: nothing active, so far ahead).
fn signed_dist(i: usize, active: Option<usize>) -> i32 {
    active.map_or(3, |a| {
        i32::try_from(i).unwrap_or(i32::MAX) - i32::try_from(a).unwrap_or(0)
    })
}

/// The synced lines as a gliding list, with the `Back to current line` pill while the user
/// scrolled away. `dense` picks the right pane sizes.
pub fn synced_list<'a>(
    app: &'a App,
    doc: &'a LyricsDoc,
    width: f32,
    dense: bool,
) -> Element<'a, Message> {
    let focus = app.ui.lyric_focus();
    let active = focus.active;
    let before = if app.anim.line() < 1.0 {
        app.prev_line
    } else {
        active
    };
    let note = focus.note.filter(|n| n.presence > 0.0);
    let gap_px = lyrics_view::line_gap(dense);
    // The full-screen lines fade toward the top and bottom edges (soft mask); `top` is the
    // line's top in the viewport, after the scroll.
    let (scroll, vh) = (app.ui.lyric_scroll.value, app.ui.lyric_viewport_h);
    let mut top = -scroll;
    let mut c = column![].width(Length::Fill);
    for (i, line) in doc.lines.iter().enumerate() {
        if let Some(n) = note.filter(|n| n.before == i) {
            let (el, h) = note::row_for(n, LineRole::Active.style_d(dense), gap_px);
            c = c.push(container(el).height(Length::Fixed(h)).width(Length::Fill));
            top += h;
        }
        let (from, to) = (LineRole::of(i, before), LineRole::of(i, active));
        let mut look = Look {
            from: (from, from.style_d(dense)),
            to: (to, to.style_d(dense)),
            gap: gap_px,
            fade: 1.0,
            dist: (signed_dist(i, before), signed_dist(i, active)),
        };
        let h = metrics(app, line, width, look).2;
        if !dense {
            look.fade = lyrics_view::edge_fade(top + (h - gap_px) / 2.0, vh);
        }
        top += h;
        // A line fully under the mask is only its height.
        let el: Element<'a, Message> = if look.fade <= 0.01 {
            Space::new().into()
        } else {
            lyric_line(app, line, width, look).0
        };
        c = c.push(container(el).height(Length::Fixed(h)).width(Length::Fill));
    }
    let list = mouse_area(Offset::new(c, app.ui.lyric_scroll.value)).on_scroll(|delta| {
        let px = match delta {
            mouse::ScrollDelta::Lines { y, .. } => -y * 48.0,
            mouse::ScrollDelta::Pixels { y, .. } => -y,
        };
        Message::Ui(Event::LyricsScrolled(px))
    });
    let mut layers: Vec<Element<'a, Message>> = vec![list.into()];
    if !app.ui.lyrics_following() {
        let pad = if dense { gap::SM } else { gap::MD };
        layers.push(
            container(super::now_playing::pill_button(
                "Back to current line",
                Message::Ui(Event::LyricsFollow),
            ))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::End)
            .center_x(Length::Fill)
            .padding(pad)
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
                .font(display_font(false))
                .color(col(color::on_surface_variant()))
                .wrapping(Wrapping::Word),
        );
    }
    super::super::style::scroll(c).height(Length::Fill).into()
}

/// The lyrics body for the right pane or the wide full-screen layout.
pub fn body(app: &App, width: f32, dense: bool) -> Element<'_, Message> {
    match &app.ui.lyrics {
        LyricsState::Ready(doc) if doc.instrumental => status_text("Instrumental", None),
        LyricsState::Ready(doc) if doc.granularity == Granularity::Unsynced => unsynced(
            doc,
            if dense {
                type_scale::BODY_MD
            } else {
                type_scale::TITLE_SM
            },
        ),
        LyricsState::Ready(doc) => synced_list(app, doc, width, dense),
        _ => lyrics_status(app, width),
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
