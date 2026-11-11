//! Lyric lines, shared by the right pane, the full-screen Now Playing and its compact layout
//! (`design.md` section 10, wave-lyrics criteria 6 to 9). Lines wrap and are never cut; each
//! row is as tall as its wrapped text, measured with the real font advances, and the reducer
//! anchors the scroll on the same numbers. When the active line changes, the old and the new
//! active lines cross-fade size and ink while the list glides on the `spatial.default` spring.
//! Word- and syllable-timed lines colour every piece by its own timing.

use iced::widget::text::{Span, Wrapping};
use iced::widget::{Space, column, container, mouse_area, rich_text, stack, text};
use iced::{Alignment, Color, Element, Length, Padding, Pixels, mouse};
use nemo_core::lyrics::{Granularity, LyricLine, LyricsCredit, LyricsDoc};

use super::super::app::{App, Message};
use super::super::assets::{Icon, font};
use super::super::style::{col, gap, icon, label};
use super::super::textfit;
use super::super::widgets::Offset;
use crate::canvas::coming_alpha;
use crate::lyric_pieces;
use crate::lyrics_view::{self, LINE_GAP, LineRole, LineRows};
use crate::state::{Event, LyricsState};
use crate::theme::{TextStyle, color, type_scale};

/// Room kept at the right of a lyric line for the glow copies, px.
pub const GLOW_INSET: f32 = 4.0;
/// Offset of the glow copies of the active line, px.
const GLOW_PX: f32 = 3.0;

/// Width a lyric line wraps at in a column `width` px wide.
#[must_use]
pub fn wrap_width(width: f32) -> f32 {
    (width - GLOW_INSET).max(1.0)
}

/// Rows `text` wraps onto at `size` px in a column `width` px wide.
#[must_use]
pub fn rows_at(text: &str, width: f32, size: f32) -> u8 {
    u8::try_from(textfit::rows(text, wrap_width(width), size)).unwrap_or(u8::MAX)
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

/// Ink of a lyric line in a role: `on-surface` when active, the "coming" ink otherwise (it keeps
/// 4.5:1 on the song canvas in every base mode, see `canvas.rs`).
#[must_use]
pub fn line_ink(role: LineRole) -> Color {
    let base = crate::theme::color::on_surface();
    match role {
        LineRole::Active => col(base),
        _ => col(base.with_alpha(coming_alpha(color_base()))),
    }
}

/// The base mode the active neutrals belong to (light text on dark, or dark on light).
fn color_base() -> crate::scheme::BaseMode {
    if color::surface().relative_luminance() > 0.5 {
        crate::scheme::BaseMode::Light
    } else {
        crate::scheme::BaseMode::Dark
    }
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
}

impl Look {
    /// A line that did not change.
    #[must_use]
    pub fn settled(role: LineRole, st: TextStyle) -> Self {
        Self {
            from: (role, st),
            to: (role, st),
        }
    }
}

fn text_run<'a>(content: String, size: f32, line_h: f32, bold: bool, ink: Color) -> text::Text<'a> {
    text(content)
        .size(Pixels(size))
        .line_height(text::LineHeight::Absolute(Pixels(line_h)))
        .font(font(bold))
        .color(ink)
        .wrapping(Wrapping::WordOrGlyph)
}

/// One lyric line `width` px wide and its height in px (wrapped rows plus the line gap).
pub fn lyric_line<'a>(
    app: &App,
    line: &LyricLine,
    width: f32,
    look: Look,
) -> (Element<'a, Message>, f32) {
    let p = app.anim.line().clamp(0.0, 1.0);
    let (from_role, from_st) = look.from;
    let (role, st) = look.to;
    let size = mix(f32::from(from_st.size), f32::from(st.size), p);
    let line_h = mix(f32::from(from_st.line), f32::from(st.line), p);
    let rows = rows_at(&line.text, width, size);
    let height = f32::from(rows) * line_h + LINE_GAP;
    let wrap = wrap_width(width);
    let ink = mix_color(line_ink(from_role), line_ink(role), p);
    let bold = role == LineRole::Active;
    if role != LineRole::Active {
        let body = text_run(line.text.clone(), size, line_h, bold, ink).width(Length::Fixed(wrap));
        return (body.into(), height);
    }
    let timed = !line.words.is_empty()
        && matches!(&app.ui.lyrics, LyricsState::Ready(d) if d.granularity >= Granularity::Word);
    // Piece-timed lines get no glow: it would tint the pieces still to come.
    let mut layers: Vec<Element<'a, Message>> = if timed {
        Vec::new()
    } else {
        glow_layers(app, &line.text, (size, line_h), wrap, p)
    };
    let main: Element<'a, Message> = if timed {
        pieces_text(app, line, size, line_h, wrap)
    } else {
        text_run(line.text.clone(), size, line_h, true, ink)
            .width(Length::Fixed(wrap))
            .into()
    };
    let half = GLOW_PX / 2.0;
    layers.push(
        container(main)
            .padding(Padding {
                left: half,
                top: half,
                ..Padding::ZERO
            })
            .into(),
    );
    (stack(layers).width(Length::Fill).into(), height)
}

/// Four faint accent copies behind the active line, fading in with the line change.
fn glow_layers<'a>(
    app: &App,
    text: &str,
    (size, line_h): (f32, f32),
    wrap: f32,
    p: f32,
) -> Vec<Element<'a, Message>> {
    if p <= 0.02 {
        return Vec::new();
    }
    let glow = col(app.accent().primary.with_alpha(0.22 * p));
    [
        (0.0, 0.0),
        (GLOW_PX, 0.0),
        (0.0, GLOW_PX),
        (GLOW_PX, GLOW_PX),
    ]
    .into_iter()
    .map(|(dx, dy)| {
        let copy = text_run(text.to_owned(), size, line_h, true, glow).width(Length::Fixed(wrap));
        container(copy)
            .padding(Padding {
                left: dx,
                top: dy,
                ..Padding::ZERO
            })
            .into()
    })
    .collect()
}

/// The active line as rich text, one span per timed piece (wave-lyrics criterion 8).
fn pieces_text<'a>(
    app: &App,
    line: &LyricLine,
    size: f32,
    line_h: f32,
    wrap: f32,
) -> Element<'a, Message> {
    let t = i64::try_from(app.ui.position.as_millis())
        .unwrap_or(i64::MAX)
        .saturating_add(app.ui.lyrics_offset_ms);
    let base = color::on_surface();
    let coming = coming_alpha(color_base());
    let spans: Vec<Span<'a, (), iced::Font>> =
        lyric_pieces::pieces(line, t, coming, app.ui.reduced_motion)
            .into_iter()
            .map(|piece| Span::new(piece.text).color(col(base.with_alpha(piece.alpha))))
            .collect();
    rich_text(spans)
        .size(Pixels(size))
        .line_height(text::LineHeight::Absolute(Pixels(line_h)))
        .font(font(true))
        .wrapping(Wrapping::WordOrGlyph)
        .width(Length::Fixed(wrap))
        .into()
}

/// The synced lines as a gliding list, with the `Back to current line` pill while the user
/// scrolled away. `dense` picks the right pane sizes.
pub fn synced_list<'a>(
    app: &'a App,
    doc: &'a LyricsDoc,
    width: f32,
    dense: bool,
) -> Element<'a, Message> {
    let active = app.ui.active_lyric();
    let before = if app.anim.line() < 1.0 {
        app.prev_line
    } else {
        active
    };
    let mut c = column![].width(Length::Fill);
    for (i, line) in doc.lines.iter().enumerate() {
        let (from, to) = (LineRole::of(i, before), LineRole::of(i, active));
        let look = Look {
            from: (from, from.style_d(dense)),
            to: (to, to.style_d(dense)),
        };
        let (el, h) = lyric_line(app, line, width, look);
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
            label(
                line.text.clone(),
                st,
                false,
                col(color::on_surface_variant()),
            )
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

/// The credit line a source asks for: its label, the submitter and the link, for example
/// "Lyrics from Unison \u{b7} by Someone" or "Lyrics via Better Lyrics \u{b7} better-lyrics.boidu.dev".
#[must_use]
pub fn credit_text(c: &LyricsCredit) -> String {
    let mut parts = vec![c.label.clone()];
    if let Some(by) = &c.by {
        parts.push(format!("by {by}"));
    }
    if let Some(url) = &c.url {
        let host = url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/');
        if !host.is_empty() {
            parts.push(host.to_owned());
        }
    }
    parts.join(" \u{b7} ")
}

/// Height of the footer's main label row (one row plus air above it), px.
const FOOTER_ROW: f32 = 24.0;

fn footer_parts(app: &App) -> Option<(String, Option<String>)> {
    let LyricsState::Ready(doc) = &app.ui.lyrics else {
        return None;
    };
    Some((
        lyrics_view::footer(Some(doc)),
        app.lyric_credit.as_ref().map(credit_text),
    ))
}

/// Height the footer takes beyond one 24 px label row: the credit line and any wrapped rows.
#[must_use]
pub fn footer_extra(app: &App, width: f32) -> f32 {
    let Some((text, credit)) = footer_parts(app) else {
        return 0.0;
    };
    let st = type_scale::LABEL_SM;
    let row = f32::from(st.line);
    let size = f32::from(st.size);
    let extra_rows = textfit::rows(&text, width, size).saturating_sub(1);
    // Row counts are tiny, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let mut extra = extra_rows as f32 * row;
    if let Some(c) = credit {
        #[allow(clippy::cast_precision_loss)]
        let credit_rows = textfit::rows(&c, width, size) as f32;
        // The credit, the gap under it and air above it, so it never touches a lyric line.
        extra += credit_rows * row + gap::XS + gap::SM;
    }
    extra
}

/// Provider and sync level, and the credit; never hidden and never cut (`design.md` section 10).
pub fn footer(app: &App, width: f32) -> Element<'_, Message> {
    // Without lyrics there is nothing to credit; the empty state names the providers instead.
    let Some((text, credit)) = footer_parts(app) else {
        return Space::new().height(gap::SM).into();
    };
    let st = type_scale::LABEL_SM;
    let wrapped = |s: String, ink: Color| {
        label(s, st, false, ink)
            .wrapping(Wrapping::Word)
            .width(Length::Fixed(width))
    };
    let mut c = column![].width(Length::Fixed(width));
    if let Some(credit) = credit {
        c = c
            .push(Space::new().height(gap::SM))
            .push(wrapped(credit, col(color::on_surface())))
            .push(Space::new().height(gap::XS));
    }
    let rows = textfit::rows(&text, width, f32::from(st.size));
    // Row counts are tiny, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let h = FOOTER_ROW + (rows.saturating_sub(1)) as f32 * f32::from(st.line);
    c.push(
        container(wrapped(text, col(color::on_surface_variant())))
            .height(Length::Fixed(h))
            .align_y(Alignment::End),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credit_names_the_submitter_and_the_link_host() {
        let c = LyricsCredit {
            label: "Lyrics via Better Lyrics".to_owned(),
            url: Some("https://better-lyrics.boidu.dev/".to_owned()),
            by: None,
            by_url: None,
            note: None,
        };
        assert_eq!(
            credit_text(&c),
            "Lyrics via Better Lyrics \u{b7} better-lyrics.boidu.dev"
        );
        let u = LyricsCredit {
            label: "Lyrics from Unison".to_owned(),
            url: None,
            by: Some("mira_k".to_owned()),
            by_url: None,
            note: None,
        };
        assert_eq!(credit_text(&u), "Lyrics from Unison \u{b7} by mira_k");
    }

    #[test]
    fn long_lines_wrap_instead_of_being_cut() {
        let long = "và tâm hồn tôi là một bản nhạc chưa bao giờ được hát hết, dù đêm đã rất dài";
        assert!(rows_at(long, 292.0, 20.0) >= 2);
        assert_eq!(rows_at("short", 292.0, 20.0), 1);
    }
}
