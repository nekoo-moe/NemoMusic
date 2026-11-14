//! Events, effects and the small value types the UI state reducer exchanges with the GUI shell.

use std::sync::Arc;
use std::time::Duration;

use nemo_core::catalog::SearchResults;
use nemo_core::lyrics::{LyricsDoc, ProviderId};
use nemo_core::player::{PlayerCommand, PlayerEvent, PlayerSnapshot};
use nemo_core::services::LyricsLookup;
use nemo_core::track::{Track, TrackId};

use crate::keys::KeyAction;
use crate::lyrics_view;
use crate::palette::Palette;
use crate::route::Route;
use crate::settings::{SettingChange, UiSettings};

/// How long a playback-failure notice stays, in ms.
pub const FAILURE_TOAST_MS: f32 = 7_000.0;
/// How long the "skipped segment" pill with its Undo button stays (ADR 0003: 5 s).
pub const SKIP_TOAST_MS: f32 = 5_000.0;

/// A calm notice above the player bar. Never red; it has no counter and does not blink.
#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    /// Short text.
    pub text: String,
    /// Time left in ms.
    pub left_ms: f32,
    /// Show an `Undo` button (SponsorBlock skips).
    pub undo: bool,
}

/// What a context-menu action looks up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveKind {
    /// Open the album of a track.
    Album,
    /// Open the first artist of a track.
    Artist,
}

/// A change to the queue from the queue panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueOp {
    /// Play the entry at this index.
    Jump(usize),
    /// Remove the entry at this index.
    Remove(usize),
    /// Move the entry at this index to play right after the current track.
    PlayNext(usize),
    /// Remove everything except the current track.
    Clear,
}

/// Lyrics of the current track.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LyricsState {
    /// No track.
    #[default]
    Idle,
    /// Lookup running.
    Loading,
    /// Lyrics found.
    Ready(LyricsDoc),
    /// Nothing found; the providers that were asked.
    Missing(Vec<ProviderId>),
}

/// Something that happened.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A key action.
    Key(KeyAction),
    /// The player reported something (the GUI then refreshes the snapshot).
    Player(PlayerEvent),
    /// A fresh player snapshot.
    Snapshot(PlayerSnapshot),
    /// The playback position read from the player.
    Position(Duration),
    /// A rendered frame, `dt_ms` after the previous one.
    Frame {
        /// Milliseconds since the previous frame.
        dt_ms: f32,
    },
    /// Navigate.
    Go(Route),
    /// History back.
    Back,
    /// History forward.
    Forward,
    /// Show or hide the queue panel.
    ToggleQueue,
    /// A cover finished analysing.
    CoverReady {
        /// Track the cover belongs to.
        track: TrackId,
        /// Derived palette.
        palette: Palette,
    },
    /// A lyrics lookup finished.
    LyricsReady {
        /// Track the lookup was for.
        track: TrackId,
        /// The answer.
        lookup: LyricsLookup,
        /// Stored offset for the track.
        offset_ms: i64,
    },
    /// Send a command to the player.
    Command(PlayerCommand),
    /// Seek to a fraction (0 to 1) of the current track.
    SeekFraction(f32),
    /// The user scrolled the lyrics by this many px (positive scrolls down).
    LyricsScrolled(f32),
    /// Jump back to following the current line.
    LyricsFollow,
    /// Lyrics viewport height in px.
    LyricsViewport(f32),
    /// The lyrics are shown dense (right pane) or roomy (full window).
    LyricsDense(bool),
    /// Wrapped row counts of every lyric line at the current column width (one entry per line).
    LyricsRows(Vec<lyrics_view::LineRows>),
    /// A text field gained or lost focus.
    TextFocus(bool),
    /// Reset the lyrics offset to zero.
    LyricsOffsetReset,
    /// A setting changed on the Settings screen.
    Setting(SettingChange),
    /// The notice checkbox was ticked or cleared.
    TosCheck(bool),
    /// Continue on the first-run notice (needs the checkbox).
    TosContinue,
    /// Close the notice without accepting it, for this session.
    TosLater,
    /// Show the notice again.
    TosReview,
    /// The search box text changed.
    SearchInput(String),
    /// The debounce timer of this sequence number fired.
    SearchDebounced(u64),
    /// Enter in the search box.
    SearchSubmit,
    /// Try the failed search again.
    SearchRetry,
    /// A search finished.
    SearchDone {
        /// The query that was run.
        query: String,
        /// The answer or a short error.
        result: Result<Arc<SearchResults>, String>,
    },
    /// Replace the queue with `tracks` and start at `start`.
    PlayTracks {
        /// The new queue.
        tracks: Vec<Track>,
        /// Index to start with.
        start: usize,
    },
    /// Play this track after the current one.
    PlayNext(Track),
    /// Add this track to the end of the queue.
    Enqueue(Track),
    /// Change the queue.
    Queue(QueueOp),
    /// Open the track menu.
    OpenMenu(Track),
    /// Close the track menu.
    CloseMenu,
    /// Open the album of this track.
    GoToAlbum(Track),
    /// Open the artist of this track.
    GoToArtist(Track),
    /// Hide the notice above the bar.
    DismissToast,
    /// Undo the last SponsorBlock skip.
    UndoSkip,
    /// Open or close the signal-path popover.
    ToggleSignalPath,
    /// Show a short calm notice above the bar.
    Notice(String),
}

/// Work for the GUI shell.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Send to the player.
    Send(PlayerCommand),
    /// Re-read the player snapshot and position.
    RefreshSnapshot,
    /// Load and analyse the cover of this track.
    LoadCover(Track),
    /// Look up lyrics for this track.
    LoadLyrics(Track),
    /// Store a lyrics offset.
    SaveLyricsOffset {
        /// Track key.
        track_key: String,
        /// Offset in milliseconds.
        offset_ms: i64,
    },
    /// Flip the frame-time readout (the GUI owns the sampler).
    ToggleFrameReadout,
    /// Start the debounce timer for this sequence number.
    Debounce(u64),
    /// Run this search.
    RunSearch(String),
    /// Put the keyboard focus in the search box.
    FocusSearch,
    /// Tell the app the settings changed.
    SettingsChanged(UiSettings),
    /// Look up the album or artist of this track, then open it.
    Resolve {
        /// What to open.
        kind: ResolveKind,
        /// The track the menu was opened on.
        track: Track,
    },
}
