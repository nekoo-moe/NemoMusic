//! The program iced runs: the splash first, then the app (startup-flow spec, task 1).
//!
//! The window and the splash open at once. The services are built by the caller's start
//! function on its own thread; each finished step arrives as a message and moves the progress
//! line. When the last step is in, [`App`] is built under the splash, which fades out.

use std::sync::Mutex;
use std::time::Instant;

use iced::widget::stack;
use iced::{Element, Subscription, Task, window};
use nemo_core::services::Backend;

use super::app::{self, App, UiOptions};
use super::views::splash::{SplashLook, view as splash_view};
use super::{assets, boot_app, fx, style};
use crate::motion::Spring;
use crate::scheme::accent_palette;
use crate::splash::{Splash, StartStep};
use crate::theme::{color, shape};

/// Reports a finished start-up step to the splash. Cheap and callable from any thread.
pub type StepReporter<'a> = &'a (dyn Fn(StartStep) + Send + Sync);

/// The work that opens the services, run on its own thread behind the first frame. It reports
/// each real step as it finishes, and returns the backend or a plain sentence for the user.
pub type StartFn = Box<dyn for<'a> FnOnce(StepReporter<'a>) -> Result<Backend, String> + Send>;

/// The result of the start function.
#[derive(Clone)]
pub struct Started(Result<Backend, String>);

impl std::fmt::Debug for Started {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Ok(_) => f.write_str("Started(ok)"),
            Err(message) => write!(f, "Started(failed: {message})"),
        }
    }
}

/// Messages of the shell.
#[derive(Debug, Clone)]
pub enum ShellMessage {
    /// The start function finished a step.
    Step(StartStep),
    /// The start function returned.
    Started(Started),
    /// A frame was drawn (only while the splash is up).
    Frame(Instant),
    /// A message of the app.
    App(app::Message),
    /// Close the window (after a failed start).
    Quit,
}

enum Stage {
    Starting,
    Running(Box<App>),
}

/// The splash and, once the services are up, the app.
pub struct Shell {
    stage: Stage,
    options: UiOptions,
    splash: Splash,
    t0: Instant,
    now_ms: u64,
    last_frame: Option<Instant>,
    bar: Spring,
}

impl Shell {
    /// The shell at the first frame, and the task that listens to the start function.
    fn boot(start: StartFn, options: UiOptions) -> (Self, Task<ShellMessage>) {
        let reduced = options.reduced_motion || options.settings.reduce_motion;
        fx::set_reduced_motion(reduced);
        color::set_active(options.settings.base.neutrals());
        shape::set_scale_percent(options.settings.corner_style.percent());
        let splash = Splash::new(reduced);
        let mut bar = Spring::tuned(0.0, 700.0, 1.0);
        bar.snap(splash.fraction());
        let mut shell = Self {
            stage: Stage::Starting,
            options,
            splash,
            t0: Instant::now(),
            now_ms: 0,
            last_frame: None,
            bar,
        };
        let (tx, rx) = iced::futures::channel::mpsc::unbounded::<ShellMessage>();
        let steps = tx.clone();
        let spawned = std::thread::Builder::new()
            .name("startup".into())
            .spawn(move || {
                // A closed channel means the window is gone; nothing is left to tell.
                let report = move |step| {
                    let _ = steps.unbounded_send(ShellMessage::Step(step));
                };
                let result = start(&report);
                let _ = tx.unbounded_send(ShellMessage::Started(Started(result)));
            });
        if spawned.is_err() {
            shell
                .splash
                .fail("NemoMusic could not start its background work.".to_owned());
        }
        (shell, Task::run(rx, |message| message))
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.t0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// iced `update`.
    pub fn update(&mut self, message: ShellMessage) -> Task<ShellMessage> {
        match message {
            ShellMessage::Step(step) => {
                self.splash.finish(step, self.elapsed_ms());
                Task::none()
            }
            ShellMessage::Started(Started(Ok(backend))) => self.open_app(backend),
            ShellMessage::Started(Started(Err(message))) => {
                self.splash.fail(message);
                Task::none()
            }
            ShellMessage::Frame(now) => {
                self.on_frame(now);
                Task::none()
            }
            ShellMessage::App(message) => match &mut self.stage {
                Stage::Running(app) => app.update(message).map(ShellMessage::App),
                Stage::Starting => Task::none(),
            },
            ShellMessage::Quit => iced::exit(),
        }
    }

    /// Build the app under the splash and start its first requests.
    fn open_app(&mut self, backend: Backend) -> Task<ShellMessage> {
        let (app, task) = boot_app(backend, &self.options);
        self.stage = Stage::Running(Box::new(app));
        self.splash
            .finish(StartStep::FirstScreen, self.elapsed_ms());
        task.map(ShellMessage::App)
    }

    fn on_frame(&mut self, now: Instant) {
        let dt = self
            .last_frame
            .map_or(0.0, |last| now.duration_since(last).as_secs_f32());
        self.last_frame = Some(now);
        self.now_ms =
            u64::try_from(now.saturating_duration_since(self.t0).as_millis()).unwrap_or(u64::MAX);
        self.splash.tick(self.now_ms);
        self.bar.target = self.splash.fraction();
        if self.splash.opacity(self.now_ms) < 1.0 || fx::reduced_motion() {
            self.bar.finish();
        } else {
            self.bar.step(dt);
        }
    }

    /// The window title: the app's once it runs.
    #[must_use]
    pub fn title(&self) -> String {
        match &self.stage {
            Stage::Running(app) => app.title(),
            Stage::Starting => "NemoMusic".to_owned(),
        }
    }

    /// iced `theme`.
    #[must_use]
    pub fn theme(&self) -> iced::Theme {
        style::theme()
    }

    /// iced `style`: the window colour. Transparent while the window glass is clear, so only
    /// the backdrop decides what the desktop shows through (appearance-glass spec); the splash
    /// and every other state keep the opaque colour of the theme.
    #[must_use]
    pub fn style(&self, theme: &iced::Theme) -> iced::theme::Style {
        let base = iced::theme::Base::base(theme);
        match &self.stage {
            Stage::Running(app) if app.window_is_clear() => iced::theme::Style {
                background_color: iced::Color::TRANSPARENT,
                ..base
            },
            _ => base,
        }
    }

    /// iced `subscription`: the app's streams, and frames while the splash moves.
    pub fn subscription(&self) -> Subscription<ShellMessage> {
        let mut subs = Vec::new();
        if let Stage::Running(app) = &self.stage {
            subs.push(app.subscription().map(ShellMessage::App));
        }
        if self.splash.visible() && self.splash.failure().is_none() {
            subs.push(window::frames().map(ShellMessage::Frame));
        }
        Subscription::batch(subs)
    }

    /// iced `view`.
    #[must_use]
    pub fn view(&self) -> Element<'_, ShellMessage> {
        let splash = self.splash.visible().then(|| self.splash_layer());
        match (&self.stage, splash) {
            (Stage::Running(app), Some(layer)) => {
                stack![app.view().map(ShellMessage::App), layer].into()
            }
            (Stage::Running(app), None) => app.view().map(ShellMessage::App),
            (Stage::Starting, layer) => layer.unwrap_or_else(|| iced::widget::Space::new().into()),
        }
    }

    /// The idle accent: the saved theme on the saved base, with no song colour.
    fn splash_layer(&self) -> Element<'static, ShellMessage> {
        let s = &self.options.settings;
        let status = self.splash.status(self.now_ms);
        let look = SplashLook {
            fraction: self.bar.value,
            status: &status,
            opacity: self.splash.opacity(self.now_ms),
            accent: accent_palette(s.theme, s.base, s.accent_intensity, None),
            failed: self.splash.failure().is_some(),
        };
        splash_view(&look, ShellMessage::Quit)
    }
}

/// Run the UI: the window and the splash open at once, `start` opens the services behind the
/// first frame, and the app takes over when they are ready.
///
/// # Errors
/// Returns the iced error when the window or the renderer cannot start.
pub fn run_with_startup(start: StartFn, options: UiOptions) -> iced::Result {
    let (width, height) = options.window_size;
    let maximized = options.maximized;
    // The window is created once: it is see-through only when the switch is on now.
    let transparent = options.settings.window_glass;
    let min = crate::theme::layout::MIN_WINDOW;
    let slot = Mutex::new(Some((start, options)));
    iced::application(
        move || {
            let taken = slot.lock().ok().and_then(|mut guard| guard.take());
            match taken {
                Some((start, options)) => Shell::boot(start, options),
                // The boot function runs once; this is a state that cannot happen.
                None => Shell::boot(
                    Box::new(|_| Err("NemoMusic could not start twice.".to_owned())),
                    UiOptions::default(),
                ),
            }
        },
        Shell::update,
        Shell::view,
    )
    .title(Shell::title)
    .subscription(Shell::subscription)
    .theme(Shell::theme)
    .style(Shell::style)
    .font(assets::FONT_REGULAR)
    .font(assets::FONT_MEDIUM)
    .font(assets::FONT_SEMIBOLD)
    .font(assets::DISPLAY_REGULAR)
    .font(assets::DISPLAY_SEMIBOLD)
    .default_font(assets::font(false))
    .antialiasing(true)
    .window(window::Settings {
        size: iced::Size::new(width, height),
        min_size: Some(iced::Size::new(min.0, min.1)),
        maximized,
        transparent,
        ..window::Settings::default()
    })
    .run()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    use super::*;
    use crate::mock::mock_backend;
    use crate::splash::SplashPhase;

    fn shell_with(start: StartFn) -> Shell {
        Shell::boot(start, UiOptions::default()).0
    }

    fn at(shell: &Shell, ms: u64) -> Instant {
        shell.t0 + Duration::from_millis(ms)
    }

    #[test]
    fn the_first_frame_does_not_wait_for_the_start_function() {
        let (release, wait) = channel::<()>();
        let finished = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&finished);
        let shell = shell_with(Box::new(move |_| {
            let _ = wait.recv();
            flag.store(true, Ordering::SeqCst);
            Err("stopped by the test".to_owned())
        }));
        // `boot` returned while the start function is still blocked, and the first view is the
        // splash, built without any backend.
        assert!(!finished.load(Ordering::SeqCst));
        assert_eq!(shell.title(), "NemoMusic");
        assert_eq!(shell.splash.phase(), SplashPhase::Showing);
        drop(shell.view());
        let _ = release.send(());
    }

    #[test]
    fn the_app_opens_under_the_splash_and_the_splash_fades_after_the_floor() {
        let mut shell = shell_with(Box::new(|_| Err("unused".to_owned())));
        for step in [
            StartStep::Library,
            StartStep::CredentialStore,
            StartStep::Player,
        ] {
            drop(shell.update(ShellMessage::Step(step)));
        }
        drop(shell.update(ShellMessage::Started(Started(Ok(mock_backend())))));
        assert!(matches!(shell.stage, Stage::Running(_)));
        drop(shell.view());

        drop(shell.update(ShellMessage::Frame(at(&shell, 100))));
        assert_eq!(shell.splash.phase(), SplashPhase::Showing);
        drop(shell.update(ShellMessage::Frame(at(&shell, 520))));
        assert_eq!(shell.splash.phase(), SplashPhase::Fading);
        drop(shell.update(ShellMessage::Frame(at(&shell, 760))));
        assert_eq!(shell.splash.phase(), SplashPhase::Gone);
        drop(shell.view());
    }

    #[test]
    fn a_failed_start_stays_on_the_splash_with_its_sentence() {
        let mut shell = shell_with(Box::new(|_| Err("unused".to_owned())));
        drop(shell.update(ShellMessage::Started(Started(Err(
            "NemoMusic could not start. The data folder is read-only.".to_owned(),
        )))));
        assert!(matches!(shell.stage, Stage::Starting));
        drop(shell.update(ShellMessage::Frame(at(&shell, 9_000))));
        assert!(shell.splash.visible());
        assert!(
            shell
                .splash
                .failure()
                .is_some_and(|f| f.contains("read-only"))
        );
        drop(shell.view());
    }
}
