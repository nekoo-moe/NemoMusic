//! The song-coloured canvas: behind the whole window (living-backdrop spec), behind the right Now
//! Playing pane and behind the full-screen Now Playing (wave-lyrics criterion 12). The colour
//! work ran on the cover worker (`crate::canvas`); here it is only drawn. The pane uses gradient
//! stops, because images cannot be clipped to the pane's round corners on tiny-skia (see
//! `bar.rs`); the full screen has square corners and draws the toned texture. All of them
//! cross-fade with the cover (400 ms). Only the window backdrop moves: a slow drift at a capped
//! frame rate ([`crate::drift`]).

use iced::widget::{Space, container, image, stack};
use iced::{Background, ContentFit, Element, Length};

use super::super::app::{App, Message};
use super::super::style::{DOWN, col, vertical_gradient};
use crate::canvas::{STOPS, mix_stops};
use crate::drift::{Inputs, Mode, VIGNETTE_ALPHA};
use crate::route::Route;
use crate::theme::{Rgba, color};

mod field;
#[cfg(test)]
mod tests;

pub use field::{Field, Layer};

/// What decides how the backdrop is drawn (see [`crate::drift::Mode`]). The window focus and
/// the renderer are filled in by the widget itself; here they read "focused, GPU".
#[must_use]
pub fn window_inputs(app: &App) -> Inputs {
    Inputs {
        reduce_motion: app.ui.reduced_motion,
        reduce_transparency: app.ui.settings.reduce_transparency,
        focused: true,
        playing: app.ui.is_playing(),
        visible: !matches!(app.ui.route(), Route::NowPlaying),
        software: false,
    }
}

/// The field behind the panes. With the Dynamic theme and a cover: the cover's toned canvas,
/// drifting slowly, the old cover under the new one while they cross-fade; a flat tonal colour
/// from the same canvas under reduce transparency. Otherwise (a fixed theme, or no cover) the
/// soft three-colour gradient of `scheme::bg_colors`.
pub fn window_field(app: &App) -> Element<'_, Message> {
    let dynamic = app.ui.settings.theme.seed().is_none();
    let Some(cover) = app.cover.as_ref().filter(|_| dynamic) else {
        return gradient_field(app);
    };
    let base = app.ui.settings.base;
    let fade = app.anim.cover();
    let prev = app.prev_cover.as_ref().filter(|_| fade < 0.999);
    let inputs = window_inputs(app);
    if Mode::of(inputs) == Mode::Flat {
        let mid = STOPS / 2;
        let now = cover.canvas.stops(base)[mid];
        let flat = prev.map_or(now, |p| {
            crate::palette::mix_rgb(p.canvas.stops(base)[mid], now, f64::from(fade))
        });
        return container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(col(flat).into()),
                ..container::Style::default()
            })
            .into();
    }
    let mut layers = Vec::with_capacity(2);
    if let Some(p) = prev {
        layers.push(Layer {
            handle: p.canvas.texture(base),
            opacity: 1.0,
        });
    }
    layers.push(Layer {
        handle: cover.canvas.texture(base),
        opacity: if prev.is_some() { fade } else { 1.0 },
    });
    stack![Element::from(Field::new(layers, inputs)), vignette()]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// A darkening toward the bottom of the window.
fn vignette<'a>() -> Element<'a, Message> {
    let base = color::surface_container_lowest();
    let g = iced::gradient::Linear::new(iced::Radians(DOWN))
        .add_stop(0.0, iced::Color::TRANSPARENT)
        .add_stop(1.0, col(base.with_alpha(VIGNETTE_ALPHA)));
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Gradient(g.into())),
            ..container::Style::default()
        })
        .into()
}

/// The soft diagonal gradient of three dark tonal colours (`scheme::bg_colors`), cross-faded by
/// `scheme::BgFade`, with the vignette.
fn gradient_field(app: &App) -> Element<'_, Message> {
    let [c0, c1, c2] = app.bg_fade.current();
    let diag = iced::gradient::Linear::new(iced::Radians(2.35))
        .add_stop(0.0, col(c0))
        .add_stop(0.55, col(c1))
        .add_stop(1.0, col(c2));
    stack![
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Gradient(diag.into())),
                ..container::Style::default()
            }),
        vignette()
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

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
