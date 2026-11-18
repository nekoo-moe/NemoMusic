//! Frame cost of the window backdrop on the CPU renderer (tiny-skia, headless), for the
//! song-colour-theme spec: one flat frame, one drift frame and one drift plus heartbeat frame
//! (the fastest of the measured frames, so other processes on the machine count less).
//! Every frame builds the renderer and draws the backdrop alone at 2x, so the three numbers share
//! the same set-up cost; the backdrop's own cost is the difference to the flat one. This is the
//! slowest renderer (the GPU path draws 4 textured quads), not a measure of a real window.

use std::time::Instant;

use iced::{Pixels, Size};
use iced_test::Simulator;
use nemo_core::services::Backend;

use super::super::views::backdrop::window_field;
use super::super::{assets, style};
use super::{Message, Scenario, build_app};
use crate::route::Route;

/// Time of the fastest frame in ms, with the song colours off (a flat colour), with the drift, and
/// with the drift and the heartbeat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackdropCost {
    /// Flat colour: the set-up cost every frame shares.
    pub flat_ms: f64,
    /// Colour fields drifting.
    pub drift_ms: f64,
    /// Colour fields drifting and pulsing.
    pub drift_beat_ms: f64,
    /// What the CPU renderer really draws: the one composited image, no motion.
    pub cpu_ms: f64,
}

/// Measure `frames` frames of each kind on `backend` at `size` logical px.
///
/// # Errors
/// A message when the renderer cannot be created.
pub fn backdrop_frame_cost(
    backend: &Backend,
    size: (u32, u32),
    frames: u32,
) -> Result<BackdropCost, String> {
    let mut app = build_app(
        backend,
        &Scenario {
            route: Route::Home,
            size,
            track: 1,
            playing: true,
            full_motion: true,
            ..Scenario::default()
        },
    );
    let settings = || iced::Settings {
        fonts: assets::ALL_FONTS.iter().map(|f| (*f).into()).collect(),
        default_font: assets::font(false),
        default_text_size: Pixels(14.0),
        antialiasing: true,
        ..iced::Settings::default()
    };
    // Window sizes are small positive numbers.
    #[allow(clippy::cast_precision_loss)]
    let logical = Size::new(size.0 as f32, size.1 as f32);
    let step_ms = 1000.0 / f64::from(crate::theme::motion::AMBIENT_FPS);
    let mut fastest = |song_colours: bool, heartbeat: bool, preview: bool| -> Result<f64, String> {
        app.ui.settings.song_colour_theme = song_colours;
        app.ui.settings.song_heartbeat = heartbeat;
        let mut best = f64::MAX;
        for k in 0..frames {
            // Spread the frames over a drift cycle and one heartbeat.
            app.backdrop_preview_ms = preview.then(|| f64::from(k) * step_ms * 7.0);
            let view: iced::Element<'_, Message> = window_field(&app);
            let start = Instant::now();
            let mut sim = Simulator::with_size(settings(), logical, view);
            sim.snapshot(&style::theme()).map_err(|e| e.to_string())?;
            best = best.min(start.elapsed().as_secs_f64() * 1000.0);
        }
        Ok(best)
    };
    Ok(BackdropCost {
        flat_ms: fastest(false, false, true)?,
        drift_ms: fastest(true, false, true)?,
        drift_beat_ms: fastest(true, true, true)?,
        cpu_ms: fastest(true, true, false)?,
    })
}
