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
use crate::drift::{Inputs, Mode};
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
        // Nothing but a menu moves while a track menu is open.
        playing: app.ui.is_playing() && !app.surfaces.menu.visible(),
        visible: !matches!(app.ui.route(), Route::NowPlaying),
        software: false,
    }
}

/// The field behind the panes. With `Match colours to the playing song` on and a cover: the
/// cover's toned canvas, drifting slowly, the old cover under the new one while they cross-fade;
/// a flat tonal colour from the same canvas under reduce transparency. Otherwise (the switch is
/// off, or there is no cover) one flat tonal colour from the theme (song-colour-theme rule
/// table).
pub fn window_field(app: &App) -> Element<'_, Message> {
    let Some(cover) = song_cover(app) else {
        return flat_field(app);
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
    stack![
        Element::from(Field::new(layers, inputs)),
        ambient_glow(app),
        vignette()
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// A soft top ambient glow from the active cover accent color.
fn ambient_glow<'a>(app: &App) -> Element<'a, Message> {
    let primary = app.accent().primary;
    let g = iced::gradient::Linear::new(iced::Radians(DOWN))
        .add_stop(0.0, col(primary.with_alpha(0.18)))
        .add_stop(0.5, col(primary.with_alpha(0.04)))
        .add_stop(1.0, iced::Color::TRANSPARENT);
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Gradient(g.into())),
            ..container::Style::default()
        })
        .into()
}

/// A darkening toward the bottom of the window.
fn vignette<'a>() -> Element<'a, Message> {
    let base = color::surface_container_lowest();
    let g = iced::gradient::Linear::new(iced::Radians(DOWN))
        .add_stop(0.0, iced::Color::TRANSPARENT)
        .add_stop(0.4, col(base.with_alpha(0.25)))
        .add_stop(1.0, col(base.with_alpha(0.75)));
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Gradient(g.into())),
            ..container::Style::default()
        })
        .into()
}

/// One flat tonal colour: the middle colour of `scheme::bg_colors` for the theme's seed (a cool
/// grey for Dynamic), cross-faded by `scheme::BgFade`.
fn flat_field(app: &App) -> Element<'_, Message> {
    let flat = app.bg_fade.current()[1];
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(col(flat).into()),
            ..container::Style::default()
        })
        .into()
}

/// The cover that colours the window: none while `Match colours to the playing song` is off.
fn song_cover(app: &App) -> Option<&crate::gui::app::CoverView> {
    app.cover
        .as_ref()
        .filter(|_| app.ui.settings.song_colour_theme)
}

/// The pane gradient stops now, cross-faded from the previous cover's.
fn pane_stops(app: &App) -> Option<[Rgba; STOPS]> {
    let base = app.ui.settings.base;
    let now = song_cover(app)?.canvas.stops(base);
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
    let Some(c) = song_cover(app) else {
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
