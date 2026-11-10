//! Headless rendering with `iced_test`'s tiny-skia renderer: no window, no GPU. Used for the
//! snapshot PNGs in `docs/ui/snapshots/` and for layout tests.
//!
//! Set `ICED_TEST_BACKEND=tiny-skia` before starting the process; `iced_test` reads it when it
//! creates the renderer (this crate forbids `unsafe`, so it cannot set the variable itself).

use std::path::{Path, PathBuf};

use iced::{Pixels, Size};
use iced_test::Simulator;
use nemo_core::player::{PlayState, PlayerCommand};
use nemo_core::services::Backend;

use nemo_core::account::AccountError;
use nemo_core::secret::Secret;

use super::app::{AccountMsg, App, HomeFilter, Message, NpTab, SettingsSection, UiOptions};
use super::assets;
use super::style;
use crate::route::Route;
use crate::settings::UiSettings;
use crate::state::{Event, Toast};

/// Render `app` at `size` logical px to a PNG at `out`. The image is written at 2x scale
/// (`iced_test` fixes the scale factor at 2.0), so a 1280 x 800 window gives 2560 x 1600 pixels.
///
/// # Errors
/// A message when the renderer cannot be created or the file cannot be written.
pub fn render_png(app: &App, size: (u32, u32), out: &Path) -> Result<(), String> {
    render_view_png(app.view(), size, out)
}

/// Type specimen: Vietnamese and Latin text at the sizes and weights of the type scale, with
/// tabular digits. Proves the bundled font draws the owner's test strings.
#[must_use]
pub fn specimen<'a>() -> iced::Element<'a, Message> {
    use crate::theme::type_scale as t;
    use iced::widget::{column, container};
    let ink = style::col(crate::theme::color::ON_SURFACE);
    let dim = style::col(crate::theme::color::ON_SURFACE_VARIANT);
    let lines: [(crate::theme::TextStyle, bool, &str); 8] = [
        (
            t::DISPLAY_MD,
            true,
            "M\u{1edb}i Quen Nhau \u{110}\u{e3} R\u{1ee7}",
        ),
        (t::HEADLINE_MD, true, "HIEUTHUHAI, Ph\u{1b0}\u{1a1}ng Ly"),
        (
            t::TITLE_LG,
            true,
            "\u{110}\u{ea}m M\u{1b0}a Nh\u{1edb} Ph\u{1ed1} \u{b7} Lam Khu\u{ea}",
        ),
        (
            t::TITLE_MD,
            false,
            "ANH L\u{c0} TH\u{1eb0}NG T\u{1ed2}I \u{b7} Ch\u{103}m Hoa \u{b7} L\u{1edd}i H\u{1ee9}a N\u{103}m 18",
        ),
        (
            t::BODY_MD,
            false,
            "Ti\u{1ebf}ng Vi\u{1ec7}t: \u{1ea0}\u{1ea2}\u{1ea4}\u{1ea6}\u{1ea8}\u{1eaa}\u{1eac}\u{1eae}\u{1eb0}\u{1eb2}\u{1eb4}\u{1eb6} \u{1ebe}\u{1ec0}\u{1ec2}\u{1ec4}\u{1ec6} \u{1ed0}\u{1ed2}\u{1ed4}\u{1ed6}\u{1ed8} \u{1edc}\u{1ede}\u{1ee0}\u{1ee2} \u{1eea}\u{1eec}\u{1eee}\u{1ef0} \u{1ef2}\u{1ef4}\u{1ef6}\u{1ef8}",
        ),
        (
            t::BODY_MD,
            false,
            "ạảấầẩẫậắằẳẵặ ẹẻẽếềểễệ ọỏốồổỗộớờởỡợ ụủứừửữự ỳỵỷỹ \u{111}",
        ),
        (
            t::LABEL_MD,
            true,
            "0:11  1:11  2:22  3:33  4:44  5:55  6:66  7:77  8:88  9:99  (tabular digits)",
        ),
        (
            t::BODY_SM,
            false,
            "Opus 160 kbps \u{b7} Lossy \u{b7} Resample \u{b7} Lyrics: LRCLIB \u{b7} Synced",
        ),
    ];
    let mut c = column![].spacing(12);
    for (i, (st, bold, text)) in lines.into_iter().enumerate() {
        c = c.push(style::label(text, st, bold, if i < 4 { ink } else { dim }));
    }
    container(c)
        .padding(32)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .style(|_| container::Style {
            background: Some(style::col(crate::theme::color::SURFACE_CONTAINER).into()),
            ..container::Style::default()
        })
        .into()
}

/// Render any view at `size` logical px to a PNG (2x scale, see [`render_png`]).
///
/// # Errors
/// A message when the renderer cannot be created or the file cannot be written.
pub fn render_view_png(
    view: iced::Element<'_, Message>,
    size: (u32, u32),
    out: &Path,
) -> Result<(), String> {
    let settings = iced::Settings {
        fonts: vec![
            assets::FONT_REGULAR.into(),
            assets::FONT_MEDIUM.into(),
            assets::FONT_SEMIBOLD.into(),
        ],
        default_font: assets::font(false),
        default_text_size: Pixels(14.0),
        antialiasing: true,
        ..iced::Settings::default()
    };
    // Window sizes are small positive numbers.
    #[allow(clippy::cast_precision_loss)]
    let logical = Size::new(size.0 as f32, size.1 as f32);
    let mut sim = Simulator::with_size(settings, logical, view);
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
    /// Settings at start (`tos_acknowledged` decides whether the notice sheet shows).
    pub settings: UiSettings,
    /// Music folders listed in Settings.
    pub music_folders: Vec<String>,
    /// Open the queue panel.
    pub queue_open: bool,
    /// Type this into the search box (and wait for the answer).
    pub search: Option<String>,
    /// Show the player as buffering.
    pub buffering: bool,
    /// Open the signal-path popover.
    pub signal_open: bool,
    /// Open the track menu on this mock track.
    pub menu_track: Option<usize>,
    /// Show this notice above the bar.
    pub toast: Option<String>,
    /// Settings section to show.
    pub section: SettingsSection,
    /// Tab of the right pane.
    pub np_tab: NpTab,
    /// Force the right pane open or closed (default: the window size decides).
    pub pane_open: Option<bool>,
    /// Home filter chip.
    pub home_filter: HomeFilter,
    /// The sign-in sheet to show, if any.
    pub sheet: Sheet,
}

/// Which state of the sign-in sheet a snapshot shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sheet {
    /// No sheet.
    #[default]
    None,
    /// Signed out, notice accepted.
    SignIn,
    /// The masked cookie field with a typed (hidden) value.
    Cookie,
    /// A blocked sign-in window and its plain-words error.
    Error,
    /// Signed in (use a signed-in backend).
    SignedIn,
    /// Signed in, waiting for the sign-out confirmation.
    ConfirmSignOut,
}

impl Default for Scenario {
    fn default() -> Self {
        Self {
            route: Route::Home,
            size: (1366, 697),
            position_secs: 83,
            track: 1,
            playing: false,
            settings: UiSettings {
                tos_acknowledged: true,
                ..UiSettings::default()
            },
            music_folders: Vec::new(),
            queue_open: false,
            search: None,
            buffering: false,
            signal_open: false,
            menu_track: None,
            toast: None,
            section: SettingsSection::default(),
            np_tab: NpTab::Lyrics,
            pane_open: None,
            home_filter: HomeFilter::All,
            sheet: Sheet::None,
        }
    }
}

/// Run one message and everything it starts to completion.
fn run(app: &mut App, message: Message) {
    let jobs = app.update_core(message);
    app.settle_blocking(jobs);
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
        settings: scenario.settings.clone(),
        music_folders: scenario.music_folders.clone(),
        ..UiOptions::default()
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
    app.settings_section = scenario.section;
    app.dispatch(Event::Position(backend.player.position()));
    if let Some(q) = &scenario.search {
        run(&mut app, Message::Ui(Event::Go(Route::Search(q.clone()))));
    }
    if scenario.queue_open {
        run(&mut app, Message::Ui(Event::ToggleQueue));
    }
    if scenario.signal_open {
        run(&mut app, Message::Ui(Event::ToggleSignalPath));
    }
    if let Some(n) = scenario.menu_track {
        run(
            &mut app,
            Message::Ui(Event::OpenMenu(crate::mock::track(n))),
        );
    }
    if let Some(open) = scenario.pane_open {
        app.pane_open = open;
    }
    if scenario.np_tab != NpTab::Lyrics {
        run(&mut app, Message::NpTab(scenario.np_tab));
    }
    app.home_filter = scenario.home_filter;
    match scenario.sheet {
        Sheet::None => {}
        Sheet::SignIn | Sheet::SignedIn => run(&mut app, Message::Account(AccountMsg::Open)),
        Sheet::Cookie => {
            run(&mut app, Message::Account(AccountMsg::Open));
            run(&mut app, Message::Account(AccountMsg::CookieMode(true)));
            run(
                &mut app,
                Message::Account(AccountMsg::CookieInput(Secret::new(
                    "not-a-real-cookie-value".to_owned(),
                ))),
            );
        }
        Sheet::Error => {
            run(&mut app, Message::Account(AccountMsg::Open));
            run(
                &mut app,
                Message::Account(AccountMsg::SignedIn(Err(AccountError::Blocked))),
            );
        }
        Sheet::ConfirmSignOut => {
            run(&mut app, Message::Account(AccountMsg::Open));
            run(&mut app, Message::Account(AccountMsg::AskSignOut));
        }
    }
    if scenario.buffering {
        app.ui.snapshot.state = PlayState::Buffering;
    }
    if let Some(text) = &scenario.toast {
        app.ui.toast = Some(Toast {
            text: text.clone(),
            left_ms: 10_000.0,
            undo: false,
        });
    }
    app.finish_animations();
    app
}
