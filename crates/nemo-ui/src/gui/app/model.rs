//! Small state types of the app: home, detail, tabs, filters, account and settings sections.

use crate::home;
use crate::route::Route;
use nemo_core::account::{AccountError, AccountInfo, SignOutReport};
use nemo_core::catalog::{AlbumDetail, ArtistDetail, BrowseId, PlaylistDetail, Shelf, ShelfItem};
use nemo_core::secret::Secret;
use std::hash::Hash;
use std::sync::Arc;

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
