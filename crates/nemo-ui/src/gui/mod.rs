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
pub mod lyric_list;
pub mod shell;
pub mod shimmer;
pub mod style;
pub mod textfit;
pub mod views;
pub mod widgets;

pub use app::{AccountLibrary, App, RestartHandler, UiHooks, UiOptions};
pub use shell::{StartFn, StepReporter, run_with_startup};

use nemo_core::services::Backend;

/// Build the app on `backend`, with the task that starts its first jobs and asks the window for
/// its real size and scale (events keep them current afterwards).
pub(crate) fn boot_app(backend: Backend, options: &UiOptions) -> (App, iced::Task<app::Message>) {
    let (app, jobs) = App::new(backend, options);
    let b = app.backend.clone();
    let jobs = iced::Task::batch(
        jobs.into_iter()
            .map(|j| iced::Task::perform(app::run_job(j, b.clone()), |m| m)),
    );
    let size = iced::window::oldest()
        .and_then(iced::window::size)
        .map(app::Message::Resized);
    let scale = iced::window::oldest()
        .and_then(iced::window::scale_factor)
        .map(app::Message::Rescaled);
    (app, iced::Task::batch([jobs, size, scale]))
}

/// Run the UI on `backend` until the window closes.
///
/// # Errors
/// Returns the iced error when the window or the renderer cannot start.
pub fn run(backend: Backend, options: UiOptions) -> iced::Result {
    let (width, height) = options.window_size;
    let maximized = options.maximized;
    let transparent = options.settings.window_glass;
    let min = crate::theme::layout::MIN_WINDOW;
    iced::application(
        move || {
            let (app, task) = boot_app(backend.clone(), &options);
            (app, task)
        },
        App::update,
        App::view,
    )
    .title(App::title)
    .subscription(App::subscription)
    .theme(App::theme)
    .style(App::style)
    .font(assets::FONT_REGULAR)
    .font(assets::FONT_MEDIUM)
    .font(assets::FONT_SEMIBOLD)
    .font(assets::DISPLAY_REGULAR)
    .font(assets::DISPLAY_SEMIBOLD)
    .default_font(assets::font(false))
    .antialiasing(true)
    .window(iced::window::Settings {
        size: iced::Size::new(width, height),
        min_size: Some(iced::Size::new(min.0, min.1)),
        maximized,
        transparent,
        ..iced::window::Settings::default()
    })
    .run()
}
