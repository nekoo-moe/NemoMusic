//! The song-coloured canvas behind the right Now Playing pane and the full-screen Now Playing
//! (wave-lyrics criterion 12). The colour work ran on the cover worker (`crate::canvas`); here
//! it is only drawn. The pane uses gradient stops, because images cannot be clipped to the
//! pane's round corners on tiny-skia (see `bar.rs`); the full screen has square corners and
//! draws the toned texture. Both cross-fade with the cover (400 ms) and never loop.

use iced::widget::{Space, image, stack};
use iced::{Background, ContentFit, Element, Length};

use super::super::app::{App, Message};
use super::super::style::{DOWN, col, vertical_gradient};
use crate::canvas::{STOPS, mix_stops};
use crate::theme::{Rgba, color};

/// The pane gradient stops now, cross-faded from the previous cover's.
fn pane_stops(app: &App) -> Option<[Rgba; STOPS]> {
    let base = app.ui.settings.base;
    let now = app.cover.as_ref()?.canvas.stops(base);
    let fade = app.anim.cover();
    Some(match &app.prev_cover {
        Some(prev) if fade < 0.999 => mix_stops(prev.canvas.stops(base), now, fade),
        _ => now,
    })
}

/// Background of the right pane: the cover's canvas gradient, or the accent gradient while
/// there is no cover.
#[must_use]
pub fn pane_background(app: &App) -> Background {
    let Some(stops) = pane_stops(app) else {
        let accent = app.accent();
        let base = color::surface_container();
        let top = accent.primary_container.with_alpha(0.50).over(base);
        return vertical_gradient(
            col(top.with_alpha(0.9)),
            col(base.with_alpha(crate::scheme::PANE_ALPHA)),
        );
    };
    let mut g = iced::gradient::Linear::new(iced::Radians(DOWN));
    // Stop positions are k / (STOPS - 1), small exact fractions.
    #[allow(clippy::cast_precision_loss)]
    for (k, c) in stops.iter().enumerate() {
        g = g.add_stop(k as f32 / (STOPS - 1) as f32, col(*c));
    }
    Background::Gradient(g.into())
}

/// The full-screen canvas: the toned cover texture, the old one under the new one while they
/// cross-fade. Plain window background when there is no cover.
pub fn full_backdrop(app: &App) -> Element<'_, Message> {
    let Some(c) = &app.cover else {
        return Space::new().into();
    };
    let base = app.ui.settings.base;
    let draw = |handle: image::Handle, opacity: f32| -> Element<'_, Message> {
        image(handle)
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(ContentFit::Cover)
            .opacity(opacity)
            .into()
    };
    let fade = app.anim.cover();
    match &app.prev_cover {
        Some(prev) if fade < 0.999 => stack![
            draw(prev.canvas.texture(base), 1.0),
            draw(c.canvas.texture(base), fade)
        ]
        .into(),
        _ => draw(c.canvas.texture(base), 1.0),
    }
}
