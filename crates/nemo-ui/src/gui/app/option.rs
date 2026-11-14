//! Hooks and options the parent passes in when it builds the app.

use crate::explore::SleepChoice;
use crate::route::Route;
use crate::settings::UiSettings;
use crate::theme::layout as theme_layout;
use nemo_core::catalog::{AlbumSummary, ArtistSummary, PlaylistSummary};
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
}

impl std::fmt::Debug for UiHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiHooks")
            .field("on_create_playlist", &self.on_create_playlist.is_some())
            .field("on_set_speed", &self.on_set_speed.is_some())
            .field("utc_offset_minutes", &self.utc_offset_minutes)
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
    /// Called on the UI thread with the new settings after every change on the Settings screen
    /// and when the first-run notice is accepted. Must not block (store the value and return).
    pub on_settings: Option<Arc<dyn Fn(UiSettings) + Send + Sync>>,
    /// Music folders shown (read-only) on the Settings screen.
    pub music_folders: Vec<String>,
    /// Called when the user presses "Add folder" in Settings; the app opens its folder picker.
    /// The button is hidden when this is `None`.
    pub on_add_music_folder: Option<Arc<dyn Fn() + Send + Sync>>,
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
            .field("on_settings", &self.on_settings.is_some())
            .field("music_folders", &self.music_folders)
            .field("on_add_music_folder", &self.on_add_music_folder.is_some())
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
            on_settings: None,
            music_folders: Vec::new(),
            on_add_music_folder: None,
        }
    }
}
