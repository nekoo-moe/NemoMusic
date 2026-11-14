//! The iced application: state, messages, update, subscriptions. The pure reducer in
//! [`crate::state`] decides what happens; this file runs its effects against the [`Backend`].
//!
//! Nothing here blocks: every catalog call, image decode and lookup is a [`Job`] that runs as a
//! task, and its answer comes back as a [`Message`].

use super::fx;
use crate::anim::Anim;
use crate::daily::{DailyData, DailySection, DayContext};
use crate::explore::{ContentFilter, LibLayout, LibSort, LibTab};
use crate::frame_stats::FrameStats;
use crate::layout;
use crate::lru::{ART_CAPACITY, Lru};
use crate::palette::Seed;
use crate::route::Route;
use crate::scheme::{
    AccentIntensity, BaseMode, BgFade, CornerStyle, SchemeFade, ThemeChoice, bg_colors,
};
use crate::settings::UiSettings;
use crate::state::{Event, UiState};
use crate::theme::{color, shape};
use iced::Size;
use nemo_core::catalog::{AlbumSummary, Shelf};
use nemo_core::lyrics::{LyricsCandidate, LyricsCredit};
use nemo_core::services::{Backend, LyricsProviderStatus};
use nemo_core::track::{Track, TrackId};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

mod account;
mod art;
mod dispatch;
mod handle;
mod job;
mod message;
mod model;
mod option;
mod query;
mod subscribe;
mod sync;
mod update;

pub use art::{ArtEntry, ArtShape, ArtSpec, CanvasView, CoverView, TILE_RADIUS};
pub use job::{art_key, run_job};
pub use message::{AccountBundle, CoverBuilt, Job, Message};
pub use model::{
    AccountMsg, AccountUi, CompactTab, DetailData, DetailKey, DetailKind, DetailState, HomeFilter,
    HomePhase, HomeState, LibFilter, ListSource, NpTab, SettingsSection,
};
pub use option::{AccountLibrary, CreateMsg, SleepState, UiHooks, UiOptions};

/// Widget id of the search box.
pub const SEARCH_ID: &str = "nemo-search";
/// Most rows a list draws at once; "Show more" adds this many again.
pub const ROWS_STEP: usize = 200;
/// Images fetched at the same time.
const ART_PARALLEL: usize = 6;
/// Message of the search page while the first-run notice is not accepted.
pub const GATE_MESSAGE: &str = "YouTube Music is waiting for you to read the notice.";

/// Size of the mini player window.
pub const MINI_SIZE: (f32, f32) = (420.0, 168.0);

/// The application state.
pub struct App {
    pub(crate) backend: Backend,
    pub(crate) ui: UiState,
    pub(crate) window: Size,
    pub(crate) home: HomeState,
    pub(crate) local_albums: Vec<AlbumSummary>,
    pub(crate) local_tracks: Vec<Track>,
    pub(crate) library_requested: bool,
    pub(crate) library_loaded: bool,
    pub(crate) details: HashMap<DetailKey, DetailState>,
    pub(crate) art: Lru<ArtEntry>,
    wants: RefCell<Vec<ArtSpec>>,
    art_queue: VecDeque<ArtSpec>,
    art_inflight: usize,
    pub(crate) cover: Option<CoverView>,
    /// The cover before the current one, drawn under it while they cross-fade.
    pub(crate) prev_cover: Option<CoverView>,
    pub(crate) anim: Anim,
    pub(crate) pane_open: bool,
    pane_user_set: bool,
    pub(crate) np_tab: NpTab,
    pub(crate) home_filter: HomeFilter,
    pub(crate) lib_filter: Option<LibFilter>,
    pub(crate) account: AccountUi,
    /// Likes the user changed in this session (over what the Liked songs shelf says).
    likes: HashMap<TrackId, bool>,
    last_line: Option<usize>,
    /// The active lyric line before the last change (the lines cross-fade from it).
    pub(crate) prev_line: Option<usize>,
    pub(crate) seek_drag: Option<f32>,
    pub(crate) compact_tab: CompactTab,
    pub(crate) settings_section: SettingsSection,
    pub(crate) rows_limit: usize,
    pub(crate) spin: f32,
    /// Phase of the loading shimmer, 0 to 1.
    pub(crate) shimmer: f32,
    /// Phase of the equaliser bars on the playing row.
    pub(crate) eq_phase: f32,
    /// Sliding selection pill of chip rows: x and width springs (0: Home, 1: Library tabs).
    pub(crate) chip_pills: [(crate::motion::Spring, crate::motion::Spring); 2],
    chip_pills_init: bool,
    /// Queue reorder: the two rows that swapped and the offset spring (1 to 0).
    pub(crate) queue_swap: (usize, usize),
    pub(crate) queue_anim: crate::motion::Spring,
    pub(crate) frame_stats: FrameStats,
    pub(crate) readout: bool,
    pub(crate) music_folders: Vec<String>,
    on_settings: Option<Arc<dyn Fn(UiSettings) + Send + Sync>>,
    on_add_music_folder: Option<Arc<dyn Fn() + Send + Sync>>,
    focus_search: bool,
    last_route: Route,
    last_frame: Option<Instant>,
    /// Animated surface colours (the theme switch).
    pub(crate) scheme_fade: SchemeFade,
    /// The coloured background field behind the panes, and its cross-fade.
    pub(crate) bg_fade: BgFade,
    /// Seeds of the album and playlist pages that were opened (their cover colour).
    page_seeds: HashMap<DetailKey, Seed>,
    /// Seed of the playing cover, for the Dynamic theme.
    cover_seed: Option<Seed>,
    /// The theme settings the screen shows right now.
    theme_key: Option<(ThemeChoice, BaseMode, AccentIntensity, CornerStyle)>,
    /// Header content chip on Home.
    pub(crate) content_filter: ContentFilter,
    /// Library tab, sort and layout.
    pub(crate) lib_tab: LibTab,
    pub(crate) lib_sort: LibSort,
    pub(crate) lib_layout: LibLayout,
    /// Account library from the parent (hook); `None` until it arrives.
    pub(crate) account_library: Option<Arc<AccountLibrary>>,
    /// Shelves for Explore from the parent (hook).
    pub(crate) explore_shelves: Vec<Shelf>,
    /// Tools panel, shortcut sheet and "Create playlist" sheet.
    pub(crate) tools_open: bool,
    pub(crate) shortcuts_open: bool,
    pub(crate) create_open: bool,
    pub(crate) create_name: String,
    /// Sleep timer.
    pub(crate) sleep: Option<SleepState>,
    /// Playback speed in percent (100 is off).
    pub(crate) speed_percent: u16,
    /// Mini player and full-screen Now Playing.
    pub(crate) mini: bool,
    pub(crate) fullscreen: bool,
    /// Window size before the mini player.
    normal_size: Size,
    hooks: UiHooks,
    /// Lyrics provider switcher: every provider's answer for the playing song, best first.
    pub(crate) lyric_candidates: Vec<LyricsCandidate>,
    /// Index into `lyric_candidates` of the lyrics shown now.
    pub(crate) lyric_provider: usize,
    /// `true` while the user's own pick of source is stored for this song (else: best, auto).
    pub(crate) lyric_pinned: bool,
    /// Credit the source of the shown lyrics asks for (Unison).
    pub(crate) lyric_credit: Option<LyricsCredit>,
    /// Providers with their on/off state, for Settings.
    pub(crate) lyric_sources: Vec<LyricsProviderStatus>,
    /// A page whose songs should play as soon as it has loaded.
    pending_play: Option<DetailKey>,
    /// Daily sections: the data, the built sections and what they were built from.
    pub(crate) daily: DailyData,
    pub(crate) daily_sections: Vec<DailySection>,
    daily_requested: Option<bool>,
    daily_key: Option<(usize, bool, u64, usize, usize)>,
    daily_gen: usize,
    /// The local day (seed and greeting).
    pub(crate) day: DayContext,
}

impl App {
    /// Build the application; returns the jobs to start.
    #[must_use]
    pub fn new(backend: Backend, options: &UiOptions) -> (Self, Vec<Job>) {
        let ui = UiState::new(
            options.start_route.clone(),
            backend.player.snapshot(),
            options.reduced_motion,
        )
        .with_settings(options.settings.clone());
        let reduced = ui.reduced_motion;
        fx::set_reduced_motion(reduced);
        // Window sizes are small positive numbers.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let pane_open = layout::right_pushes(options.window_size.0 as u32);
        let mut app = Self {
            backend,
            ui,
            window: Size::new(options.window_size.0, options.window_size.1),
            home: HomeState::default(),
            local_albums: Vec::new(),
            local_tracks: Vec::new(),
            library_requested: false,
            library_loaded: false,
            details: HashMap::new(),
            art: Lru::new(ART_CAPACITY),
            wants: RefCell::new(Vec::new()),
            art_queue: VecDeque::new(),
            art_inflight: 0,
            cover: None,
            prev_cover: None,
            anim: Anim::new(reduced),
            pane_open,
            pane_user_set: false,
            np_tab: NpTab::default(),
            home_filter: HomeFilter::default(),
            lib_filter: None,
            account: AccountUi::default(),
            likes: HashMap::new(),
            last_line: None,
            prev_line: None,
            seek_drag: None,
            compact_tab: CompactTab::default(),
            settings_section: SettingsSection::default(),
            rows_limit: ROWS_STEP,
            spin: 0.0,
            shimmer: 0.0,
            eq_phase: 0.0,
            chip_pills: [
                (
                    crate::motion::Spring::tuned(0.0, 1400.0, 1.0),
                    crate::motion::Spring::tuned(0.0, 1400.0, 1.0),
                ),
                (
                    crate::motion::Spring::tuned(0.0, 1400.0, 1.0),
                    crate::motion::Spring::tuned(0.0, 1400.0, 1.0),
                ),
            ],
            chip_pills_init: false,
            queue_swap: (0, 0),
            queue_anim: crate::motion::Spring::tuned(0.0, 700.0, 1.0),
            frame_stats: FrameStats::default(),
            readout: options.show_frame_readout,
            music_folders: options.music_folders.clone(),
            on_settings: options.on_settings.clone(),
            on_add_music_folder: options.on_add_music_folder.clone(),
            focus_search: false,
            last_route: options.start_route.clone(),
            last_frame: None,
            scheme_fade: SchemeFade::settled(options.settings.base.neutrals()),
            bg_fade: BgFade::settled(bg_colors(
                options.settings.theme.seed(),
                options.settings.base,
            )),
            page_seeds: HashMap::new(),
            cover_seed: None,
            theme_key: None,
            content_filter: ContentFilter::default(),
            lib_tab: LibTab::default(),
            lib_sort: LibSort::default(),
            lib_layout: LibLayout::default(),
            account_library: None,
            explore_shelves: Vec::new(),
            tools_open: false,
            shortcuts_open: false,
            create_open: false,
            create_name: String::new(),
            sleep: None,
            speed_percent: 100,
            mini: false,
            fullscreen: false,
            normal_size: Size::new(options.window_size.0, options.window_size.1),
            hooks: options.hooks.clone(),
            lyric_candidates: Vec::new(),
            lyric_provider: 0,
            lyric_pinned: false,
            lyric_credit: None,
            lyric_sources: Vec::new(),
            pending_play: None,
            daily: DailyData::default(),
            daily_sections: Vec::new(),
            daily_requested: None,
            daily_key: None,
            daily_gen: 0,
            day: DayContext::from_unix(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs()),
                options.hooks.utc_offset_minutes,
            ),
        };
        color::set_active(app.scheme_fade.current());
        app.lyric_sources = app.backend.lyrics.providers();
        shape::set_scale_percent(options.settings.corner_style.percent());
        let mut jobs = Vec::new();
        // The reducer starts without a current track; feeding the snapshot requests cover and
        // lyrics.
        app.ui.snapshot = nemo_core::player::PlayerSnapshot::default();
        jobs.extend(app.dispatch(Event::Snapshot(app.backend.player.snapshot())));
        jobs.extend(app.dispatch(Event::Position(app.backend.player.position())));
        jobs.extend(app.sync_viewport());
        jobs.extend(app.after_update());
        (app, jobs)
    }
}
