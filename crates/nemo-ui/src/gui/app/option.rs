//! Hooks and options the parent passes in when it builds the app.

#[cfg(doc)]
use super::message::Message;

use crate::explore::SleepChoice;
use crate::route::Route;
use crate::settings::UiSettings;
use crate::theme::layout as theme_layout;
use crate::window_glass::WindowLook;
use nemo_core::catalog::{AlbumSummary, ArtistSummary, PlaylistSummary};
use nemo_core::opt_in::OptInInfo;
use nemo_core::track::{Track, TrackId};
use std::sync::Arc;
use std::time::Instant;

/// Callbacks and data the parent wires for features the UI shows but cannot do itself. Every
/// field is optional; a missing hook makes the matching control say so instead of pretending.
#[derive(Clone, Default)]
pub struct UiHooks {
    /// "Create playlist": called with the name. Without it the button explains it is not connected.
    pub on_create_playlist: Option<Arc<dyn Fn(String) + Send + Sync>>,
    /// Playback speed in percent (100 is off). Without it the speed chips are disabled.
    pub on_set_speed: Option<Arc<dyn Fn(u16) + Send + Sync>>,
    /// Local time zone offset to UTC in minutes (for the daily sections and the greeting; the
    /// app knows the zone, this crate has no time library). 0 means UTC.
    pub utc_offset_minutes: i32,
    /// Resolve ahead of the click (cold-start spec): called with `Some(video)` when the pointer
    /// has rested on the play target of one song for about 150 ms or went down on it, and with
    /// `None` when the pointer leaves it again (cancel). One song at a time, never a list. Must
    /// not block.
    pub on_hint_song: Option<Arc<dyn Fn(Option<nemo_core::track::VideoId>) + Send + Sync>>,
}

impl std::fmt::Debug for UiHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiHooks")
            .field("on_create_playlist", &self.on_create_playlist.is_some())
            .field("on_set_speed", &self.on_set_speed.is_some())
            .field("utc_offset_minutes", &self.utc_offset_minutes)
            .field("on_hint_song", &self.on_hint_song.is_some())
            .finish()
    }
}

/// What the signed-in account has in its library. The parent can fill it with
/// [`Message::AccountLibrary`] once the catalog has library calls; until then the Library page
/// uses the shelves of Home.
#[derive(Debug, Clone, Default)]
pub struct AccountLibrary {
    /// Playlists.
    pub playlists: Vec<PlaylistSummary>,
    /// Saved albums.
    pub albums: Vec<AlbumSummary>,
    /// Followed artists.
    pub artists: Vec<ArtistSummary>,
    /// Liked songs.
    pub songs: Vec<Track>,
}

/// A running sleep timer.
#[derive(Debug, Clone)]
pub struct SleepState {
    /// What was chosen.
    pub choice: SleepChoice,
    /// When to pause (`None` for end of song).
    pub deadline: Option<Instant>,
    /// The song that was playing when the timer started.
    pub track: Option<TrackId>,
}

/// Messages of the "Create playlist" sheet.
#[derive(Debug, Clone)]
pub enum CreateMsg {
    /// Open the sheet.
    Open,
    /// Close it.
    Close,
    /// The name changed.
    Name(String),
    /// Create.
    Confirm,
}

/// Asks the parent to start a fresh copy of the program with the same arguments, after it made
/// sure the settings are saved. Returns `true` when the new copy started: the window then closes
/// itself. Returns `false` when it did not start: the app keeps running. The UI never spawns a
/// process itself. It may block for a moment (it waits for the settings file).
pub type RestartHandler = Arc<dyn Fn() -> bool + Send + Sync>;

/// Called once, on the UI thread, with the window just after it opened and the look of the theme,
/// when the window was created see-through and Reduce transparency is off. The parent may ask the
/// system for a blur behind it; `nemo-ui` names no platform crate and draws none itself.
pub type WindowHandler = Arc<dyn Fn(&dyn iced::window::Window, WindowLook) + Send + Sync>;

/// What opens the operating system's folder dialog. The future resolves to the folder the person
/// chose (as text for the screen), or `None` when the dialog was cancelled. It runs off the UI
/// thread, so a slow dialog never blocks a frame.
#[derive(Clone)]
pub struct FolderPicker(Arc<dyn Fn() -> PickFuture + Send + Sync>);

/// The answer of a [`FolderPicker`].
pub type PickFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Option<String>> + Send>>;

impl FolderPicker {
    /// A picker that starts `open` each time the person asks to add a folder.
    pub fn new(open: impl Fn() -> PickFuture + Send + Sync + 'static) -> Self {
        Self(Arc::new(open))
    }

    /// Open the dialog and wait for the answer.
    pub async fn pick(&self) -> Option<String> {
        (self.0)().await
    }
}

impl std::fmt::Debug for FolderPicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FolderPicker")
    }
}

/// Options for [`run`](super::run).
#[derive(Clone)]
pub struct UiOptions {
    /// Hooks the parent wires (create playlist, speed, lyrics provider switcher).
    pub hooks: UiHooks,
    /// Honour the OS "reduce motion" setting: no cross-fade, no lyric glide.
    pub reduced_motion: bool,
    /// Window size at start, logical px.
    pub window_size: (f32, f32),
    /// Open the window maximized, so it fits small screens such as 1366 x 768 laptops.
    pub maximized: bool,
    /// Show the frame-time readout at start (F3 toggles it).
    pub show_frame_readout: bool,
    /// First screen.
    pub start_route: Route,
    /// Settings at start (the app loads them from its config).
    pub settings: UiSettings,
    /// Open the first-run setup flow at start (the app sets it from
    /// [`UiSettings::needs_setup`]).
    pub start_setup: bool,
    /// Called on the UI thread with the new settings after every change on the Settings screen
    /// and when the first-run notice is accepted. Must not block (store the value and return).
    pub on_settings: Option<Arc<dyn Fn(UiSettings) + Send + Sync>>,
    /// Music folders shown (read-only) on the Settings screen.
    pub music_folders: Vec<String>,
    /// Opens the folder dialog when the user presses "Add folder" (Settings, setup). The folder
    /// it returns is shown in the lists at once; a cancelled dialog changes nothing. The button
    /// is hidden when this is `None`.
    pub on_add_music_folder: Option<FolderPicker>,
    /// Called when the user presses `Restart now` on the Window glass card (bar-material spec).
    /// The pill is hidden when this is `None`.
    pub on_restart: Option<RestartHandler>,
    /// Called once with the see-through window (see [`WindowHandler`]).
    pub on_window_ready: Option<WindowHandler>,
    /// What the SoundCloud switch sends, to which hosts, and its risk (soundcloud-account spec).
    /// The app fills it from the SoundCloud crate, so the UI never knows the service by name.
    /// The SoundCloud page and the setup card offer no switch when this is `None`.
    pub soundcloud_info: Option<OptInInfo>,
}

impl std::fmt::Debug for UiOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiOptions")
            .field("hooks", &self.hooks)
            .field("reduced_motion", &self.reduced_motion)
            .field("window_size", &self.window_size)
            .field("maximized", &self.maximized)
            .field("show_frame_readout", &self.show_frame_readout)
            .field("start_route", &self.start_route)
            .field("settings", &self.settings)
            .field("start_setup", &self.start_setup)
            .field("on_settings", &self.on_settings.is_some())
            .field("music_folders", &self.music_folders)
            .field("on_add_music_folder", &self.on_add_music_folder.is_some())
            .field("on_restart", &self.on_restart.is_some())
            .field("on_window_ready", &self.on_window_ready.is_some())
            .field("soundcloud_info", &self.soundcloud_info.map(|i| i.version))
            .finish()
    }
}

impl Default for UiOptions {
    fn default() -> Self {
        Self {
            hooks: UiHooks::default(),
            reduced_motion: false,
            window_size: theme_layout::DEFAULT_WINDOW,
            maximized: false,
            show_frame_readout: false,
            start_route: Route::Home,
            settings: UiSettings::default(),
            start_setup: false,
            on_settings: None,
            music_folders: Vec::new(),
            on_add_music_folder: None,
            on_restart: None,
            on_window_ready: None,
            soundcloud_info: None,
        }
    }
}
