//! Headless rendering with `iced_test`'s tiny-skia renderer: no window, no GPU. Used for the
//! snapshot PNGs in `docs/ui/snapshots/` and for layout tests.
//!
//! Set `ICED_TEST_BACKEND=tiny-skia` before starting the process; `iced_test` reads it when it
//! creates the renderer (this crate forbids `unsafe`, so it cannot set the variable itself).

use std::path::{Path, PathBuf};

use iced::{Pixels, Size};
use iced_test::Simulator;
use nemo_core::player::PlayerCommand;
use nemo_core::services::Backend;

use super::app::{App, UiOptions};
use super::assets;
use super::style;
use crate::route::Route;
use crate::state::Event;

/// Render `app` at `size` logical px to a PNG at `out`. The image is written at 2x scale
/// (`iced_test` fixes the scale factor at 2.0), so a 1280 x 800 window gives 2560 x 1600 pixels.
///
/// # Errors
/// A message when the renderer cannot be created or the file cannot be written.
pub fn render_png(app: &App, size: (u32, u32), out: &Path) -> Result<(), String> {
    let settings = iced::Settings {
        fonts: vec![assets::FONT_REGULAR.into(), assets::FONT_SEMIBOLD.into()],
        default_font: assets::font(false),
        default_text_size: Pixels(14.0),
        antialiasing: true,
        ..iced::Settings::default()
    };
    // Window sizes are small positive numbers.
    #[allow(clippy::cast_precision_loss)]
    let logical = Size::new(size.0 as f32, size.1 as f32);
    let mut sim = Simulator::with_size(settings, logical, app.view());
    let snapshot = sim.snapshot(&style::theme()).map_err(|e| e.to_string())?;

    // `matches_image` writes `<stem>-<renderer>.png` when the file does not exist yet.
    let dir = scratch_dir()?;
    let written = snapshot
        .matches_image(dir.join("shot"))
        .map_err(|e| e.to_string())?;
    debug_assert!(written, "a fresh path always 'matches'");
    let produced = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "png"))
        .ok_or_else(|| "no PNG was produced".to_owned())?;
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::copy(&produced, out).map_err(|e| e.to_string())?;
    std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(())
}

fn scratch_dir() -> Result<PathBuf, String> {
    let base = std::env::var_os("KIROCREW_SCRATCH").map_or_else(std::env::temp_dir, PathBuf::from);
    let dir = base.join(format!("nemo-ui-snapshot-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// A scenario for a snapshot: where to be, how far into the track, and how big.
#[derive(Debug, Clone)]
pub struct Scenario {
    /// Screen.
    pub route: Route,
    /// Window size in logical px.
    pub size: (u32, u32),
    /// Playback position of the current track in seconds.
    pub position_secs: u64,
    /// Index of the mock track to show (via the queue).
    pub track: usize,
    /// Play (true) or pause (false).
    pub playing: bool,
}

/// Build an app on `backend`, load everything synchronously and finish all animations, so the
/// first drawn frame is the settled one.
#[must_use]
pub fn build_app(backend: &Backend, scenario: &Scenario) -> App {
    let options = UiOptions {
        reduced_motion: true,
        // Window sizes are small positive numbers.
        #[allow(clippy::cast_precision_loss)]
        window_size: (scenario.size.0 as f32, scenario.size.1 as f32),
        show_frame_readout: false,
        start_route: scenario.route.clone(),
    };
    // Pick the track and position through the real player commands.
    let queue = backend.player.snapshot().queue;
    backend.player.send(PlayerCommand::SetQueue {
        tracks: queue,
        start: scenario.track,
    });
    backend
        .player
        .send(PlayerCommand::Seek(std::time::Duration::from_secs(
            scenario.position_secs,
        )));
    if !scenario.playing {
        backend.player.send(PlayerCommand::Pause);
    }
    let (mut app, jobs) = App::new(backend.clone(), &options);
    app.settle_blocking(jobs);
    app.dispatch(Event::Position(backend.player.position()));
    app.finish_animations();
    app
}
