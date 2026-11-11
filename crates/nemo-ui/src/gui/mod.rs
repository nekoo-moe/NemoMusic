//! The iced shell. Everything here needs the `gui` feature; pure logic lives in the crate root.

#![allow(
    clippy::too_many_lines,
    clippy::large_enum_variant,
    clippy::must_use_candidate
)] // view builders are long declarative trees; messages are moved, not copied in hot loops

pub mod app;
pub mod assets;
pub mod fx;
pub mod headless;
pub mod style;
pub mod textfit;
pub mod views;
pub mod widgets;

pub use app::{App, UiOptions};

use nemo_core::services::Backend;

/// Run the UI on `backend` until the window closes.
///
/// # Errors
/// Returns the iced error when the window or the renderer cannot start.
pub fn run(backend: Backend, options: UiOptions) -> iced::Result {
    let (width, height) = options.window_size;
    let maximized = options.maximized;
    let min = crate::theme::layout::MIN_WINDOW;
    iced::application(
        move || {
            let (app, jobs) = App::new(backend.clone(), &options);
            let b = app.backend.clone();
            let jobs = iced::Task::batch(
                jobs.into_iter()
                    .map(|j| iced::Task::perform(app::run_job(j, b.clone()), |m| m)),
            );
            // Ask for the real window size once; resize events keep it current afterwards.
            let size = iced::window::oldest()
                .and_then(iced::window::size)
                .map(app::Message::Resized);
            (app, iced::Task::batch([jobs, size]))
        },
        App::update,
        App::view,
    )
    .title(App::title)
    .subscription(App::subscription)
    .theme(App::theme)
    .font(assets::FONT_REGULAR)
    .font(assets::FONT_MEDIUM)
    .font(assets::FONT_SEMIBOLD)
    .default_font(assets::font(false))
    .antialiasing(true)
    .window(iced::window::Settings {
        size: iced::Size::new(width, height),
        min_size: Some(iced::Size::new(min.0, min.1)),
        maximized,
        ..iced::window::Settings::default()
    })
    .run()
}
