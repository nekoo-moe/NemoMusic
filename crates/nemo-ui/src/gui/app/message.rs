//! The [`Message`] the app handles and the [`Job`]s that produce them.

#[cfg(doc)]
use crate::explore::MOODS;
#[cfg(doc)]
use crate::home;

use super::art::ArtSpec;
use super::model::{
    AccountMsg, CompactTab, DetailData, DetailKey, HomeFilter, LibFilter, ListSource, NpTab, ScMsg,
    SettingsSection, Source,
};
use super::option::{AccountLibrary, CreateMsg};
use crate::cover::{Baked, CoverData};
use crate::daily::DailyData;
use crate::explore::{ContentFilter, LibLayout, LibSort, LibTab, SleepChoice};
use crate::keys::{Key, Mods};
use crate::palette::Seed;
use crate::refresh::Start;
use crate::state::{Event, ResolveKind};
use iced::Size;
use nemo_core::catalog::{BrowseId, Explore, FeedShelf, SearchKind, SearchResults, Shelf};
use nemo_core::lyrics::{LyricsCandidate, ProviderId};
use nemo_core::player::PlayerEvent;
use nemo_core::secret::Secret;
use nemo_core::services::LyricsLookup;
use nemo_core::track::{Artwork, Track, TrackId};
use std::sync::Arc;
use std::time::Instant;

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
    /// The window scale factor is known or changed (moved to another screen, OS scale setting).
    Rescaled(f32),
    /// The window gained or lost focus: while it has none, no frame is requested.
    Focus(bool),
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
        /// The listening area whose search it was. An answer that comes after a switch belongs to
        /// the area that asked, not to the one on screen.
        source: Source,
        /// The query that was run.
        query: String,
        /// The answer.
        result: Result<Arc<SearchResults>, String>,
    },
    /// A search with a chip answered.
    SearchKindDone {
        /// The listening area whose search it was.
        source: Source,
        /// The query that was run.
        query: String,
        /// The chip.
        kind: SearchKind,
        /// The answer.
        result: Result<Arc<SearchResults>, String>,
    },
    /// A SoundCloud account change (a like, a playlist change, an unfollow) or its answer.
    ScWrite(super::sc_write::ScWriteMsg),
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
        /// Window scale in percent the cover was baked for.
        scale: u32,
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
    /// The pointer entered (`Some`: the message that plays it) or left the play target of a song
    /// (cold-start spec: resolve ahead of the click).
    HoverTarget(Option<Box<Message>>),
    /// The hover delay of this generation ended.
    HoverDue(u64),
    /// The left mouse button went down somewhere in the window.
    PointerDown,
    /// Play a list shuffled.
    Shuffle(ListSource),
    /// Open the Sort menu of an album or playlist page, or close it (`None`).
    SortMenu(Option<DetailKey>),
    /// Show an album or playlist page in this order, and remember it for the page.
    SortCollection {
        /// The page.
        key: DetailKey,
        /// The order.
        sort: crate::collection_sort::CollectionSort,
    },
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
    /// The SoundCloud page and the SoundCloud card of the setup flow.
    Sc(ScMsg),
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
    /// The `Restart now` pill of the Window glass card (bar-material spec).
    Restart,
    /// The folder dialog closed: the chosen folder, or `None` when it was cancelled.
    FolderPicked(Option<String>),
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
    /// Play every song of a page shuffled (loads it first when needed).
    ShuffleCollection(DetailKey),
    /// A built-in mood tile on Explore (index into [`MOODS`]), shown when the catalog gave none.
    PickMood(usize),
    /// Hook: shelves for the Explore page from the catalog.
    ExploreLoaded(Arc<Vec<Shelf>>),
    /// The Explore page and its mood and genre pages.
    Explore(ExploreMsg),
    /// Hook: the account library from the catalog.
    AccountLibrary(Arc<AccountLibrary>),
    /// Open or close the tools panel (sleep timer, speed, shortcuts, window modes).
    Tools(bool),
    /// The speaker button of the player bar: volume to 0, or back to what it was.
    ToggleMute,
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
    /// Forget the per-song pick and show the best source again (Tools panel).
    LyricsAuto,
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
    /// Reload the music data of the account (the Refresh button, `F5`, `Ctrl+R`).
    Refresh,
    /// A step of a reload answered.
    Refreshed {
        /// The run the step belongs to.
        generation: u64,
        /// The answer.
        data: RefreshData,
    },
    /// Nothing.
    Noop,
}

/// Messages of the Explore page (real-explore spec).
#[derive(Debug, Clone)]
pub enum ExploreMsg {
    /// The catalog's Explore page answered.
    Loaded(Result<Arc<Explore>, String>),
    /// A mood or genre page answered.
    MoodLoaded {
        /// The mood id.
        id: BrowseId,
        /// Its shelves.
        result: Result<Arc<Vec<FeedShelf>>, String>,
    },
    /// Ask for the Explore page again.
    Retry,
    /// Ask for a mood page again.
    RetryMood(BrowseId),
}

/// The answer of one reload step.
#[derive(Debug, Clone)]
pub enum RefreshData {
    /// The Home shelves. `Ok(None)` when the catalog has no home page (signed out).
    Home(Result<Option<Arc<Vec<Shelf>>>, String>),
    /// The account library, history and home feed.
    Account(Arc<AccountBundle>),
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
    /// Open the folder dialog and wait for the person's choice.
    PickFolder(super::FolderPicker),
    /// Wait out the hover delay of a play target.
    HoverDelay(u64),
    /// Ask the catalog for a home feed.
    HomeFeed,
    /// Run seed search number `n` for Home.
    HomeSeed(usize),
    /// List the local library.
    Library,
    /// Run a search.
    Search(String),
    /// Run a search narrowed to one kind (a filter chip).
    SearchKind {
        /// The query.
        query: String,
        /// The filter.
        kind: nemo_core::catalog::SearchKind,
    },
    /// Wait out the search debounce.
    Debounce(u64),
    /// Load a detail page.
    Detail(DetailKey),
    /// Load one picture.
    Art(ArtSpec),
    /// Load and analyse a cover for a window scale in percent.
    Cover(Track, u32),
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
    /// Open the SoundCloud sign-in window.
    ScSignIn,
    /// Sign in to SoundCloud with a pasted token.
    ScSignInToken(Secret<String>),
    /// Sign out of SoundCloud, then the switch goes off (turning it off wipes the token).
    ScTurnOff,
    /// Sign out of SoundCloud.
    ScSignOut,
    /// Search SoundCloud (the Search page is set to SoundCloud).
    ScSearch(String),
    /// Search SoundCloud, one kind (a filter chip).
    ScSearchKind {
        /// The text.
        query: String,
        /// The chip.
        kind: nemo_core::catalog::SearchKind,
    },
    /// Send one change of the SoundCloud account.
    ScWrite(super::sc_write::ScWriteJob),
    /// Read the SoundCloud library (likes, playlists, albums, followed artists, history).
    ScLibrary {
        /// The read; its answer carries it back.
        generation: u64,
    },
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
    /// Ask the catalog for the account library, history and home feed (one call after the
    /// other, at human pace).
    AccountData,
    /// Ask the catalog for its Explore page.
    Explore,
    /// Ask the catalog for a mood or genre page.
    Mood(BrowseId),
    /// Run one step of a reload.
    Refresh(Start),
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
