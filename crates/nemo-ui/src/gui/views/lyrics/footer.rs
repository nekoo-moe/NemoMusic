//! The lyrics footer: one line with the provider, the sync level and the credit a source asks
//! for; never hidden and never cut (`design.md` section 10).

use iced::widget::text::Wrapping;
use iced::widget::{Space, container};
use iced::{Alignment, Element, Length};

use super::super::super::app::{App, Message};
use super::super::super::style::{col, gap, label};
use super::super::super::textfit;
use crate::lyrics_view;
use crate::state::LyricsState;
use crate::theme::{color, type_scale};

/// Height of the footer's label row (one row plus air above it), px.
const FOOTER_ROW: f32 = 24.0;

/// The footer text, or `None` while there are no lyrics to credit.
fn footer_text(app: &App) -> Option<String> {
    let LyricsState::Ready(doc) = &app.ui.lyrics else {
        return None;
    };
    Some(lyrics_view::footer_line(
        Some(doc),
        app.lyric_credit.as_ref(),
    ))
}

/// Height the footer takes beyond one 24 px label row: any wrapped rows.
#[must_use]
pub fn footer_extra(app: &App, width: f32) -> f32 {
    let Some(text) = footer_text(app) else {
        return 0.0;
    };
    let st = type_scale::LABEL_SM;
    let extra_rows = textfit::rows(&text, width, f32::from(st.size)).saturating_sub(1);
    // Row counts are tiny, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let extra = extra_rows as f32 * f32::from(st.line);
    extra
}

/// Provider, sync level and credit on one line; never hidden and never cut.
pub fn footer(app: &App, width: f32) -> Element<'_, Message> {
    // Without lyrics there is nothing to credit; the empty state names the providers instead.
    let Some(text) = footer_text(app) else {
        return Space::new().height(gap::SM).into();
    };
    let st = type_scale::LABEL_SM;
    let rows = textfit::rows(&text, width, f32::from(st.size));
    // Row counts are tiny, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let h = FOOTER_ROW + (rows.saturating_sub(1)) as f32 * f32::from(st.line);
    container(
        label(text, st, false, col(color::on_surface_variant()))
            .wrapping(Wrapping::Word)
            .width(Length::Fixed(width)),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(h))
    .align_y(Alignment::End)
    .into()
}
