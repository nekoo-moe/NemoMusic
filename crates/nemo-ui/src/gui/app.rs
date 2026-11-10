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
use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistDetail, BrowseId, PlaylistDetail, SearchResults, Shelf,
    ShelfItem,
};
use nemo_core::lyrics::LyricsQuery;
use nemo_core::player::PlayerEvent;
use nemo_core::services::{Backend, LyricsLookup, PlayerControl};
use nemo_core::track::{Artwork, Track, TrackId};

use super::style;
use super::views;
use crate::cover::{self, Baked, CoverData};
use crate::frame_stats::FrameStats;
use crate::home;
use crate::keys::{self, Key, Mods};
use crate::lru::{ART_CAPACITY, Lru};
use crate::palette::Palette;
use crate::route::Route;
use crate::search::DEBOUNCE_MS;
use crate::settings::UiSettings;
use crate::state::{Effect, Event, ResolveKind, UiState};
use crate::theme::{layout as theme_layout, shape};

/// Widget id of the search box.
pub const SEARCH_ID: &str = "nemo-search";
/// Most rows a list draws at once; "Show more" adds this many again.
pub const ROWS_STEP: usize = 200;
/// Images fetched at the same time.
const ART_PARALLEL: usize = 6;
/// Message of the search page while the first-run notice is not accepted.
pub const GATE_MESSAGE: &str = "YouTube Music is waiting for you to read the notice.";

/// Options for [`run`](super::run).
#[derive(Clone)]
pub struct UiOptions {
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
        let radius = match self.shape {
            ArtShape::Circle => return 0.5,
            ArtShape::Card => shape::MD,
            ArtShape::Row => shape::SM,
            ArtShape::Hero => shape::LG,
        };
        // Slot sizes are small integers, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let size = self.size.max(1) as f32;
        radius / size
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
    /// Load a detail page again after an error.
    RetryDetail(DetailKey),
    /// The user wants to add a music folder.
    AddFolder,
    /// Nothing.
    Noop,
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
    /// Find the album or artist page of a track.
    Resolve {
        /// What to open.
        kind: ResolveKind,
        /// The track.
        track: Track,
    },
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
                        let art = cover::bake(&raw, 480, 16.0 / 480.0)?;
                        let bar = cover::bake(&raw, 128, 12.0 / 64.0)?;
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
        Job::Resolve { kind, track } => Message::Ui(resolve(&backend, kind, &track).await),
    }
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
    pub(crate) seek_drag: Option<f32>,
    pub(crate) compact_tab: CompactTab,
    pub(crate) settings_section: SettingsSection,
    pub(crate) rows_limit: usize,
    pub(crate) spin: f32,
    pub(crate) frame_stats: FrameStats,
    pub(crate) readout: bool,
    pub(crate) music_folders: Vec<String>,
    on_settings: Option<Arc<dyn Fn(UiSettings) + Send + Sync>>,
    on_add_music_folder: Option<Arc<dyn Fn() + Send + Sync>>,
    focus_search: bool,
    last_route: Route,
    last_frame: Option<Instant>,
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
            seek_drag: None,
            compact_tab: CompactTab::default(),
            settings_section: SettingsSection::default(),
            rows_limit: ROWS_STEP,
            spin: 0.0,
            frame_stats: FrameStats::default(),
            readout: options.show_frame_readout,
            music_folders: options.music_folders.clone(),
            on_settings: options.on_settings.clone(),
            on_add_music_folder: options.on_add_music_folder.clone(),
            focus_search: false,
            last_route: options.start_route.clone(),
            last_frame: None,
        };
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
        let v = views::now_playing::lyrics_viewport_height(w, h);
        self.dispatch(Event::LyricsViewport(v))
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
        if *self.ui.route() != self.last_route {
            self.last_route = self.ui.route().clone();
            self.rows_limit = ROWS_STEP;
        }
        let mut jobs = self.pump_loads();
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
                Some(action) => self.dispatch(Event::Key(action)),
                None => Vec::new(),
            },
            Message::Resized(size) => {
                self.window = size;
                self.sync_viewport()
            }
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
                self.details.insert(key, state);
                Vec::new()
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
            } => self.dispatch(Event::LyricsReady {
                track,
                lookup: lookup.as_ref().clone(),
                offset_ms,
            }),
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
            Message::AddFolder => {
                if let Some(cb) = &self.on_add_music_folder {
                    cb();
                }
                Vec::new()
            }
            Message::Noop => Vec::new(),
        }
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
        if self.ui.is_buffering() && !self.ui.reduced_motion {
            // One calm turn per 1.2 s.
            self.spin = (self.spin + dt.min(100.0) / 1200.0 * std::f32::consts::TAU)
                % std::f32::consts::TAU;
        }
        if self.ui.is_playing() {
            jobs.extend(self.dispatch(Event::Position(self.backend.player.position())));
        }
        jobs.extend(self.dispatch(Event::Frame { dt_ms: dt }));
        if !self.ui.needs_frames() {
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
                self.cover = Some(CoverView {
                    track: track.clone(),
                    art: handle(&b.art),
                    art_bar: handle(&b.bar),
                    strip: b.data.backdrop_strip,
                    backdrop: image::Handle::from_bytes(b.data.backdrop_png.clone()),
                    tint_alpha: cover::glass_tint_alpha(b.data.backdrop_lightest),
                    scrim_alpha: cover::backdrop_scrim_alpha(b.data.backdrop_lightest),
                });
                self.dispatch(Event::CoverReady {
                    track,
                    palette: b.data.palette,
                })
            }
            Err(_) if is_current => {
                // No cover: neutral placeholder, fallback accent.
                self.cover = None;
                self.dispatch(Event::CoverReady {
                    track,
                    palette: Palette::fallback(),
                })
            }
            Ok(_) | Err(_) => Vec::new(),
        }
    }

    fn jobs_to_task(&self, jobs: Vec<Job>) -> Task<Message> {
        Task::batch(jobs.into_iter().map(|job| {
            let backend = self.backend.clone();
            Task::perform(run_job(job, backend), |m| m)
        }))
    }

    /// iced `update`.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        let jobs = self.update_core(message);
        let mut tasks = vec![self.jobs_to_task(jobs)];
        if std::mem::take(&mut self.focus_search) {
            tasks.push(iced::widget::operation::focus(SEARCH_ID));
        }
        Task::batch(tasks)
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
        if self.ui.needs_frames() || self.readout {
            subs.push(window::frames().map(Message::Frame));
        }
        Subscription::batch(subs)
    }

    /// Finish animations and cross-fades (headless runs).
    pub fn finish_animations(&mut self) {
        for _ in 0..240 {
            self.dispatch(Event::Frame { dt_ms: 16.0 });
        }
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
