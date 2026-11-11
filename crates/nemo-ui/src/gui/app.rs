//! The iced application: state, messages, update, subscriptions. The pure reducer in
//! [`crate::state`] decides what happens; this file runs its effects against the [`Backend`].
//!
//! Nothing here blocks: every catalog call, image decode and lookup is a [`Job`] that runs as a
//! task, and its answer comes back as a [`Message`].

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use iced::widget::image;
use iced::{Element, Size, Subscription, Task, keyboard, stream, window};
use nemo_core::account::{AccountError, AccountInfo, AccountState, SignOutReport};
use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, BrowseId, PlaylistDetail,
    PlaylistSummary, SearchResults, Shelf, ShelfItem,
};
use nemo_core::lyrics::{LyricsCandidate, LyricsCredit, LyricsQuery, ProviderId};
use nemo_core::player::{PlayerCommand, PlayerEvent};
use nemo_core::secret::Secret;
use nemo_core::services::{Backend, LyricsLookup, LyricsProviderStatus, PlayerControl};
use nemo_core::track::{Artwork, Track, TrackId};

use super::fx;
use super::style;
use super::views;
use crate::anim::Anim;
use crate::copy;
use crate::cover::{self, Baked, CoverData};
use crate::daily::{self, DailyData, DailySection, DayContext};
use crate::explore::{ContentFilter, LibLayout, LibSort, LibTab, MOODS, SleepChoice};
use crate::frame_stats::FrameStats;
use crate::home;
use crate::keys::{self, Key, Mods};
use crate::layout;
use crate::lru::{ART_CAPACITY, Lru};
use crate::palette::{Palette, Seed};
use crate::route::Route;
use crate::scheme::{
    self, AccentIntensity, BaseMode, BgFade, CornerStyle, SchemeFade, ThemeChoice, bg_colors,
};
use crate::search::DEBOUNCE_MS;
use crate::settings::UiSettings;
use crate::state::{Effect, Event, LyricsState, ResolveKind, UiState};
use crate::theme::{color, layout as theme_layout, shape};

/// Widget id of the search box.
pub const SEARCH_ID: &str = "nemo-search";
/// Most rows a list draws at once; "Show more" adds this many again.
pub const ROWS_STEP: usize = 200;
/// Images fetched at the same time.
const ART_PARALLEL: usize = 6;
/// Message of the search page while the first-run notice is not accepted.
pub const GATE_MESSAGE: &str = "YouTube Music is waiting for you to read the notice.";

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

/// Shape of a picture slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtShape {
    /// Card art, radius `shape.md`.
    Card,
    /// Row art, radius `shape.sm`.
    Row,
    /// Hero art, radius `shape.lg`.
    Hero,
    /// Artists: a circle.
    Circle,
    /// Large shelf card, radius 20.
    Tile,
    /// List thumbnail of 48 to 56 px, radius `shape.md`.
    Thumb,
}

/// Corner radius of the large shelf cards.
pub const TILE_RADIUS: f32 = 20.0;

impl ArtShape {
    /// Corner radius in px for a slot `size` px wide.
    #[must_use]
    pub fn radius(self, size: f32) -> f32 {
        match self {
            Self::Circle => size / 2.0,
            Self::Card | Self::Thumb => shape::md(),
            Self::Row => shape::sm(),
            Self::Hero => 24.0,
            Self::Tile => TILE_RADIUS,
        }
    }
}

/// One picture a view wants: where it comes from, the slot size and the shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtSpec {
    /// Source.
    pub artwork: Artwork,
    /// Slot size in logical px.
    pub size: u32,
    /// Corner shape.
    pub shape: ArtShape,
}

impl ArtSpec {
    /// Cache key: the source address plus slot and shape. `None` without artwork.
    #[must_use]
    pub fn key(&self) -> Option<String> {
        art_key(&self.artwork).map(|k| format!("{k}|{}|{:?}", self.size, self.shape))
    }

    /// Pixels to decode: 1.5 times the slot for sharp covers on high-DPI screens, at most 480.
    #[must_use]
    pub fn px(&self) -> u32 {
        (self.size * 3 / 2).clamp(48, 480)
    }

    /// Corner radius as a fraction of the width.
    #[must_use]
    pub fn fraction(&self) -> f32 {
        // Slot sizes are small integers, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let size = self.size.max(1) as f32;
        self.shape.radius(size) / size
    }
}

/// State of one decoded picture in the cache.
#[derive(Debug, Clone)]
pub enum ArtEntry {
    /// Requested, not yet here.
    Loading,
    /// Ready to draw (raw pixels, no decoding on the UI thread).
    Ready(image::Handle),
    /// There is no art (or it failed): draw the placeholder.
    Missing,
}

/// The current track's cover, analysed.
#[derive(Debug, Clone)]
pub struct CoverView {
    /// Track this cover belongs to.
    pub track: TrackId,
    /// Full cover with `shape.lg` corners (hero and Now Playing).
    pub art: image::Handle,
    /// Cover with `shape.md` corners for the 64 px slot in the bar.
    pub art_bar: image::Handle,
    /// Colours sampled left, middle, right from the blurred backdrop.
    pub strip: [crate::theme::Rgba; 3],
    /// Pre-blurred backdrop texture.
    pub backdrop: image::Handle,
    /// Glass tint alpha, or `None` for the tonal fallback.
    pub tint_alpha: Option<f32>,
    /// Scrim alpha over the Now Playing backdrop.
    pub scrim_alpha: f32,
}

/// Progress of the Home shelves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HomePhase {
    /// Not started (waiting for the first-run notice, or for the first update).
    #[default]
    Idle,
    /// Shelves are loading, one search at a time.
    Loading,
    /// All shelves are in.
    Done,
}

/// What Home shows besides the recent tracks and local albums.
#[derive(Debug, Clone, Default)]
pub struct HomeState {
    /// Shelves that have content, in order. A failed shelf is simply absent.
    pub shelves: Vec<Shelf>,
    /// Loading progress.
    pub phase: HomePhase,
}

/// Kind of a detail page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetailKind {
    /// Album.
    Album,
    /// Artist.
    Artist,
    /// Playlist.
    Playlist,
}

/// Key of a detail page.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DetailKey {
    /// Kind of page.
    pub kind: DetailKind,
    /// Catalog id.
    pub id: BrowseId,
}

impl DetailKey {
    /// The key of a route, if it is a detail page.
    #[must_use]
    pub fn of(route: &Route) -> Option<Self> {
        let (kind, id) = match route {
            Route::Album(id) => (DetailKind::Album, id),
            Route::Artist(id) => (DetailKind::Artist, id),
            Route::Playlist(id) => (DetailKind::Playlist, id),
            _ => return None,
        };
        Some(Self {
            kind,
            id: id.clone(),
        })
    }

    /// `true` for the id of a local album (no network needed).
    #[must_use]
    pub fn is_local(&self) -> bool {
        self.kind == DetailKind::Album && self.id.0.starts_with(home::LOCAL_ALBUM_PREFIX)
    }
}

/// A loaded detail page.
#[derive(Debug, Clone)]
pub enum DetailData {
    /// Album page.
    Album(AlbumDetail),
    /// Artist page.
    Artist(ArtistDetail),
    /// Playlist page.
    Playlist(PlaylistDetail),
}

/// State of a detail page.
#[derive(Debug, Clone)]
pub enum DetailState {
    /// Loading.
    Loading,
    /// Here.
    Ready(Arc<DetailData>),
    /// Failed (short message).
    Failed(String),
}

/// Where a list of tracks lives, so a row only carries an index and the app builds the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListSource {
    /// Songs of the search results.
    Search,
    /// Local tracks.
    Library,
    /// Recently played.
    Recent,
    /// Songs of Home shelf number `n`.
    Home(usize),
    /// Tracks of a detail page.
    Detail(DetailKey),
    /// Liked songs of the account.
    Account,
    /// Songs of daily section number `n`.
    Daily(usize),
    /// Music videos of the home feed.
    Videos,
}

/// Lower area of compact Now Playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompactTab {
    /// Three lyric lines.
    #[default]
    Lyrics,
    /// The queue.
    Queue,
}

/// Tabs of the right Now Playing pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NpTab {
    /// Synced lyrics.
    #[default]
    Lyrics,
    /// The queue.
    Queue,
    /// Details and the honest quality badge.
    Info,
}

impl NpTab {
    /// All tabs in order.
    pub const ALL: [Self; 3] = [Self::Lyrics, Self::Queue, Self::Info];

    /// Tab label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Lyrics => "Lyrics",
            Self::Queue => "Queue",
            Self::Info => "Info",
        }
    }
}

/// Filter chips on Home: they keep only shelf items of one kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HomeFilter {
    /// Everything.
    #[default]
    All,
    /// Songs only.
    Songs,
    /// Albums only.
    Albums,
    /// Artists only.
    Artists,
    /// Playlists only.
    Playlists,
}

impl HomeFilter {
    /// All chips in order.
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::Songs,
        Self::Albums,
        Self::Artists,
        Self::Playlists,
    ];

    /// Chip label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Songs => "Songs",
            Self::Albums => "Albums",
            Self::Artists => "Artists",
            Self::Playlists => "Playlists",
        }
    }

    /// `true` if a shelf item passes the filter.
    #[must_use]
    pub fn keeps(self, item: &ShelfItem) -> bool {
        matches!(
            (self, item),
            (Self::All, _)
                | (Self::Songs, ShelfItem::Song(_))
                | (Self::Albums, ShelfItem::Album(_))
                | (Self::Artists, ShelfItem::Artist(_))
                | (Self::Playlists, ShelfItem::Playlist(_))
        )
    }
}

/// Filter chips of the library list in the left pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibFilter {
    /// Playlists.
    Playlists,
    /// Artists.
    Artists,
    /// Albums.
    Albums,
}

impl LibFilter {
    /// All chips in order.
    pub const ALL: [Self; 3] = [Self::Playlists, Self::Artists, Self::Albums];

    /// Chip label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Playlists => "Playlists",
            Self::Artists => "Artists",
            Self::Albums => "Albums",
        }
    }
}

/// What the sign-in sheet remembers. The cookie lives only in a [`Secret`]: its `Debug` prints
/// `[redacted]`, it is never logged, and it is emptied when it is sent.
#[derive(Debug, Clone)]
pub struct AccountUi {
    /// The sheet is open.
    pub open: bool,
    /// The masked cookie field is shown.
    pub cookie_mode: bool,
    /// What was typed in the masked field.
    pub cookie: Secret<String>,
    /// A sign-in or sign-out call is running.
    pub busy: bool,
    /// Plain-words error of the last call.
    pub error: Option<String>,
    /// Sign out waits for the confirmation.
    pub confirm_sign_out: bool,
    /// Result line of the last sign-out.
    pub notice: Option<String>,
}

impl Default for AccountUi {
    fn default() -> Self {
        Self {
            open: false,
            cookie_mode: false,
            cookie: Secret::new(String::new()),
            busy: false,
            error: None,
            confirm_sign_out: false,
            notice: None,
        }
    }
}

/// Messages of the sign-in sheet.
#[derive(Debug, Clone)]
pub enum AccountMsg {
    /// Open the sheet.
    Open,
    /// Close the sheet.
    Close,
    /// Press "Sign in with Google".
    SignIn,
    /// Show or hide the masked cookie field.
    CookieMode(bool),
    /// The masked field changed.
    CookieInput(Secret<String>),
    /// Press "Check cookie".
    CookieSubmit,
    /// Press "Sign out" (asks first).
    AskSignOut,
    /// Leave the confirmation.
    CancelSignOut,
    /// Confirm sign out.
    SignOut,
    /// A sign-in call finished.
    SignedIn(Result<AccountInfo, AccountError>),
    /// A sign-out call finished.
    SignedOut(Result<SignOutReport, AccountError>),
}

/// Sections of the Settings screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsSection {
    /// SponsorBlock, ads, volume.
    #[default]
    Playback,
    /// Signal path.
    AudioOutput,
    /// Lyrics.
    Lyrics,
    /// Library folders.
    Library,
    /// Glass, motion, transparency.
    Appearance,
    /// Egress list.
    Privacy,
    /// Licences and credits.
    About,
}

impl SettingsSection {
    /// All sections in menu order.
    pub const ALL: [Self; 7] = [
        Self::Playback,
        Self::AudioOutput,
        Self::Lyrics,
        Self::Library,
        Self::Appearance,
        Self::Privacy,
        Self::About,
    ];

    /// Menu label and title.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Playback => "Playback",
            Self::AudioOutput => "Audio output",
            Self::Lyrics => "Lyrics",
            Self::Library => "Library",
            Self::Appearance => "Appearance",
            Self::Privacy => "Privacy",
            Self::About => "About",
        }
    }
}

/// Messages of the application.
#[derive(Debug, Clone)]
pub enum Message {
    /// A reducer event from a widget.
    Ui(Event),
    /// A frame was drawn.
    Frame(Instant),
    /// A key press.
    Key(Key, Mods),
    /// The window changed size.
    Resized(Size),
    /// Player event from the subscription.
    Player(PlayerEvent),
    /// The catalog's home feed answered.
    HomeFeed(Result<Arc<Vec<Shelf>>, String>),
    /// A seed search for Home answered.
    HomeSeed {
        /// Index into [`home::SEED_SHELVES`].
        index: usize,
        /// The answer.
        result: Result<Arc<SearchResults>, String>,
    },
    /// The local library listing answered.
    Library(Result<Arc<SearchResults>, String>),
    /// A search answered.
    SearchDone {
        /// The query that was run.
        query: String,
        /// The answer.
        result: Result<Arc<SearchResults>, String>,
    },
    /// A detail page answered.
    Detail {
        /// Page.
        key: DetailKey,
        /// The answer.
        result: Result<Arc<DetailData>, String>,
    },
    /// A picture arrived.
    Art {
        /// Cache key.
        key: String,
        /// Raw pixels.
        pixels: Result<Arc<Baked>, String>,
    },
    /// A cover was analysed.
    Cover {
        /// Track.
        track: TrackId,
        /// Result.
        built: Result<Arc<CoverBuilt>, String>,
    },
    /// Lyrics arrived.
    Lyrics {
        /// Track.
        track: TrackId,
        /// Lookup answer.
        lookup: Arc<LyricsLookup>,
        /// Stored offset.
        offset_ms: i64,
    },
    /// Every provider's lyrics for the song arrived (switcher).
    LyricsCandidates {
        /// Track.
        track: TrackId,
        /// Candidates, best first.
        list: Arc<Vec<LyricsCandidate>>,
    },
    /// The seek slider moved (0 to 1).
    SeekDrag(f32),
    /// The seek slider was released.
    SeekRelease,
    /// Play row `index` of a list as the queue.
    PlayAt {
        /// The list.
        source: ListSource,
        /// Row.
        index: usize,
    },
    /// Play a list shuffled.
    Shuffle(ListSource),
    /// Switch the compact Now Playing tab.
    CompactTab(CompactTab),
    /// Draw more rows of a long list.
    ShowMore,
    /// Pick a Settings section.
    SettingsSection(SettingsSection),
    /// Open Settings on this section.
    SettingsSectionGo(SettingsSection),
    /// Open or close the right Now Playing pane.
    TogglePane,
    /// Pick a tab of the right pane (opens the pane).
    NpTab(NpTab),
    /// Pick a Home filter chip.
    HomeFilter(HomeFilter),
    /// Pick or clear a library filter chip.
    LibFilter(Option<LibFilter>),
    /// Shuffle the rest of the queue.
    ShuffleQueue,
    /// Open Now Playing: the right pane when it fits, the full window otherwise.
    OpenNowPlaying,
    /// The sign-in sheet.
    Account(AccountMsg),
    /// Load a detail page again after an error.
    RetryDetail(DetailKey),
    /// Like or un-like a song (opens the sign-in sheet when nobody is signed in).
    ToggleLike(Track),
    /// A like call finished.
    LikeDone {
        /// Track.
        track: TrackId,
        /// What was asked for.
        liked: bool,
        /// The answer.
        result: Result<(), String>,
    },
    /// The user wants to add a music folder.
    AddFolder,
    /// Header content chip (All, Music, Videos, Podcasts).
    ContentFilter(ContentFilter),
    /// Library tab.
    LibTab(LibTab),
    /// Library sort order.
    LibSort(LibSort),
    /// Library list or grid.
    LibLayout(LibLayout),
    /// Play every song of a page (loads it first when needed).
    PlayCollection(DetailKey),
    /// A mood tile on Explore (index into [`MOODS`]).
    PickMood(usize),
    /// Hook: shelves for the Explore page from the catalog.
    ExploreLoaded(Arc<Vec<Shelf>>),
    /// Hook: the account library from the catalog.
    AccountLibrary(Arc<AccountLibrary>),
    /// Open or close the tools panel (sleep timer, speed, shortcuts, window modes).
    Tools(bool),
    /// Open or close the shortcut sheet.
    Shortcuts(bool),
    /// The "Create playlist" sheet.
    Create(CreateMsg),
    /// Start or stop the sleep timer.
    Sleep(Option<SleepChoice>),
    /// One second passed (sleep timer).
    TimerTick,
    /// Playback speed in percent.
    Speed(u16),
    /// Switch the mini player on or off.
    ToggleMini,
    /// Switch full-screen Now Playing on or off.
    ToggleFullScreen,
    /// Start a radio from a song (similar songs from search).
    StartRadio(Track),
    /// The radio answered.
    RadioReady {
        /// The seed song.
        seed: Track,
        /// Songs to follow it.
        tracks: Vec<Track>,
    },
    /// Copy the link of a song.
    CopyLink(Track),
    /// Move a queue entry after the current song up or down one place.
    QueueMove {
        /// Index in the queue.
        index: usize,
        /// `true` moves it earlier.
        up: bool,
    },
    /// Pick lyrics provider `i` in the switcher.
    PickLyricsProvider(usize),
    /// Switch a lyrics source on or off (Settings).
    SetLyricsSource(ProviderId, bool),
    /// The cover colour of a page arrived.
    PageSeed {
        /// Page.
        key: DetailKey,
        /// Seed.
        seed: Seed,
    },
    /// The account bundle arrived (library and daily data).
    AccountData(Arc<AccountBundle>),
    /// Open a search for this text (a mood tile).
    SearchFor(String),
    /// Nothing.
    Noop,
}

/// What one pass over the account calls of the catalog returned.
#[derive(Debug, Clone, Default)]
pub struct AccountBundle {
    /// The library sections.
    pub library: AccountLibrary,
    /// The inputs of the daily sections.
    pub daily: DailyData,
    /// Whether any account call answered.
    pub account_answered: bool,
}

/// A cover with its analysis, built off the UI thread.
#[derive(Debug)]
pub struct CoverBuilt {
    /// Cover with `shape.lg` corners.
    pub art: Baked,
    /// Cover with `shape.md` corners for the bar slot.
    pub bar: Baked,
    /// Palette and backdrop.
    pub data: CoverData,
}

/// Work that needs the backend; run as a task (or blocking, for snapshots).
#[derive(Debug, Clone)]
pub enum Job {
    /// Ask the catalog for a home feed.
    HomeFeed,
    /// Run seed search number `n` for Home.
    HomeSeed(usize),
    /// List the local library.
    Library,
    /// Run a search.
    Search(String),
    /// Wait out the search debounce.
    Debounce(u64),
    /// Load a detail page.
    Detail(DetailKey),
    /// Load one picture.
    Art(ArtSpec),
    /// Load and analyse a cover.
    Cover(Track),
    /// Look up lyrics.
    Lyrics(Track),
    /// Ask every lyrics provider (for the switcher).
    LyricsAll(Track),
    /// Open the sign-in window.
    SignIn,
    /// Sign in with a pasted cookie.
    SignInCookie(Secret<String>),
    /// Sign out.
    SignOut,
    /// Like or un-like a song.
    Rate {
        /// The song.
        track: Track,
        /// `true` to like.
        liked: bool,
    },
    /// Read the colour of a page's cover.
    PageSeed {
        /// Page.
        key: DetailKey,
        /// Its cover.
        artwork: Artwork,
    },
    /// Ask the catalog for the account library, history and explore data (one call after the
    /// other, at human pace).
    AccountData,
    /// Find songs like this one.
    Radio(Track),
    /// Find the album or artist page of a track.
    Resolve {
        /// What to open.
        kind: ResolveKind,
        /// The track.
        track: Track,
    },
}

/// Size of the mini player window.
pub const MINI_SIZE: (f32, f32) = (420.0, 168.0);

/// The link of a YouTube Music song.
fn link_of(video_id: &str) -> String {
    format!("https://music.youtube.com/watch?v={video_id}")
}

/// Cache key of an artwork (first thumbnail address or file path).
#[must_use]
pub fn art_key(artwork: &Artwork) -> Option<String> {
    match artwork {
        Artwork::None => None,
        Artwork::Remote(t) => t.first().map(|t| t.url.clone()),
        Artwork::LocalFile(p) => Some(p.display().to_string()),
    }
}

fn sleep_without_blocking(ms: u64) -> impl std::future::Future<Output = ()> {
    let (tx, rx) = iced::futures::channel::oneshot::channel::<()>();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(ms));
        let _ = tx.send(());
    });
    async move {
        let _ = rx.await;
    }
}

/// Run a job to completion.
pub async fn run_job(job: Job, backend: Backend) -> Message {
    match job {
        Job::HomeFeed => Message::HomeFeed(
            backend
                .catalog
                .home()
                .await
                .map(Arc::new)
                .map_err(|e| e.to_string()),
        ),
        Job::HomeSeed(index) => {
            let (_, query) = home::SEED_SHELVES[index.min(home::SEED_SHELVES.len() - 1)];
            Message::HomeSeed {
                index,
                result: backend
                    .catalog
                    .search(query)
                    .await
                    .map(Arc::new)
                    .map_err(|e| e.to_string()),
            }
        }
        Job::Library => Message::Library(
            backend
                .catalog
                .search("")
                .await
                .map(Arc::new)
                .map_err(|e| e.to_string()),
        ),
        Job::Search(query) => {
            let result = backend
                .catalog
                .search(&query)
                .await
                .map(Arc::new)
                .map_err(|e| e.to_string());
            Message::SearchDone { query, result }
        }
        Job::Debounce(seq) => {
            sleep_without_blocking(DEBOUNCE_MS).await;
            Message::Ui(Event::SearchDebounced(seq))
        }
        Job::Detail(key) => {
            let result = match key.kind {
                DetailKind::Album => backend.catalog.album(&key.id).await.map(DetailData::Album),
                DetailKind::Artist => backend
                    .catalog
                    .artist(&key.id)
                    .await
                    .map(DetailData::Artist),
                DetailKind::Playlist => backend
                    .catalog
                    .playlist(&key.id)
                    .await
                    .map(DetailData::Playlist),
            }
            .map(Arc::new)
            .map_err(|e| e.to_string());
            Message::Detail { key, result }
        }
        Job::Art(spec) => {
            let key = spec.key().unwrap_or_default();
            let pixels = match backend.images.image(&spec.artwork, spec.px()).await {
                Ok(raw) => cover::bake(&raw, spec.px(), spec.fraction())
                    .map(Arc::new)
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e.to_string()),
            };
            Message::Art { key, pixels }
        }
        Job::Cover(track) => {
            let built = match backend.images.image(&track.artwork, 640).await {
                Ok(raw) => cover::analyze(&raw)
                    .and_then(|data| {
                        let art = cover::bake(&raw, 480, 24.0 / 480.0)?;
                        let bar = cover::bake(&raw, 128, 12.0 / 48.0)?;
                        Ok(Arc::new(CoverBuilt { art, bar, data }))
                    })
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e.to_string()),
            };
            Message::Cover {
                track: track.id,
                built,
            }
        }
        Job::Lyrics(track) => {
            let lookup = backend
                .lyrics
                .lookup(&LyricsQuery::from_track(&track))
                .await;
            let offset_ms = backend.lyrics.offset_ms(&track.id.0);
            Message::Lyrics {
                track: track.id,
                lookup: Arc::new(lookup),
                offset_ms,
            }
        }
        Job::LyricsAll(track) => {
            let list = backend
                .lyrics
                .lookup_all(&LyricsQuery::from_track(&track))
                .await;
            Message::LyricsCandidates {
                track: track.id,
                list: Arc::new(list),
            }
        }
        Job::SignIn => Message::Account(AccountMsg::SignedIn(backend.account.sign_in().await)),
        Job::SignInCookie(cookie) => Message::Account(AccountMsg::SignedIn(
            backend.account.sign_in_with_cookie(cookie).await,
        )),
        Job::SignOut => Message::Account(AccountMsg::SignedOut(backend.account.sign_out().await)),
        Job::Rate { track, liked } => {
            let result = match &track.video_id {
                Some(video) => backend
                    .catalog
                    .rate_song(video, liked)
                    .await
                    .map_err(|e| e.to_string()),
                None => Err("not a YouTube Music song".to_owned()),
            };
            Message::LikeDone {
                track: track.id,
                liked,
                result,
            }
        }
        Job::PageSeed { key, artwork } => match backend.images.image(&artwork, 96).await {
            Ok(raw) => match ::image::load_from_memory(&raw) {
                Ok(img) => {
                    let small = img.thumbnail(64, 64).to_rgba8();
                    Message::PageSeed {
                        key,
                        seed: crate::palette::seed_from_pixels(small.as_raw()),
                    }
                }
                Err(_) => Message::Noop,
            },
            Err(_) => Message::Noop,
        },
        Job::AccountData => Message::AccountData(Arc::new(account_bundle(&backend).await)),
        Job::Radio(seed) => {
            let query = format!(
                "{} {}",
                seed.artists.first().cloned().unwrap_or_default(),
                seed.title
            );
            let tracks = backend
                .catalog
                .search(query.trim())
                .await
                .map(|r| r.songs)
                .unwrap_or_default();
            Message::RadioReady { seed, tracks }
        }
        Job::Resolve { kind, track } => Message::Ui(resolve(&backend, kind, &track).await),
    }
}

/// Call the account and explore parts of the catalog one after the other. Every call may fail
/// (signed out, offline, not built): a failure just leaves that part empty.
async fn account_bundle(backend: &Backend) -> AccountBundle {
    use nemo_core::catalog::FeedItem;
    let c = &backend.catalog;
    let mut b = AccountBundle::default();
    if let Ok(v) = c.library_playlists().await {
        b.library.playlists = v;
        b.account_answered = true;
    }
    if let Ok(v) = c.liked_songs().await {
        b.library.songs.clone_from(&v);
        b.daily.liked = v;
        b.account_answered = true;
    }
    if let Ok(v) = c.library_albums().await {
        b.library.albums = v;
    }
    if let Ok(v) = c.library_artists().await {
        b.library.artists.clone_from(&v);
        b.daily.followed = v;
    }
    if let Ok(sections) = c.history().await {
        b.daily.history = sections.into_iter().flat_map(|s| s.tracks).collect();
    }
    if let Ok(e) = c.explore().await {
        b.daily.new_releases = e.new_releases;
        b.daily.moods = e.moods;
    }
    if let Ok(feed) = c.home_feed().await {
        if b.daily.moods.is_empty() {
            b.daily.moods = feed.chips;
        }
        for shelf in &feed.shelves {
            for item in &shelf.items {
                match item {
                    FeedItem::Video(t) => b.daily.videos.push(t.clone()),
                    FeedItem::Podcast(p) => b.daily.podcasts.push(p.clone()),
                    _ => {}
                }
            }
        }
        b.daily.feed_shelves = feed.shelves;
    }
    b
}

/// Find the page of a track's artist or album; fall back to a search for the name.
async fn resolve(backend: &Backend, kind: ResolveKind, track: &Track) -> Event {
    let artist = track.artists.first().cloned().unwrap_or_default();
    let fallback = |q: String| Event::Go(Route::Search(q));
    match kind {
        ResolveKind::Artist => {
            if artist.is_empty() {
                return fallback(track.title.clone());
            }
            match backend.catalog.search(&artist).await {
                Ok(r) => r
                    .artists
                    .iter()
                    .find(|a| a.name.eq_ignore_ascii_case(&artist))
                    .or_else(|| r.artists.first())
                    .map_or_else(
                        || fallback(artist.clone()),
                        |a| Event::Go(Route::Artist(a.id.clone())),
                    ),
                Err(_) => fallback(artist),
            }
        }
        ResolveKind::Album => {
            let Some(album) = track.album.clone() else {
                return fallback(track.title.clone());
            };
            let query = format!("{album} {artist}").trim().to_owned();
            match backend.catalog.search(&query).await {
                Ok(r) => r
                    .albums
                    .iter()
                    .find(|a| a.title.eq_ignore_ascii_case(&album))
                    .or_else(|| r.albums.first())
                    .map_or_else(
                        || fallback(query.clone()),
                        |a| Event::Go(Route::Album(a.id.clone())),
                    ),
                Err(_) => fallback(query),
            }
        }
    }
}

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

    /// `true` when the parent wired playback speed (otherwise the chips are disabled).
    #[must_use]
    pub fn speed_wired(&self) -> bool {
        self.hooks.on_set_speed.is_some()
    }

    /// `true` when the app gave a callback for adding a music folder.
    #[must_use]
    pub fn has_add_folder(&self) -> bool {
        self.on_add_music_folder.is_some()
    }

    /// Window title.
    #[must_use]
    pub fn title(&self) -> String {
        match &self.ui.snapshot.current {
            Some(t) if self.ui.is_playing() => format!("{} \u{b7} NemoMusic", t.title),
            _ => "NemoMusic".to_owned(),
        }
    }

    /// The iced theme.
    #[must_use]
    pub fn theme(&self) -> iced::Theme {
        style::theme()
    }

    /// The accent palette to draw now (mid cross-fade it is a mix).
    #[must_use]
    pub fn accent(&self) -> Palette {
        self.ui.accent.current()
    }

    /// Current window size in logical px (rounded).
    #[must_use]
    pub fn window_px(&self) -> (u32, u32) {
        // Window sizes are small positive numbers.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        (
            self.window.width.max(0.0) as u32,
            self.window.height.max(0.0) as u32,
        )
    }

    /// Picture for a slot. Returns `None` while it loads (the caller draws the placeholder) and
    /// records the request, which the app starts after the next update.
    pub fn art_handle(
        &self,
        artwork: &Artwork,
        size: u32,
        shape: ArtShape,
    ) -> Option<image::Handle> {
        let spec = ArtSpec {
            artwork: artwork.clone(),
            size,
            shape,
        };
        let key = spec.key()?;
        match self.art.get(&key) {
            Some(ArtEntry::Ready(h)) => Some(h.clone()),
            Some(ArtEntry::Loading | ArtEntry::Missing) => None,
            None => {
                self.wants.borrow_mut().push(spec);
                None
            }
        }
    }

    /// `true` when nobody needs to sign in to like songs.
    #[must_use]
    pub fn signed_in(&self) -> bool {
        matches!(self.backend.account.state(), AccountState::SignedIn(_))
    }

    /// `true` when the song is liked: what the user did this session, else the Liked songs shelf.
    #[must_use]
    pub fn is_liked(&self, track: &Track) -> bool {
        self.likes.get(&track.id).copied().unwrap_or_else(|| {
            self.home
                .shelves
                .iter()
                .filter(|s| s.title == "Liked songs")
                .flat_map(|s| s.items.iter())
                .any(|i| matches!(i, ShelfItem::Song(t) if t.id == track.id))
        })
    }

    /// The tracks of a list.
    #[must_use]
    pub fn tracks_of(&self, source: &ListSource) -> Vec<Track> {
        match source {
            ListSource::Search => self
                .ui
                .search
                .results
                .as_ref()
                .map(|r| r.songs.clone())
                .unwrap_or_default(),
            ListSource::Library => self.local_tracks.clone(),
            ListSource::Daily(n) => self
                .daily_sections
                .get(*n)
                .map(|s| s.tracks.clone())
                .unwrap_or_default(),
            ListSource::Videos => self.daily.videos.clone(),
            ListSource::Account => match &self.account_library {
                Some(lib) => lib.songs.clone(),
                None => self
                    .home
                    .shelves
                    .iter()
                    .filter(|s| s.title == "Liked songs")
                    .flat_map(|s| s.items.iter())
                    .filter_map(|i| match i {
                        ShelfItem::Song(t) => Some(t.clone()),
                        _ => None,
                    })
                    .collect(),
            },
            ListSource::Recent => self.ui.recent.clone(),
            ListSource::Home(n) => self
                .home
                .shelves
                .get(*n)
                .map(|s| {
                    s.items
                        .iter()
                        .filter_map(|i| match i {
                            ShelfItem::Song(t) => Some(t.clone()),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            ListSource::Detail(key) => match self.details.get(key) {
                Some(DetailState::Ready(d)) => match d.as_ref() {
                    DetailData::Album(a) => a.tracks.clone(),
                    DetailData::Artist(a) => a.top_songs.clone(),
                    DetailData::Playlist(p) => p.tracks.clone(),
                },
                _ => Vec::new(),
            },
        }
    }

    fn sync_viewport(&mut self) -> Vec<Job> {
        let (w, h) = self.window_px();
        let full = matches!(self.ui.route(), Route::NowPlaying);
        let v = if full {
            views::now_playing::lyrics_viewport_height(w, h)
        } else {
            layout::pane_lyrics_height(w, h)
        };
        // The switcher and the credit line take room under the lyrics.
        let v = (v - views::now_playing::footer_extra(self)).max(60.0);
        let mut jobs = self.dispatch(Event::LyricsDense(!full));
        jobs.extend(self.dispatch(Event::LyricsViewport(v)));
        jobs
    }

    /// `true` while a sign-in or sign-out call runs (here or in the service).
    #[must_use]
    pub fn account_busy(&self) -> bool {
        self.account.busy || matches!(self.backend.account.state(), AccountState::SigningIn)
    }

    /// `true` while the page of the route is still loading (a skeleton is drawn).
    #[must_use]
    pub fn detail_loading(&self) -> bool {
        DetailKey::of(self.ui.route()).is_some_and(|k| {
            matches!(self.details.get(&k), None | Some(DetailState::Loading))
                && (self.ui.youtube_enabled() || k.is_local())
        })
    }

    /// `true` while anything animates; the window subscribes to frames only then.
    #[must_use]
    pub fn needs_frames(&self) -> bool {
        self.ui.needs_frames()
            || self.anim.active()
            || self.account_busy()
            || !self.scheme_fade.is_done()
            || !self.bg_fade.is_done()
            || self.detail_loading()
            || self
                .chip_pills
                .iter()
                .any(|(x, w)| !x.is_settled() || !w.is_settled())
            || !self.queue_anim.is_settled()
    }

    /// Point every animation at the state it should show. Cheap; runs after every update.
    /// Apply the theme settings when they changed: corner scale at once, surfaces and accent as
    /// one animated cross-fade.
    fn sync_theme(&mut self) {
        let s = &self.ui.settings;
        let key = (s.theme, s.base, s.accent_intensity, s.corner_style);
        if self.theme_key == Some(key) {
            return;
        }
        let first = self.theme_key.is_none();
        self.theme_key = Some(key);
        let (theme, base, intensity, corner) = key;
        shape::set_scale_percent(corner.percent());
        let reduced = self.ui.reduced_motion || first;
        self.scheme_fade.retarget(base.neutrals(), reduced);
        color::set_active(self.scheme_fade.current());
        let palette = scheme::accent_palette(theme, base, intensity, self.cover_seed);
        self.ui.accent.retarget(palette, reduced);
    }

    /// Point the background field at the colour it should have now: a fixed theme keeps its own
    /// seed; Dynamic uses the page being viewed (album or playlist) and otherwise the song.
    fn sync_bg(&mut self) {
        let s = &self.ui.settings;
        let seed = s.theme.seed().or_else(|| {
            DetailKey::of(self.ui.route())
                .and_then(|k| self.page_seeds.get(&k).copied())
                .or(self.cover_seed)
        });
        let colors = bg_colors(seed, s.base);
        let reduced = self.ui.reduced_motion;
        self.bg_fade.retarget(colors, reduced);
    }

    fn sync_chips(&mut self) {
        use super::views::kit::chip_geometry;
        let home: Vec<&str> = ContentFilter::ALL.iter().map(|f| f.label()).collect();
        let home_sel = ContentFilter::ALL
            .iter()
            .position(|f| *f == self.content_filter)
            .unwrap_or(0);
        let lib: Vec<&str> = LibTab::ALL.iter().map(|t| t.label()).collect();
        let lib_sel = LibTab::ALL
            .iter()
            .position(|t| *t == self.lib_tab)
            .unwrap_or(0);
        for (slot, (labels, sel)) in [(home, home_sel), (lib, lib_sel)].into_iter().enumerate() {
            let (x, w) = chip_geometry(&labels, sel);
            let (sx, sw) = &mut self.chip_pills[slot];
            sx.target = x;
            sw.target = w;
            if !self.chip_pills_init || self.ui.reduced_motion {
                sx.snap(x);
                sw.snap(w);
            }
        }
        self.chip_pills_init = true;
    }

    fn sync_anim(&mut self) {
        self.sync_chips();
        // The queue flag and the pane tab stay one fact.
        if self.ui.queue_open && !(self.pane_open && self.np_tab == NpTab::Queue) {
            self.pane_open = true;
            self.np_tab = NpTab::Queue;
        } else if !self.ui.queue_open && self.np_tab == NpTab::Queue {
            self.np_tab = NpTab::Lyrics;
        }
        let reduced = self.ui.reduced_motion;
        fx::set_reduced_motion(reduced);
        fx::set_focus_color(style::col(self.ui.accent.current().primary));
        self.anim.set_reduced(reduced);
        let unit = |on: bool| if on { 1.0 } else { 0.0 };
        self.anim.pane.target = unit(self.pane_open);
        self.anim.capsule.target = unit(self.ui.snapshot.current.is_some());
        self.anim.play.target = unit(self.ui.is_playing() && !self.ui.is_buffering());
        self.anim.sheet.target = unit(self.account.open);
        if !self.account.open {
            self.anim.sheet.snap(0.0);
        }
        let line = self.ui.active_lyric();
        if line != self.last_line {
            self.last_line = line;
            self.anim.restart_line();
        }
    }

    fn set_tab(&mut self, tab: NpTab) {
        self.pane_open = true;
        self.pane_user_set = true;
        self.np_tab = tab;
        self.ui.queue_open = tab == NpTab::Queue;
    }

    fn account_msg(&mut self, msg: AccountMsg) -> Vec<Job> {
        let acc = &mut self.account;
        match msg {
            AccountMsg::Open => {
                acc.open = true;
                acc.error = None;
                acc.confirm_sign_out = false;
                acc.cookie_mode = false;
                self.anim.sheet.snap(0.0);
            }
            AccountMsg::Close => {
                acc.open = false;
                acc.cookie = Secret::new(String::new());
                acc.cookie_mode = false;
                acc.notice = None;
                acc.confirm_sign_out = false;
                self.ui.text_focused = false;
            }
            AccountMsg::SignIn => {
                if self.ui.settings.tos_acknowledged && !acc.busy {
                    acc.busy = true;
                    acc.error = None;
                    acc.notice = None;
                    return vec![Job::SignIn];
                }
            }
            AccountMsg::CookieMode(on) => {
                acc.cookie_mode = on;
                acc.error = None;
                if !on {
                    acc.cookie = Secret::new(String::new());
                }
                self.ui.text_focused = on;
            }
            AccountMsg::CookieInput(text) => acc.cookie = text,
            AccountMsg::CookieSubmit => {
                if self.ui.settings.tos_acknowledged
                    && !acc.busy
                    && !acc.cookie.expose().trim().is_empty()
                {
                    acc.busy = true;
                    acc.error = None;
                    // The cookie leaves the UI state here; only the service keeps it.
                    let cookie = std::mem::replace(&mut acc.cookie, Secret::new(String::new()));
                    return vec![Job::SignInCookie(cookie)];
                }
            }
            AccountMsg::AskSignOut => acc.confirm_sign_out = true,
            AccountMsg::CancelSignOut => acc.confirm_sign_out = false,
            AccountMsg::SignOut => {
                acc.busy = true;
                acc.confirm_sign_out = false;
                return vec![Job::SignOut];
            }
            AccountMsg::SignedIn(result) => {
                acc.busy = false;
                match result {
                    Ok(_) => {
                        acc.error = None;
                        acc.cookie_mode = false;
                        acc.notice = None;
                        self.ui.text_focused = false;
                    }
                    Err(e) => {
                        acc.error = Some(copy::account_error(&e));
                        if matches!(e, AccountError::Blocked | AccountError::Unavailable(_)) {
                            acc.cookie_mode = true;
                            self.ui.text_focused = true;
                        }
                    }
                }
            }
            AccountMsg::SignedOut(result) => {
                acc.busy = false;
                match result {
                    Ok(report) => {
                        acc.error = None;
                        acc.notice = Some(copy::sign_out_result(&report));
                    }
                    Err(e) => acc.error = Some(copy::account_error(&e)),
                }
            }
        }
        Vec::new()
    }

    /// Shuffle what plays after the current track without interrupting it: every other entry is
    /// removed and added back in random order.
    fn shuffle_queue(&mut self) -> Vec<Job> {
        let snap = self.ui.snapshot.clone();
        let Some(current) = snap.queue_index else {
            return Vec::new();
        };
        let mut jobs = Vec::new();
        let mut others: Vec<Track> = snap
            .queue
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != current)
            .map(|(_, t)| t.clone())
            .collect();
        if others.len() < 2 {
            return jobs;
        }
        shuffle(&mut others);
        for i in (0..snap.queue.len()).rev() {
            if i != current {
                jobs.extend(self.dispatch(Event::Command(PlayerCommand::RemoveAt(i))));
            }
        }
        for t in others {
            jobs.extend(self.dispatch(Event::Command(PlayerCommand::Enqueue(t))));
        }
        jobs
    }

    /// Run an event through the reducer and its effects; returns jobs to start.
    pub fn dispatch(&mut self, event: Event) -> Vec<Job> {
        let mut jobs = Vec::new();
        let mut pending = VecDeque::from([event]);
        while let Some(ev) = pending.pop_front() {
            for fx in self.ui.apply(ev) {
                match fx {
                    Effect::Send(cmd) => {
                        self.backend.player.send(cmd);
                        pending.push_back(Event::Snapshot(self.backend.player.snapshot()));
                        pending.push_back(Event::Position(self.backend.player.position()));
                    }
                    Effect::RefreshSnapshot => {
                        pending.push_back(Event::Snapshot(self.backend.player.snapshot()));
                        pending.push_back(Event::Position(self.backend.player.position()));
                    }
                    Effect::LoadCover(t) => jobs.push(Job::Cover(t)),
                    Effect::LoadLyrics(t) => jobs.push(Job::Lyrics(t)),
                    Effect::SaveLyricsOffset {
                        track_key,
                        offset_ms,
                    } => self.backend.lyrics.set_offset_ms(&track_key, offset_ms),
                    Effect::ToggleFrameReadout => self.readout = !self.readout,
                    Effect::Debounce(seq) => jobs.push(Job::Debounce(seq)),
                    Effect::RunSearch(query) => {
                        if self.ui.youtube_enabled() {
                            jobs.push(Job::Search(query));
                        } else {
                            // Nothing leaves the computer before the notice is accepted.
                            pending.push_back(Event::SearchDone {
                                query,
                                result: Err(GATE_MESSAGE.to_owned()),
                            });
                        }
                    }
                    Effect::FocusSearch => self.focus_search = true,
                    Effect::SettingsChanged(settings) => {
                        if let Some(cb) = &self.on_settings {
                            cb(settings);
                        }
                    }
                    Effect::Resolve { kind, track } => {
                        if self.ui.youtube_enabled()
                            || track.origin == nemo_core::audio::Origin::Local
                        {
                            jobs.push(Job::Resolve { kind, track });
                        }
                    }
                }
            }
        }
        jobs
    }

    /// Start whatever the current state asks for and has not asked for yet: the library listing,
    /// Home shelves (after the notice), the detail page of the route, and pictures in view.
    fn after_update(&mut self) -> Vec<Job> {
        let mut jobs = Vec::new();
        if *self.ui.route() != self.last_route {
            self.last_route = self.ui.route().clone();
            self.rows_limit = ROWS_STEP;
            self.anim.restart_page();
            jobs.extend(self.sync_viewport());
        }
        self.sync_theme();
        self.refresh_daily();
        self.sync_bg();
        jobs.extend(self.check_sleep());
        self.sync_anim();
        jobs.extend(self.pump_loads());
        self.probe_art();
        jobs.extend(self.pump_art());
        jobs
    }

    fn pump_loads(&mut self) -> Vec<Job> {
        let mut jobs = Vec::new();
        if !self.library_requested {
            self.library_requested = true;
            jobs.push(Job::Library);
        }
        if self.ui.youtube_enabled() && self.daily_requested != Some(self.signed_in()) {
            self.daily_requested = Some(self.signed_in());
            jobs.push(Job::AccountData);
        }
        if self.ui.youtube_enabled() && self.home.phase == HomePhase::Idle {
            self.home.phase = HomePhase::Loading;
            jobs.push(Job::HomeFeed);
        }
        if let Some(key) = DetailKey::of(self.ui.route())
            && !self.details.contains_key(&key)
            && (self.ui.youtube_enabled() || key.is_local())
        {
            self.details.insert(key.clone(), DetailState::Loading);
            jobs.push(Job::Detail(key));
        }
        jobs
    }

    /// Build the view once, only to learn which pictures it asks for.
    fn probe_art(&mut self) {
        self.wants.borrow_mut().clear();
        drop(views::root(self));
        let wants = std::mem::take(&mut *self.wants.borrow_mut());
        self.art_queue.clear();
        let mut seen: Vec<String> = Vec::new();
        for spec in wants {
            if let Some(key) = spec.key()
                && !seen.contains(&key)
                && !self.art.contains(&key)
            {
                seen.push(key);
                self.art_queue.push_back(spec);
            }
        }
    }

    fn pump_art(&mut self) -> Vec<Job> {
        let mut jobs = Vec::new();
        while self.art_inflight < ART_PARALLEL {
            let Some(spec) = self.art_queue.pop_front() else {
                break;
            };
            let Some(key) = spec.key() else { continue };
            if self.art.contains(&key) {
                continue;
            }
            self.art
                .insert(key, ArtEntry::Loading, |e| matches!(e, ArtEntry::Loading));
            self.art_inflight += 1;
            jobs.push(Job::Art(spec));
        }
        jobs
    }

    /// Handle a message; returns jobs to start. Shared by the window and the headless runner.
    pub fn update_core(&mut self, message: Message) -> Vec<Job> {
        let probe = !matches!(
            message,
            Message::Frame(_) | Message::Art { .. } | Message::SeekDrag(_) | Message::Noop
        );
        let art_arrived = matches!(message, Message::Art { .. });
        let mut jobs = self.handle(message);
        if probe {
            jobs.extend(self.after_update());
        } else if art_arrived {
            jobs.extend(self.pump_art());
        }
        jobs
    }

    fn handle(&mut self, message: Message) -> Vec<Job> {
        match message {
            Message::Ui(ev) => self.dispatch(ev),
            Message::Player(ev) => self.dispatch(Event::Player(ev)),
            Message::Frame(now) => self.on_frame(now),
            Message::Key(key, mods) => match keys::map(key, mods, self.ui.text_focused) {
                // Esc closes the sign-in sheet first.
                Some(keys::KeyAction::Close) if self.account.open => {
                    self.account_msg(AccountMsg::Close)
                }
                Some(keys::KeyAction::Close) if self.create_open => {
                    self.create_msg(CreateMsg::Close)
                }
                Some(keys::KeyAction::Close) if self.shortcuts_open || self.tools_open => {
                    self.shortcuts_open = false;
                    self.tools_open = false;
                    Vec::new()
                }
                Some(keys::KeyAction::Close) if self.mini => self.handle(Message::ToggleMini),
                Some(keys::KeyAction::Close) if self.fullscreen => {
                    self.handle(Message::ToggleFullScreen)
                }
                Some(keys::KeyAction::ShowShortcuts) => {
                    self.handle(Message::Shortcuts(!self.shortcuts_open))
                }
                Some(keys::KeyAction::FullScreen) => self.handle(Message::ToggleFullScreen),
                Some(keys::KeyAction::Mini) => self.handle(Message::ToggleMini),
                Some(keys::KeyAction::Tools) => self.handle(Message::Tools(!self.tools_open)),
                Some(keys::KeyAction::LikeCurrent) => match self.ui.snapshot.current.clone() {
                    Some(t) => self.handle(Message::ToggleLike(t)),
                    None => Vec::new(),
                },
                Some(action) => self.dispatch(Event::Key(action)),
                None => Vec::new(),
            },
            Message::Resized(size) => {
                self.window = size;
                if !self.pane_user_set {
                    let (w, _) = self.window_px();
                    self.pane_open = layout::right_pushes(w);
                }
                self.sync_viewport()
            }
            Message::TogglePane => {
                self.pane_open = !self.pane_open;
                self.pane_user_set = true;
                if !self.pane_open {
                    self.ui.queue_open = false;
                }
                Vec::new()
            }
            Message::NpTab(tab) => {
                self.set_tab(tab);
                Vec::new()
            }
            Message::HomeFilter(filter) => {
                self.home_filter = filter;
                Vec::new()
            }
            Message::LibFilter(filter) => {
                self.lib_filter = filter;
                Vec::new()
            }
            Message::ShuffleQueue => self.shuffle_queue(),
            Message::OpenNowPlaying => {
                let (w, _) = self.window_px();
                if layout::right_pushes(w) {
                    self.pane_open = !self.pane_open || self.np_tab != NpTab::Lyrics;
                    self.pane_user_set = true;
                    self.np_tab = NpTab::Lyrics;
                    self.ui.queue_open = false;
                    Vec::new()
                } else {
                    self.dispatch(Event::Go(Route::NowPlaying))
                }
            }
            Message::Account(msg) => self.account_msg(msg),
            Message::HomeFeed(result) => match result {
                Ok(shelves) if !shelves.is_empty() => {
                    self.home.shelves.clone_from(shelves.as_ref());
                    self.home.phase = HomePhase::Done;
                    Vec::new()
                }
                // No feed (the real backend): build Home from calm seed searches.
                Ok(_) | Err(_) => vec![Job::HomeSeed(0)],
            },
            Message::HomeSeed { index, result } => {
                if let Ok(r) = result {
                    self.learn_local(&r);
                    let title = home::SEED_SHELVES[index.min(home::SEED_SHELVES.len() - 1)].0;
                    if let Some(shelf) = home::shelf_from_search(title, r.as_ref().clone()) {
                        self.home.shelves.push(shelf);
                    }
                }
                let next = index + 1;
                if next < home::SEED_SHELVES.len() {
                    vec![Job::HomeSeed(next)]
                } else {
                    self.home.phase = HomePhase::Done;
                    Vec::new()
                }
            }
            Message::Library(result) => {
                self.library_loaded = true;
                if let Ok(r) = result {
                    self.learn_local(&r);
                }
                Vec::new()
            }
            Message::SearchDone { query, result } => {
                if let Ok(r) = &result {
                    self.learn_local(r);
                }
                self.dispatch(Event::SearchDone { query, result })
            }
            Message::Detail { key, result } => {
                let state = match result {
                    Ok(d) => DetailState::Ready(d),
                    Err(e) => DetailState::Failed(e),
                };
                let ready = matches!(state, DetailState::Ready(_));
                let mut jobs = Vec::new();
                if let DetailState::Ready(d) = &state
                    && !self.page_seeds.contains_key(&key)
                {
                    let artwork = match d.as_ref() {
                        DetailData::Album(a) => Artwork::Remote(a.summary.thumbnails.clone()),
                        DetailData::Playlist(p) => Artwork::Remote(p.summary.thumbnails.clone()),
                        DetailData::Artist(a) => Artwork::Remote(a.summary.thumbnails.clone()),
                    };
                    jobs.push(Job::PageSeed {
                        key: key.clone(),
                        artwork,
                    });
                }
                self.details.insert(key.clone(), state);
                if self.pending_play.as_ref() == Some(&key) {
                    self.pending_play = None;
                    if ready {
                        let tracks = self.tracks_of(&ListSource::Detail(key));
                        jobs.extend(self.dispatch(Event::PlayTracks { tracks, start: 0 }));
                    }
                }
                jobs
            }
            Message::Art { key, pixels } => {
                self.art_inflight = self.art_inflight.saturating_sub(1);
                let entry = match pixels {
                    Ok(b) => {
                        ArtEntry::Ready(image::Handle::from_rgba(b.width, b.height, b.rgba.clone()))
                    }
                    Err(_) => ArtEntry::Missing,
                };
                self.art
                    .insert(key, entry, |e| matches!(e, ArtEntry::Loading));
                Vec::new()
            }
            Message::Cover { track, built } => self.on_cover(track, built),
            Message::Lyrics {
                track,
                lookup,
                offset_ms,
            } => {
                self.lyric_candidates.clear();
                self.lyric_provider = 0;
                self.lyric_credit = lookup
                    .doc
                    .as_ref()
                    .and_then(|d| self.backend.lyrics.credit(&track.0, d.provider));
                let current = self.ui.snapshot.current.clone();
                let mut jobs = self.dispatch(Event::LyricsReady {
                    track: track.clone(),
                    lookup: lookup.as_ref().clone(),
                    offset_ms,
                });
                if let Some(t) = current.filter(|t| t.id == track) {
                    jobs.push(Job::LyricsAll(t));
                }
                jobs.extend(self.sync_viewport());
                jobs
            }
            Message::LyricsCandidates { track, list } => {
                let current = self.ui.snapshot.current.as_ref().map(|t| &t.id);
                if current == Some(&track) {
                    let shown = match &self.ui.lyrics {
                        LyricsState::Ready(d) => Some(d.provider),
                        _ => None,
                    };
                    self.lyric_provider = shown
                        .and_then(|p| list.iter().position(|c| c.provider == p))
                        .unwrap_or(0);
                    self.lyric_candidates.clone_from(list.as_ref());
                }
                self.sync_viewport()
            }
            Message::SeekDrag(f) => {
                self.seek_drag = Some(f);
                Vec::new()
            }
            Message::SeekRelease => match self.seek_drag.take() {
                Some(f) => self.dispatch(Event::SeekFraction(f)),
                None => Vec::new(),
            },
            Message::PlayAt { source, index } => {
                let tracks = self.tracks_of(&source);
                self.dispatch(Event::PlayTracks {
                    tracks,
                    start: index,
                })
            }
            Message::Shuffle(source) => {
                let mut tracks = self.tracks_of(&source);
                shuffle(&mut tracks);
                self.dispatch(Event::PlayTracks { tracks, start: 0 })
            }
            Message::CompactTab(tab) => {
                self.compact_tab = tab;
                Vec::new()
            }
            Message::ShowMore => {
                self.rows_limit += ROWS_STEP;
                Vec::new()
            }
            Message::SettingsSection(s) => {
                self.settings_section = s;
                Vec::new()
            }
            Message::SettingsSectionGo(s) => {
                self.settings_section = s;
                self.dispatch(Event::Go(Route::Settings))
            }
            Message::RetryDetail(key) => {
                self.details.remove(&key);
                Vec::new()
            }
            Message::ToggleLike(track) => {
                self.ui.menu = None;
                if !self.signed_in() {
                    return self.account_msg(AccountMsg::Open);
                }
                if track.video_id.is_none() {
                    return Vec::new();
                }
                let liked = !self.is_liked(&track);
                self.likes.insert(track.id.clone(), liked);
                vec![Job::Rate { track, liked }]
            }
            Message::LikeDone {
                track,
                liked,
                result,
            } => {
                if result.is_err() {
                    self.likes.insert(track, !liked);
                    return self.dispatch(Event::Notice(
                        "Could not update your like. Try again in a moment.".to_owned(),
                    ));
                }
                Vec::new()
            }
            Message::AddFolder => {
                if let Some(cb) = &self.on_add_music_folder {
                    cb();
                }
                Vec::new()
            }
            Message::ContentFilter(f) => {
                self.content_filter = f;
                Vec::new()
            }
            Message::LibTab(tab) => {
                self.lib_tab = tab;
                self.rows_limit = ROWS_STEP;
                Vec::new()
            }
            Message::LibSort(s) => {
                self.lib_sort = s;
                Vec::new()
            }
            Message::LibLayout(l) => {
                self.lib_layout = l;
                Vec::new()
            }
            Message::PlayCollection(key) => {
                if let Some(DetailState::Ready(_)) = self.details.get(&key) {
                    let tracks = self.tracks_of(&ListSource::Detail(key));
                    return self.dispatch(Event::PlayTracks { tracks, start: 0 });
                }
                self.pending_play = Some(key.clone());
                if self.details.contains_key(&key) {
                    return Vec::new();
                }
                self.details.insert(key.clone(), DetailState::Loading);
                vec![Job::Detail(key)]
            }
            Message::PickMood(i) => {
                let mood = MOODS[i.min(MOODS.len() - 1)];
                self.dispatch(Event::Go(Route::Search(mood.query.to_owned())))
            }
            Message::ExploreLoaded(shelves) => {
                self.explore_shelves.clone_from(shelves.as_ref());
                Vec::new()
            }
            Message::AccountLibrary(lib) => {
                self.account_library = Some(lib);
                Vec::new()
            }
            Message::Tools(on) => {
                self.tools_open = on;
                self.shortcuts_open = false;
                Vec::new()
            }
            Message::Shortcuts(on) => {
                self.shortcuts_open = on;
                self.tools_open = false;
                Vec::new()
            }
            Message::Create(msg) => self.create_msg(msg),
            Message::Sleep(choice) => {
                self.sleep = choice.map(|choice| SleepState {
                    choice,
                    deadline: choice.duration().map(|d| Instant::now() + d),
                    track: self.ui.snapshot.current.as_ref().map(|t| t.id.clone()),
                });
                let text = match choice {
                    Some(c) => format!("Sleep timer set: {}", c.label()),
                    None => "Sleep timer off".to_owned(),
                };
                self.dispatch(Event::Notice(text))
            }
            Message::Speed(p) => {
                if let Some(cb) = &self.hooks.on_set_speed {
                    self.speed_percent = p;
                    cb(p);
                }
                Vec::new()
            }
            Message::ToggleMini => {
                self.mini = !self.mini;
                self.fullscreen = false;
                self.tools_open = false;
                Vec::new()
            }
            Message::ToggleFullScreen => {
                self.fullscreen = !self.fullscreen;
                self.mini = false;
                self.tools_open = false;
                if self.fullscreen {
                    self.dispatch(Event::Go(Route::NowPlaying))
                } else {
                    self.dispatch(Event::Back)
                }
            }
            Message::StartRadio(track) => {
                self.ui.menu = None;
                if track.origin == nemo_core::audio::Origin::YoutubeMusic
                    && !self.ui.youtube_enabled()
                {
                    return Vec::new();
                }
                vec![Job::Radio(track)]
            }
            Message::RadioReady { seed, tracks } => {
                let mut queue = vec![seed.clone()];
                queue.extend(tracks.into_iter().filter(|t| t.id != seed.id).take(24));
                let n = queue.len();
                let mut jobs = self.dispatch(Event::PlayTracks {
                    tracks: queue,
                    start: 0,
                });
                jobs.extend(self.dispatch(Event::Notice(format!(
                    "Radio from \u{201c}{}\u{201d}: {n} songs from search results",
                    seed.title
                ))));
                jobs
            }
            Message::CopyLink(_) => {
                // The clipboard write is a task; `update` runs it.
                self.ui.menu = None;
                Vec::new()
            }
            Message::QueueMove { index, up } => self.queue_move(index, up),
            Message::PickLyricsProvider(i) => {
                let Some(track) = self.ui.snapshot.current.clone() else {
                    return Vec::new();
                };
                let Some(cand) = self.lyric_candidates.get(i).cloned() else {
                    return Vec::new();
                };
                self.lyric_provider = i;
                self.backend.lyrics.choose(&track.id.0, cand.provider);
                self.lyric_credit = cand
                    .credit
                    .clone()
                    .or_else(|| self.backend.lyrics.credit(&track.id.0, cand.provider));
                let offset_ms = self.ui.lyrics_offset_ms;
                let mut jobs = self.dispatch(Event::LyricsReady {
                    track: track.id,
                    lookup: LyricsLookup {
                        doc: Some(cand.doc),
                        tried: Vec::new(),
                    },
                    offset_ms,
                });
                jobs.extend(self.sync_viewport());
                jobs
            }
            Message::SetLyricsSource(id, on) => {
                self.backend.lyrics.set_provider_enabled(id, on);
                self.lyric_sources = self.backend.lyrics.providers();
                Vec::new()
            }
            Message::PageSeed { key, seed } => {
                self.page_seeds.insert(key, seed);
                Vec::new()
            }
            Message::AccountData(b) => {
                self.daily = b.daily.clone();
                self.daily_gen += 1;
                let lib = &b.library;
                let any = !(lib.playlists.is_empty()
                    && lib.albums.is_empty()
                    && lib.artists.is_empty()
                    && lib.songs.is_empty());
                if self.signed_in() && any {
                    self.account_library = Some(Arc::new(b.library.clone()));
                }
                Vec::new()
            }
            Message::SearchFor(q) => self.dispatch(Event::Go(Route::Search(q))),
            Message::TimerTick | Message::Noop => Vec::new(),
        }
    }

    /// The "Create playlist" sheet.
    fn create_msg(&mut self, msg: CreateMsg) -> Vec<Job> {
        match msg {
            CreateMsg::Open => {
                if !self.signed_in() {
                    return self.account_msg(AccountMsg::Open);
                }
                self.create_open = true;
                self.create_name.clear();
                self.ui.text_focused = true;
                Vec::new()
            }
            CreateMsg::Close => {
                self.create_open = false;
                self.ui.text_focused = false;
                Vec::new()
            }
            CreateMsg::Name(n) => {
                self.create_name = n.chars().take(80).collect();
                Vec::new()
            }
            CreateMsg::Confirm => {
                let name = self.create_name.trim().to_owned();
                if name.is_empty() {
                    return Vec::new();
                }
                self.create_open = false;
                self.ui.text_focused = false;
                let text = if let Some(cb) = &self.hooks.on_create_playlist {
                    cb(name.clone());
                    format!("Creating \u{201c}{name}\u{201d}\u{2026}")
                } else {
                    "Creating playlists is not connected yet.".to_owned()
                };
                self.dispatch(Event::Notice(text))
            }
        }
    }

    /// Move a queue entry after the current song one place up or down. Only the entries after
    /// the current song are rewritten, so the song keeps playing and earlier ones stay put.
    fn queue_move(&mut self, index: usize, up: bool) -> Vec<Job> {
        let snap = self.ui.snapshot.clone();
        let Some(cur) = snap.queue_index else {
            return Vec::new();
        };
        let first = cur + 1;
        if index < first || index >= snap.queue.len() {
            return Vec::new();
        }
        let target = if up {
            if index == first {
                return Vec::new();
            }
            index - 1
        } else {
            if index + 1 >= snap.queue.len() {
                return Vec::new();
            }
            index + 1
        };
        let mut order: Vec<Track> = snap.queue[first..].to_vec();
        order.swap(index - first, target - first);
        self.queue_swap = (index, target);
        self.queue_anim.snap(1.0);
        self.queue_anim.target = 0.0;
        if self.ui.reduced_motion {
            self.queue_anim.snap(0.0);
        }
        let mut jobs = Vec::new();
        for i in (first..snap.queue.len()).rev() {
            jobs.extend(self.dispatch(Event::Command(PlayerCommand::RemoveAt(i))));
        }
        for t in order {
            jobs.extend(self.dispatch(Event::Command(PlayerCommand::Enqueue(t))));
        }
        jobs
    }

    /// Rebuild the daily sections when their inputs changed.
    fn refresh_daily(&mut self) {
        let key = (
            self.daily_gen,
            self.signed_in(),
            self.day.day,
            self.ui.recent.len(),
            self.home.shelves.len(),
        );
        if self.daily_key == Some(key) {
            return;
        }
        self.daily_key = Some(key);
        let pool: Vec<Track> = self
            .home
            .shelves
            .iter()
            .flat_map(|s| s.items.iter())
            .filter_map(|i| match i {
                ShelfItem::Song(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        self.daily_sections = daily::build(
            &self.daily,
            &self.ui.recent,
            &pool,
            self.signed_in(),
            self.day,
        );
    }

    /// Pause when the sleep timer ran out (or the song ended, for "End of song").
    fn check_sleep(&mut self) -> Vec<Job> {
        let Some(s) = self.sleep.clone() else {
            return Vec::new();
        };
        let now_id = self.ui.snapshot.current.as_ref().map(|t| t.id.clone());
        let due = match s.deadline {
            Some(d) => Instant::now() >= d,
            None => now_id != s.track,
        };
        if !due {
            return Vec::new();
        }
        self.sleep = None;
        let mut jobs = self.dispatch(Event::Command(PlayerCommand::Pause));
        jobs.extend(self.dispatch(Event::Notice(
            "Sleep timer ended. Playback paused.".to_owned(),
        )));
        jobs
    }

    fn learn_local(&mut self, r: &SearchResults) {
        home::merge_albums(&mut self.local_albums, home::local_albums(r));
        home::merge_local_tracks(&mut self.local_tracks, r);
    }

    fn on_frame(&mut self, now: Instant) -> Vec<Job> {
        self.frame_stats.record(now);
        let dt = self.last_frame.map_or(16.0, |p| {
            now.saturating_duration_since(p).as_secs_f32() * 1000.0
        });
        self.last_frame = Some(now);
        let mut jobs = Vec::new();
        self.anim.step(dt);
        self.scheme_fade.advance(dt);
        self.bg_fade.advance(dt);
        for (sx, sw) in &mut self.chip_pills {
            sx.step(dt.min(50.0) / 1000.0);
            sw.step(dt.min(50.0) / 1000.0);
        }
        self.queue_anim.step(dt.min(50.0) / 1000.0);
        if self.ui.is_playing() && !self.ui.reduced_motion {
            self.eq_phase = (self.eq_phase + dt.min(100.0) / 170.0) % (std::f32::consts::TAU * 8.0);
        }
        if self.detail_loading() && !self.ui.reduced_motion {
            self.shimmer = (self.shimmer + dt.min(100.0) / 1400.0) % 1.0;
        }
        color::set_active(self.scheme_fade.current());
        if (self.ui.is_buffering() || self.account_busy()) && !self.ui.reduced_motion {
            // One calm turn per 1.2 s.
            self.spin = (self.spin + dt.min(100.0) / 1200.0 * std::f32::consts::TAU)
                % std::f32::consts::TAU;
        }
        if self.ui.is_playing() {
            jobs.extend(self.dispatch(Event::Position(self.backend.player.position())));
        }
        jobs.extend(self.dispatch(Event::Frame { dt_ms: dt }));
        self.sync_anim();
        if !self.needs_frames() {
            // Idle: the next burst of frames must not see this gap as a long frame.
            self.last_frame = None;
        }
        jobs
    }

    fn on_cover(&mut self, track: TrackId, built: Result<Arc<CoverBuilt>, String>) -> Vec<Job> {
        let is_current = self.ui.snapshot.current.as_ref().map(|t| &t.id) == Some(&track);
        match built {
            Ok(b) if is_current => {
                let handle =
                    |p: &Baked| image::Handle::from_rgba(p.width, p.height, p.rgba.clone());
                self.prev_cover = self.cover.take();
                self.anim.restart_cover();
                self.cover = Some(CoverView {
                    track: track.clone(),
                    art: handle(&b.art),
                    art_bar: handle(&b.bar),
                    strip: b.data.backdrop_strip,
                    backdrop: image::Handle::from_bytes(b.data.backdrop_png.clone()),
                    tint_alpha: cover::glass_tint_alpha(b.data.backdrop_lightest),
                    scrim_alpha: cover::backdrop_scrim_alpha(b.data.backdrop_lightest),
                });
                self.cover_seed = Some(b.data.seed);
                let palette = self.themed_palette();
                self.dispatch(Event::CoverReady { track, palette })
            }
            Err(_) if is_current => {
                // No cover: neutral placeholder, fallback accent.
                self.cover = None;
                self.prev_cover = None;
                self.cover_seed = None;
                let palette = self.themed_palette();
                self.dispatch(Event::CoverReady { track, palette })
            }
            Ok(_) | Err(_) => Vec::new(),
        }
    }

    /// The accent palette for the current theme settings and cover.
    fn themed_palette(&self) -> Palette {
        let s = &self.ui.settings;
        scheme::accent_palette(s.theme, s.base, s.accent_intensity, self.cover_seed)
    }

    fn jobs_to_task(&self, jobs: Vec<Job>) -> Task<Message> {
        Task::batch(jobs.into_iter().map(|job| {
            let backend = self.backend.clone();
            Task::perform(run_job(job, backend), |m| m)
        }))
    }

    /// iced `update`.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Tab walks the focusable controls ([`fx::Fx`] draws the focus ring on the one that has it).
        if let Message::Key(Key::Tab, mods) = &message {
            return if mods.shift {
                iced::widget::operation::focus_previous()
            } else {
                iced::widget::operation::focus_next()
            };
        }
        let before = (self.mini, self.fullscreen);
        let copy = match &message {
            Message::CopyLink(t) => t.video_id.as_ref().map(|v| link_of(v.as_str())),
            _ => None,
        };
        let jobs = self.update_core(message);
        let mut tasks = vec![self.jobs_to_task(jobs)];
        if let Some(url) = copy {
            tasks.push(iced::clipboard::write(url));
            tasks.push(Task::done(Message::Ui(Event::Notice(
                "Link copied".to_owned(),
            ))));
        }
        if before != (self.mini, self.fullscreen) {
            tasks.push(self.window_mode_task(before));
        }
        if std::mem::take(&mut self.focus_search) {
            tasks.push(iced::widget::operation::focus(SEARCH_ID));
        }
        Task::batch(tasks)
    }

    /// Resize and re-mode the window after the mini player or full screen was toggled.
    fn window_mode_task(&mut self, before: (bool, bool)) -> Task<Message> {
        let (was_mini, was_full) = before;
        let (mini, full) = (self.mini, self.fullscreen);
        if mini && !was_mini {
            self.normal_size = self.window;
        }
        let normal = self.normal_size;
        let min = theme_layout::MIN_WINDOW;
        window::oldest().and_then(move |id| {
            let mut t: Vec<Task<Message>> = Vec::new();
            if full != was_full {
                t.push(window::set_mode(
                    id,
                    if full {
                        window::Mode::Fullscreen
                    } else {
                        window::Mode::Windowed
                    },
                ));
            }
            if mini != was_mini {
                if mini {
                    t.push(window::set_min_size(
                        id,
                        Some(Size::new(MINI_SIZE.0, MINI_SIZE.1)),
                    ));
                    t.push(window::resize(id, Size::new(MINI_SIZE.0, MINI_SIZE.1)));
                    t.push(window::set_level(id, window::Level::AlwaysOnTop));
                } else {
                    t.push(window::set_level(id, window::Level::Normal));
                    t.push(window::set_min_size(id, Some(Size::new(min.0, min.1))));
                    t.push(window::resize(id, normal));
                }
            }
            Task::batch(t)
        })
    }

    /// iced `view`.
    #[must_use]
    pub fn view(&self) -> Element<'_, Message> {
        views::root(self)
    }

    /// iced `subscription`: keys, window size, player events always; frames only while
    /// something moves, so an idle window redraws nothing.
    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            keyboard::listen().filter_map(map_key),
            window::resize_events().map(|(_, size)| Message::Resized(size)),
            Subscription::run_with(PlayerKey(Arc::clone(&self.backend.player)), player_events),
        ];
        if self.sleep.is_some() {
            subs.push(Subscription::run(timer_ticks));
        }
        if self.needs_frames() || self.readout {
            subs.push(window::frames().map(Message::Frame));
        }
        Subscription::batch(subs)
    }

    /// Finish animations and cross-fades (headless runs).
    pub fn finish_animations(&mut self) {
        for _ in 0..240 {
            self.dispatch(Event::Frame { dt_ms: 16.0 });
        }
        self.sync_anim();
        self.bg_fade.finish();
        self.anim.finish();
    }

    /// Run jobs to completion on the calling thread (headless runs and tests).
    pub fn settle_blocking(&mut self, jobs: Vec<Job>) {
        let mut queue = jobs;
        while let Some(job) = queue.pop() {
            let msg =
                iced_test::futures::futures::executor::block_on(run_job(job, self.backend.clone()));
            queue.extend(self.update_core(msg));
        }
    }
}

/// Shuffle in place with a small xorshift generator seeded from the clock.
fn shuffle(tracks: &mut [Track]) {
    let mut x = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0x9E37_79B9_7F4A_7C15, |d| {
            u64::from(d.subsec_nanos()) ^ d.as_secs().rotate_left(32)
        })
        | 1;
    for i in (1..tracks.len()).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        // The modulus is at most the list length, so it fits usize.
        #[allow(clippy::cast_possible_truncation)]
        let j = (x % (i as u64 + 1)) as usize;
        tracks.swap(i, j);
    }
}

fn map_key(event: keyboard::Event) -> Option<Message> {
    use keyboard::key::Named;
    let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
        return None;
    };
    let mapped = match key.as_ref() {
        keyboard::Key::Named(Named::Space) => Key::Space,
        keyboard::Key::Named(Named::ArrowLeft) => Key::Left,
        keyboard::Key::Named(Named::ArrowRight) => Key::Right,
        keyboard::Key::Named(Named::ArrowUp) => Key::Up,
        keyboard::Key::Named(Named::ArrowDown) => Key::Down,
        keyboard::Key::Named(Named::Escape) => Key::Escape,
        keyboard::Key::Named(Named::F3) => Key::F3,
        keyboard::Key::Named(Named::Tab) => Key::Tab,
        keyboard::Key::Character(c) => Key::Char(c.chars().next()?.to_ascii_lowercase()),
        _ => return None,
    };
    Some(Message::Key(
        mapped,
        Mods {
            ctrl: modifiers.control() || modifiers.command(),
            shift: modifiers.shift(),
        },
    ))
}

/// Subscription identity: one stream per player.
struct PlayerKey(Arc<dyn PlayerControl>);

impl Hash for PlayerKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // The address of the player object identifies it.
        std::ptr::addr_of!(*self.0).cast::<()>().hash(state);
    }
}

/// Poll the player's event receiver on a plain thread (the trait is non-blocking and
/// runtime-agnostic) and forward real events only, so an idle player causes no UI work.
fn player_events(key: &PlayerKey) -> impl iced::futures::Stream<Item = Message> + use<> {
    let player = Arc::clone(&key.0);
    stream::channel(
        64,
        async move |mut tx: iced::futures::channel::mpsc::Sender<Message>| {
            let mut rx = player.subscribe();
            thread::spawn(move || {
                loop {
                    let mut sent = false;
                    while let Some(ev) = rx.try_next() {
                        if tx.try_send(Message::Player(ev)).is_err() && tx.is_closed() {
                            return;
                        }
                        sent = true;
                    }
                    if !sent && tx.is_closed() {
                        return;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            });
        },
    )
}

/// One tick per second while a sleep timer runs (a plain thread, like the player events).
fn timer_ticks() -> impl iced::futures::Stream<Item = Message> {
    stream::channel(
        4,
        async move |mut tx: iced::futures::channel::mpsc::Sender<Message>| {
            thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_secs(1));
                    if tx.try_send(Message::TimerTick).is_err() && tx.is_closed() {
                        return;
                    }
                }
            });
        },
    )
}
