//! Small state types of the app: home, detail, tabs, filters, account and settings sections.

use crate::home;
use crate::route::Route;
use nemo_core::account::{AccountError, AccountInfo, SignOutReport};
use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, BrowseId, HistorySection,
    PlaylistDetail, PlaylistSummary, Shelf, ShelfItem,
};
use nemo_core::secret::Secret;
use nemo_core::source_account::{SourceAccountError, SourceAccountInfo};
use nemo_core::track::Track;
use std::hash::Hash;
use std::sync::Arc;

/// Every SoundCloud page id starts with this (the SoundCloud crate's `BrowseId`s).
pub const SOUNDCLOUD_ID_PREFIX: &str = "sc:";

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

    /// `true` for the page of a SoundCloud playlist, album or user. It loads from the SoundCloud
    /// catalog and needs no YouTube Music notice.
    #[must_use]
    pub fn is_soundcloud(&self) -> bool {
        self.id.0.starts_with(SOUNDCLOUD_ID_PREFIX)
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
    /// Music videos of the search results.
    SearchVideos,
    /// Songs and videos of Explore shelf number `n` (charts first).
    Explore(usize),
    /// Songs and videos of shelf number `n` of a mood page.
    Mood(BrowseId, usize),
    /// The liked tracks of the SoundCloud account.
    ScLikes,
    /// The tracks of history section number `n` of the SoundCloud account.
    ScHistory(usize),
    /// The SoundCloud songs under the YouTube Music results of `All` in Search.
    ScSearch,
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

/// What the SoundCloud page remembers (soundcloud-account spec). The token lives only in a
/// [`Secret`]: its `Debug` prints `[redacted]`, it is never logged, and it is emptied when it is
/// sent.
#[derive(Debug, Clone)]
pub struct ScUi {
    /// The opt-in panel ("Turn on", "Cancel") is open. The switch is still off.
    pub confirming: bool,
    /// The masked token field is shown.
    pub paste: bool,
    /// What was typed in the masked field.
    pub token: Secret<String>,
    /// A sign-in or sign-out call is running.
    pub busy: bool,
    /// The running call is the sign-in window (the token field still works, and a pasted token
    /// takes over from the window).
    pub window: bool,
    /// A token took over from the window: the window's own answer is dropped when it arrives.
    pub stale_window: bool,
    /// "I am signed in, check again" was pressed for this window.
    pub rechecked: bool,
    /// Plain-words error of the last call.
    pub error: Option<String>,
    /// Sign out waits for the confirmation.
    pub confirm_sign_out: bool,
    /// Turning the switch off waits for the confirmation (it signs out and wipes the token).
    pub confirm_off: bool,
    /// Result line of the last sign-out.
    pub notice: Option<String>,
}

impl Default for ScUi {
    fn default() -> Self {
        Self {
            confirming: false,
            paste: false,
            token: Secret::new(String::new()),
            busy: false,
            window: false,
            stale_window: false,
            rechecked: false,
            error: None,
            confirm_sign_out: false,
            confirm_off: false,
            notice: None,
        }
    }
}

/// Which service the Library (and later Search) shows (soundcloud-account spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Source {
    /// YouTube Music, local files and everything as before.
    #[default]
    YouTube,
    /// The signed-in SoundCloud library.
    SoundCloud,
    /// Both, each in its own group.
    All,
}

impl Source {
    /// The choices in switch order.
    pub const ALL: [Self; 3] = [Self::YouTube, Self::SoundCloud, Self::All];

    /// The text of the switch.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::YouTube => "YouTube Music",
            Self::SoundCloud => "SoundCloud",
            Self::All => "All",
        }
    }

    /// `true` when SoundCloud results belong on screen.
    #[must_use]
    pub const fn has_soundcloud(self) -> bool {
        matches!(self, Self::SoundCloud | Self::All)
    }
}

/// Tabs of the SoundCloud library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScTab {
    /// Liked tracks.
    #[default]
    Likes,
    /// The user's playlists and the ones they liked.
    Playlists,
    /// The user's albums and the ones they liked.
    Albums,
    /// Followed artists.
    Following,
    /// Play history.
    History,
}

impl ScTab {
    /// The tabs in order.
    pub const ALL: [Self; 5] = [
        Self::Likes,
        Self::Playlists,
        Self::Albums,
        Self::Following,
        Self::History,
    ];

    /// The chip text.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Likes => "Likes",
            Self::Playlists => "Playlists",
            Self::Albums => "Albums",
            Self::Following => "Following",
            Self::History => "History",
        }
    }
}

/// What one read of the SoundCloud library brought.
#[derive(Debug, Clone, Default)]
pub struct ScLibData {
    /// Liked tracks.
    pub likes: Vec<Track>,
    /// Own playlists, then liked ones.
    pub playlists: Vec<PlaylistSummary>,
    /// How many of `playlists` the user made (they come first).
    pub own_playlists: usize,
    /// Own albums, then liked ones.
    pub albums: Vec<AlbumSummary>,
    /// Followed artists.
    pub artists: Vec<ArtistSummary>,
    /// History by day.
    pub history: Vec<HistorySection>,
}

/// Where the SoundCloud library stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScPhase {
    /// Not asked for yet.
    #[default]
    Idle,
    /// A read is running.
    Loading,
    /// The data is in.
    Done,
    /// The read failed; `error` says why in plain words.
    Failed,
}

/// The SoundCloud library the UI holds. It belongs to one sign-in: another account, or none,
/// clears it before anything is shown (criterion 12).
#[derive(Debug, Clone, Default)]
pub struct ScLibrary {
    /// Where the read stands.
    pub phase: ScPhase,
    /// The data of the last good read.
    pub data: Arc<ScLibData>,
    /// Plain-words reason of the last failure.
    pub error: Option<String>,
    /// Raised by every clear and every reload, so a late answer is dropped.
    pub generation: u64,
    /// Whether the data belongs to a signed-in account.
    pub owner: bool,
}

/// Messages of the SoundCloud page and of the SoundCloud card of the setup flow.
#[derive(Debug, Clone)]
pub enum ScMsg {
    /// Choose which service the Library shows.
    Source(Source),
    /// Open a tab of the SoundCloud library.
    Tab(ScTab),
    /// Read the SoundCloud library again (Retry, Refresh).
    Reload,
    /// A library read finished.
    LibraryLoaded {
        /// The read it answers; an older one is dropped.
        generation: u64,
        /// The data, or the plain-words reason.
        result: Result<Arc<ScLibData>, String>,
    },
    /// The switch was turned on: open the panel. The feature stays off.
    Ask,
    /// "Cancel" in the panel.
    Cancel,
    /// "Turn on" in the panel: store the choice and the confirmed notice version.
    TurnOn,
    /// The switch was turned off. Signed in, it asks first (turning off signs out and wipes the
    /// token); otherwise it turns off at once.
    TurnOff,
    /// "Turn off and sign out" after the question.
    ConfirmOff,
    /// "Cancel" after the question: the switch stays on.
    CancelOff,
    /// The sign-out that comes with turning the switch off finished.
    TurnedOff(Result<SignOutReport, SourceAccountError>),
    /// Press "Sign in with SoundCloud".
    SignIn,
    /// Show or hide the masked token field.
    PasteMode(bool),
    /// The masked field changed.
    PasteInput(Secret<String>),
    /// Press "Check token".
    PasteSubmit,
    /// Press "Sign out" (asks first).
    AskSignOut,
    /// Leave the confirmation.
    CancelSignOut,
    /// Confirm sign out.
    SignOut,
    /// Press "I am signed in, check again": the sign-in window looks for the session now.
    Recheck,
    /// The sign-in window's call finished.
    WindowEnded(Result<SourceAccountInfo, SourceAccountError>),
    /// A sign-in call with a pasted token finished.
    SignedIn(Result<SourceAccountInfo, SourceAccountError>),
    /// A sign-out call finished.
    SignedOut(Result<SignOutReport, SourceAccountError>),
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
    /// The SoundCloud switch and account.
    SoundCloud,
    /// Glass, motion, transparency.
    Appearance,
    /// Egress list.
    Privacy,
    /// Licences and credits.
    About,
}

impl SettingsSection {
    /// All sections in menu order.
    pub const ALL: [Self; 8] = [
        Self::Playback,
        Self::AudioOutput,
        Self::Lyrics,
        Self::Library,
        Self::SoundCloud,
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
            Self::SoundCloud => "SoundCloud",
            Self::Appearance => "Appearance",
            Self::Privacy => "Privacy",
            Self::About => "About",
        }
    }
}
