//! The lyrics footer: provider, sync level and the credit a source asks for; never hidden and
//! never cut (`design.md` section 10).

use iced::widget::text::Wrapping;
use iced::widget::{Space, column, container};
use iced::{Alignment, Color, Element, Length};
use nemo_core::lyrics::LyricsCredit;

use super::super::super::app::{App, Message};
use super::super::super::style::{col, gap, label};
use super::super::super::textfit;
use crate::lyrics_view;
use crate::state::LyricsState;
use crate::theme::{color, type_scale};

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
