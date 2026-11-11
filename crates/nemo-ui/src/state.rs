//! The UI state reducer. Pure: [`UiState::apply`] takes an [`Event`], changes the state and
//! returns [`Effect`]s (player commands, loads) for the GUI shell to run. No iced types here, so
//! the behaviour is unit-tested without a window.

use std::time::Duration;

use std::sync::Arc;

use nemo_core::catalog::SearchResults;
use nemo_core::lyrics::{LyricsDoc, ProviderId};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent, PlayerSnapshot};
use nemo_core::services::LyricsLookup;
use nemo_core::track::{Track, TrackId};

use crate::home;
use crate::keys::KeyAction;
use crate::lyrics_view::{self, FOLLOW_PAUSE_MS};
use crate::motion::{Crossfade, Spring};
use crate::palette::Palette;
use crate::route::{Nav, Route};
use crate::search::{SearchState, Submit};
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

/// The whole UI state, minus pixels.
#[derive(Debug, Clone)]
pub struct UiState {
    /// Route history.
    pub nav: Nav,
    /// Last player snapshot.
    pub snapshot: PlayerSnapshot,
    /// Last position read.
    pub position: Duration,
    /// Queue panel open.
    pub queue_open: bool,
    /// Accent cross-fade.
    pub accent: Crossfade,
    /// Lyrics of the current track.
    pub lyrics: LyricsState,
    /// Lyrics offset of the current track in ms.
    pub lyrics_offset_ms: i64,
    /// Lyrics scroll position (px), animated by a spring.
    pub lyric_scroll: Spring,
    /// Remaining time of the manual-scroll pause, ms (0 = following).
    pub follow_pause_ms: f32,
    /// Lyrics viewport height, px.
    pub lyric_viewport_h: f32,
    /// Lyrics are drawn dense (right pane sizes) instead of roomy (full window sizes).
    pub lyric_dense: bool,
    /// Wrapped row counts of the lyric lines, so scroll anchoring uses the real heights.
    pub lyric_rows: Vec<lyrics_view::LineRows>,
    /// Text field focused.
    pub text_focused: bool,
    /// Honour reduced motion (the OS option or the setting).
    pub reduced_motion: bool,
    /// Settings shown on the Settings screen.
    pub settings: UiSettings,
    /// The first-run notice checkbox.
    pub tos_checked: bool,
    /// The notice was closed with "Not now" for this session.
    pub tos_dismissed: bool,
    /// Search box and its answer.
    pub search: SearchState,
    /// Recently played tracks, newest first.
    pub recent: Vec<Track>,
    /// Notice above the bar.
    pub toast: Option<Toast>,
    /// Track the context menu is open on.
    pub menu: Option<Track>,
    /// Signal-path popover open.
    pub signal_open: bool,
    motion_option: bool,
    last_active: Option<usize>,
}

impl UiState {
    /// A state that starts on `route` with the given snapshot (no loads are requested; call
    /// [`Self::apply`] with [`Event::Snapshot`] for that).
    #[must_use]
    pub fn new(route: Route, snapshot: PlayerSnapshot, reduced_motion: bool) -> Self {
        Self {
            nav: Nav::new(route),
            snapshot,
            position: Duration::ZERO,
            queue_open: false,
            accent: Crossfade::settled(Palette::fallback()),
            lyrics: LyricsState::Idle,
            lyrics_offset_ms: 0,
            lyric_scroll: Spring::new(0.0),
            follow_pause_ms: 0.0,
            lyric_viewport_h: 480.0,
            lyric_dense: false,
            lyric_rows: Vec::new(),
            text_focused: false,
            reduced_motion,
            settings: UiSettings::default(),
            tos_checked: false,
            tos_dismissed: false,
            search: SearchState::default(),
            recent: Vec::new(),
            toast: None,
            menu: None,
            signal_open: false,
            motion_option: reduced_motion,
            last_active: None,
        }
    }

    /// Start with these settings (reduce motion follows them as well as the OS option).
    #[must_use]
    pub fn with_settings(mut self, settings: UiSettings) -> Self {
        self.reduced_motion = self.motion_option || settings.reduce_motion;
        self.settings = settings;
        self.accent = Crossfade::settled(self.idle_palette());
        self
    }

    /// The accent shown while no cover applies (nothing playing, or a cover that failed).
    #[must_use]
    pub fn idle_palette(&self) -> Palette {
        let s = &self.settings;
        crate::scheme::accent_palette(s.theme, s.base, s.accent_intensity, None)
    }

    /// `true` while the first-run notice covers the window.
    #[must_use]
    pub fn tos_sheet_visible(&self) -> bool {
        !self.settings.tos_acknowledged && !self.tos_dismissed
    }

    /// `true` once YouTube Music features may be used.
    #[must_use]
    pub fn youtube_enabled(&self) -> bool {
        self.settings.tos_acknowledged
    }

    /// `true` while the player waits for data (a YouTube track takes seconds to start).
    #[must_use]
    pub fn is_buffering(&self) -> bool {
        self.snapshot.state == PlayState::Buffering
    }

    /// The current route.
    #[must_use]
    pub fn route(&self) -> &Route {
        self.nav.current()
    }

    /// `true` while the player plays or buffers.
    #[must_use]
    pub fn is_playing(&self) -> bool {
        matches!(
            self.snapshot.state,
            PlayState::Playing | PlayState::Buffering
        )
    }

    /// Index of the active lyric line now.
    #[must_use]
    pub fn active_lyric(&self) -> Option<usize> {
        match &self.lyrics {
            LyricsState::Ready(doc) => lyrics_view::active_line(
                doc,
                i64::try_from(self.position.as_millis()).unwrap_or(i64::MAX),
                self.lyrics_offset_ms,
            ),
            _ => None,
        }
    }

    /// `true` while following the current line (no manual-scroll pause).
    #[must_use]
    pub fn lyrics_following(&self) -> bool {
        self.follow_pause_ms <= 0.0
    }

    /// Whether the GUI must subscribe to frames. Idle (paused, nothing animating) is `false`, so
    /// nothing redraws.
    #[must_use]
    pub fn needs_frames(&self) -> bool {
        self.is_playing()
            || !self.accent.is_done()
            || !self.lyric_scroll.is_settled()
            || self.follow_pause_ms > 0.0
            || self.toast.is_some()
    }

    /// Apply an event; returns the effects to run.
    #[allow(clippy::too_many_lines)] // one flat match over every event
    pub fn apply(&mut self, event: Event) -> Vec<Effect> {
        let mut fx = Vec::new();
        match event {
            Event::Key(action) => self.key(action, &mut fx),
            Event::Player(ev) => {
                self.player_event(&ev);
                fx.push(Effect::RefreshSnapshot);
            }
            Event::Snapshot(s) => self.snapshot(s, &mut fx),
            Event::Position(p) => self.position = p,
            Event::Frame { dt_ms } => self.frame(dt_ms),
            Event::Go(route) => self.go(route, &mut fx),
            Event::Back => {
                self.nav.back();
            }
            Event::Forward => {
                self.nav.forward();
            }
            Event::ToggleQueue => self.queue_open = !self.queue_open,
            Event::CoverReady { track, palette } => {
                if self.snapshot.current.as_ref().map(|t| &t.id) == Some(&track) {
                    self.accent.retarget(palette, self.reduced_motion);
                }
            }
            Event::LyricsReady {
                track,
                lookup,
                offset_ms,
            } => {
                if self.snapshot.current.as_ref().map(|t| &t.id) == Some(&track) {
                    self.lyrics = match lookup.doc {
                        Some(doc) => LyricsState::Ready(doc),
                        None => LyricsState::Missing(lookup.tried),
                    };
                    self.lyrics_offset_ms = offset_ms;
                    self.last_active = None;
                    self.lyric_rows.clear();
                    self.lyric_scroll.snap(0.0);
                    self.retarget_scroll();
                    self.lyric_scroll.snap(self.lyric_scroll.target);
                }
            }
            Event::Command(c) => fx.push(Effect::Send(c)),
            Event::SeekFraction(f) => {
                if let Some(d) = self.snapshot.duration {
                    fx.push(Effect::Send(PlayerCommand::Seek(
                        d.mul_f32(f.clamp(0.0, 1.0)),
                    )));
                }
            }
            Event::LyricsScrolled(px) => {
                self.follow_pause_ms = FOLLOW_PAUSE_MS;
                let v = (self.lyric_scroll.target + px).max(0.0);
                self.lyric_scroll.snap(v);
            }
            Event::LyricsFollow => {
                self.follow_pause_ms = 0.0;
                self.retarget_scroll();
            }
            Event::LyricsViewport(h) => {
                if (self.lyric_viewport_h - h).abs() > 0.5 {
                    self.lyric_viewport_h = h;
                    self.retarget_scroll();
                }
            }
            Event::LyricsDense(dense) => {
                if self.lyric_dense != dense {
                    self.lyric_dense = dense;
                    self.retarget_scroll();
                    self.lyric_scroll.snap(self.lyric_scroll.target);
                }
            }
            Event::LyricsRows(rows) => {
                if self.lyric_rows != rows {
                    self.lyric_rows = rows;
                    self.retarget_scroll();
                    self.lyric_scroll.snap(self.lyric_scroll.target);
                }
            }
            Event::TextFocus(on) => self.text_focused = on,
            Event::LyricsOffsetReset => self.set_offset(0, &mut fx),
            Event::Setting(change) => self.setting(change, &mut fx),
            Event::TosCheck(on) => self.tos_checked = on,
            Event::TosContinue => {
                if self.tos_checked {
                    self.settings.tos_acknowledged = true;
                    fx.push(Effect::SettingsChanged(self.settings.clone()));
                }
            }
            Event::TosLater => self.tos_dismissed = true,
            Event::TosReview => self.tos_dismissed = false,
            Event::SearchInput(text) => {
                if let Some(seq) = self.search.input(text) {
                    fx.push(Effect::Debounce(seq));
                }
            }
            Event::SearchDebounced(seq) => {
                if let Some(q) = self.search.debounced(seq) {
                    fx.push(Effect::RunSearch(q));
                }
            }
            Event::SearchSubmit => match self.search.submit() {
                Submit::Nothing => {}
                Submit::Run(q) => fx.push(Effect::RunSearch(q)),
                Submit::Play(tracks) => {
                    fx.push(Effect::Send(PlayerCommand::SetQueue { tracks, start: 0 }));
                }
            },
            Event::SearchRetry => {
                if let Some(q) = self.search.retry() {
                    fx.push(Effect::RunSearch(q));
                }
            }
            Event::SearchDone { query, result } => {
                if let Some(tracks) = self.search.done(&query, result) {
                    fx.push(Effect::Send(PlayerCommand::SetQueue { tracks, start: 0 }));
                }
            }
            Event::PlayTracks { tracks, start } => {
                if start < tracks.len() {
                    fx.push(Effect::Send(PlayerCommand::SetQueue { tracks, start }));
                }
            }
            Event::PlayNext(track) => {
                self.menu = None;
                fx.push(Effect::Send(PlayerCommand::PlayNext(track)));
            }
            Event::Enqueue(track) => {
                self.menu = None;
                fx.push(Effect::Send(PlayerCommand::Enqueue(track)));
            }
            Event::Queue(op) => self.queue_op(op, &mut fx),
            Event::OpenMenu(track) => self.menu = Some(track),
            Event::CloseMenu => self.menu = None,
            Event::GoToAlbum(track) => {
                self.menu = None;
                fx.push(Effect::Resolve {
                    kind: ResolveKind::Album,
                    track,
                });
            }
            Event::GoToArtist(track) => {
                self.menu = None;
                fx.push(Effect::Resolve {
                    kind: ResolveKind::Artist,
                    track,
                });
            }
            Event::DismissToast => self.toast = None,
            Event::UndoSkip => {
                self.toast = None;
                fx.push(Effect::Send(PlayerCommand::UndoSkip));
            }
            Event::ToggleSignalPath => self.signal_open = !self.signal_open,
            Event::Notice(text) => {
                self.toast = Some(Toast {
                    text,
                    left_ms: FAILURE_TOAST_MS,
                    undo: false,
                });
            }
        }
        fx
    }

    fn go(&mut self, route: Route, fx: &mut Vec<Effect>) {
        self.menu = None;
        self.signal_open = false;
        if route == Route::Queue {
            // The queue is a panel, not a page.
            self.queue_open = !self.queue_open;
            return;
        }
        if let Route::Search(q) = &route {
            let q = q.trim().to_owned();
            if !q.is_empty()
                && q != self.search.query()
                && let Some(run) = self.search.set(&q)
            {
                fx.push(Effect::RunSearch(run));
            }
            fx.push(Effect::FocusSearch);
            self.text_focused = true;
        } else {
            self.text_focused = false;
        }
        self.nav.go(route);
    }

    fn player_event(&mut self, event: &PlayerEvent) {
        match event {
            PlayerEvent::Failed { message } => {
                self.toast = Some(Toast {
                    text: message.clone(),
                    left_ms: FAILURE_TOAST_MS,
                    undo: false,
                });
            }
            PlayerEvent::SegmentSkipped(segment) => {
                self.toast = Some(Toast {
                    text: format!(
                        "Skipped sponsor segment ({})",
                        crate::format::clock(segment.start)
                    ),
                    left_ms: SKIP_TOAST_MS,
                    undo: true,
                });
            }
            _ => {}
        }
    }

    fn setting(&mut self, change: SettingChange, fx: &mut Vec<Effect>) {
        self.settings.apply(change);
        self.reduced_motion = self.motion_option || self.settings.reduce_motion;
        match change {
            SettingChange::SponsorBlock(on) => {
                fx.push(Effect::Send(PlayerCommand::SetSkipSegments(on)));
            }
            SettingChange::Volume(v) => fx.push(Effect::Send(PlayerCommand::SetVolume(v.min(100)))),
            _ => {}
        }
        fx.push(Effect::SettingsChanged(self.settings.clone()));
    }

    fn queue_op(&mut self, op: QueueOp, fx: &mut Vec<Effect>) {
        let queue = &self.snapshot.queue;
        match op {
            QueueOp::Jump(start) => {
                if start < queue.len() {
                    fx.push(Effect::Send(PlayerCommand::SetQueue {
                        tracks: queue.clone(),
                        start,
                    }));
                }
            }
            QueueOp::Remove(i) => {
                if i < queue.len() {
                    fx.push(Effect::Send(PlayerCommand::RemoveAt(i)));
                }
            }
            QueueOp::PlayNext(i) => {
                // Taking the entry out first, then inserting it after the current track.
                if let Some(track) = queue.get(i).cloned()
                    && self.snapshot.queue_index != Some(i)
                {
                    fx.push(Effect::Send(PlayerCommand::RemoveAt(i)));
                    fx.push(Effect::Send(PlayerCommand::PlayNext(track)));
                }
            }
            QueueOp::Clear => {
                // From the end, so earlier indices stay valid; the current track stays.
                for i in (0..queue.len()).rev() {
                    if self.snapshot.queue_index != Some(i) {
                        fx.push(Effect::Send(PlayerCommand::RemoveAt(i)));
                    }
                }
            }
        }
    }

    fn key(&mut self, action: KeyAction, fx: &mut Vec<Effect>) {
        match action {
            KeyAction::TogglePlay => {
                let cmd = match self.snapshot.state {
                    PlayState::Playing | PlayState::Buffering => PlayerCommand::Pause,
                    PlayState::Paused | PlayState::Stopped => PlayerCommand::Resume,
                };
                fx.push(Effect::Send(cmd));
            }
            KeyAction::SeekBy(secs) => {
                if let Some(d) = self.snapshot.duration {
                    let now = self.position.as_secs_f64();
                    #[allow(clippy::cast_precision_loss)]
                    let to = (now + secs as f64).clamp(0.0, d.as_secs_f64());
                    fx.push(Effect::Send(PlayerCommand::Seek(Duration::from_secs_f64(
                        to,
                    ))));
                }
            }
            KeyAction::VolumeBy(step) => {
                let v = (i16::from(self.snapshot.volume_percent) + i16::from(step)).clamp(0, 100);
                fx.push(Effect::Send(PlayerCommand::SetVolume(
                    u8::try_from(v).unwrap_or(100),
                )));
            }
            KeyAction::Next => fx.push(Effect::Send(PlayerCommand::Next)),
            KeyAction::Previous => fx.push(Effect::Send(PlayerCommand::Previous)),
            KeyAction::OpenSearch => self.go(Route::Search(String::new()), fx),
            KeyAction::OpenSettings => self.go(Route::Settings, fx),
            KeyAction::Close => {
                self.text_focused = false;
                if self.menu.is_some() {
                    self.menu = None;
                } else if self.signal_open {
                    self.signal_open = false;
                } else if self.queue_open {
                    self.queue_open = false;
                } else {
                    self.nav.back();
                }
            }
            KeyAction::ToggleFrameReadout => fx.push(Effect::ToggleFrameReadout),
            KeyAction::ToggleQueue => self.queue_open = !self.queue_open,
            // The window handles these (they need the shell, not the reducer).
            KeyAction::ShowShortcuts
            | KeyAction::FullScreen
            | KeyAction::Mini
            | KeyAction::LikeCurrent
            | KeyAction::Tools => {}
            KeyAction::LyricsOffsetBy(delta) => {
                let next = (self.lyrics_offset_ms + delta).clamp(-10_000, 10_000);
                self.set_offset(next, fx);
            }
        }
    }

    fn set_offset(&mut self, offset_ms: i64, fx: &mut Vec<Effect>) {
        if let Some(t) = &self.snapshot.current {
            self.lyrics_offset_ms = offset_ms;
            fx.push(Effect::SaveLyricsOffset {
                track_key: t.id.0.clone(),
                offset_ms,
            });
        }
    }

    fn snapshot(&mut self, s: PlayerSnapshot, fx: &mut Vec<Effect>) {
        let changed =
            self.snapshot.current.as_ref().map(|t| &t.id) != s.current.as_ref().map(|t| &t.id);
        self.snapshot = s;
        if !changed {
            return;
        }
        self.position = Duration::ZERO;
        self.lyrics_offset_ms = 0;
        self.last_active = None;
        self.lyric_scroll.snap(0.0);
        self.follow_pause_ms = 0.0;
        if let Some(track) = self.snapshot.current.clone() {
            home::remember(&mut self.recent, &track);
            self.lyrics = LyricsState::Loading;
            fx.push(Effect::LoadCover(track.clone()));
            fx.push(Effect::LoadLyrics(track));
        } else {
            self.lyrics = LyricsState::Idle;
            self.accent
                .retarget(self.idle_palette(), self.reduced_motion);
        }
    }

    fn frame(&mut self, dt_ms: f32) {
        let dt_ms = dt_ms.clamp(0.0, 100.0);
        self.accent.advance(dt_ms);
        if let Some(t) = &mut self.toast {
            t.left_ms -= dt_ms;
            if t.left_ms <= 0.0 {
                self.toast = None;
            }
        }
        if self.follow_pause_ms > 0.0 {
            // The "Back to current line" pill is visible meanwhile; following resumes by itself.
            self.follow_pause_ms = (self.follow_pause_ms - dt_ms).max(0.0);
        } else {
            self.retarget_scroll();
        }
        if self.reduced_motion {
            self.lyric_scroll.snap(self.lyric_scroll.target);
        } else {
            self.lyric_scroll.step(dt_ms / 1000.0);
        }
    }

    fn retarget_scroll(&mut self) {
        let active = self.active_lyric();
        self.last_active = active;
        self.lyric_scroll.target = lyrics_view::scroll_target_rows(
            active,
            self.lyric_viewport_h,
            self.lyric_dense,
            &self.lyric_rows,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;
    use nemo_core::lyrics::{Granularity, LyricLine};

    fn playing(index: usize) -> PlayerSnapshot {
        let t = mock::track(index);
        PlayerSnapshot {
            state: PlayState::Playing,
            duration: t.duration,
            queue: vec![t.clone()],
            queue_index: Some(0),
            current: Some(t),
            volume_percent: 50,
            ..PlayerSnapshot::default()
        }
    }

    fn state() -> UiState {
        let mut s = UiState::new(Route::Home, PlayerSnapshot::default(), false);
        s.apply(Event::Snapshot(playing(0)));
        s
    }

    fn doc() -> LyricsDoc {
        mock::lyrics_for(0, Duration::from_secs(200)).expect("lyrics")
    }

    fn lookup(doc: Option<LyricsDoc>) -> LyricsLookup {
        LyricsLookup {
            doc,
            tried: vec![ProviderId::new("A")],
        }
    }

    #[test]
    fn a_new_track_requests_cover_and_lyrics_once() {
        let mut s = UiState::new(Route::Home, PlayerSnapshot::default(), false);
        let fx = s.apply(Event::Snapshot(playing(0)));
        assert_eq!(fx.len(), 2);
        assert!(matches!(fx[0], Effect::LoadCover(_)) && matches!(fx[1], Effect::LoadLyrics(_)));
        assert_eq!(s.lyrics, LyricsState::Loading);
        assert!(
            s.apply(Event::Snapshot(playing(0))).is_empty(),
            "same track"
        );
        assert_eq!(s.apply(Event::Snapshot(playing(1))).len(), 2);
        assert_eq!(s.apply(Event::Snapshot(PlayerSnapshot::default())).len(), 0);
        assert_eq!(s.lyrics, LyricsState::Idle);
    }

    #[test]
    fn stale_cover_and_lyrics_are_ignored() {
        let mut s = state();
        let other = mock::track(9).id;
        s.apply(Event::CoverReady {
            track: other.clone(),
            palette: Palette::fallback(),
        });
        assert!(s.accent.is_done(), "no fade started for another track");
        s.apply(Event::LyricsReady {
            track: other,
            lookup: lookup(Some(doc())),
            offset_ms: 0,
        });
        assert_eq!(s.lyrics, LyricsState::Loading);
    }

    #[test]
    fn cover_ready_starts_a_crossfade() {
        let mut s = state();
        let mut p = Palette::fallback();
        p.primary.r = p.primary.r.wrapping_add(40);
        s.apply(Event::CoverReady {
            track: mock::track(0).id,
            palette: p,
        });
        assert!(!s.accent.is_done());
        assert!(s.needs_frames());
        for _ in 0..30 {
            s.apply(Event::Frame { dt_ms: 16.0 });
        }
        assert!(s.accent.is_done());
        assert_eq!(s.accent.current(), p);
    }

    #[test]
    fn reduced_motion_swaps_the_accent_at_once() {
        let mut s = UiState::new(Route::Home, playing(0), true);
        let mut p = Palette::fallback();
        p.primary.b = p.primary.b.wrapping_add(30);
        s.apply(Event::CoverReady {
            track: mock::track(0).id,
            palette: p,
        });
        assert!(s.accent.is_done());
    }

    #[test]
    fn lyrics_missing_and_ready() {
        let mut s = state();
        s.apply(Event::LyricsReady {
            track: mock::track(0).id,
            lookup: lookup(None),
            offset_ms: 0,
        });
        assert_eq!(s.lyrics, LyricsState::Missing(vec![ProviderId::new("A")]));
        s.apply(Event::LyricsReady {
            track: mock::track(0).id,
            lookup: lookup(Some(doc())),
            offset_ms: 250,
        });
        assert!(matches!(s.lyrics, LyricsState::Ready(_)));
        assert_eq!(s.lyrics_offset_ms, 250);
    }

    #[test]
    fn idle_needs_no_frames_and_playing_does() {
        let mut s = state();
        assert!(s.needs_frames(), "playing");
        let mut paused = playing(0);
        paused.state = PlayState::Paused;
        s.apply(Event::Snapshot(paused));
        assert!(!s.needs_frames(), "paused and settled: zero redraws");
    }

    #[test]
    fn toggle_play_follows_the_state() {
        let mut s = state();
        assert_eq!(
            s.apply(Event::Key(KeyAction::TogglePlay)),
            vec![Effect::Send(PlayerCommand::Pause)]
        );
        let mut snap = playing(0);
        snap.state = PlayState::Paused;
        s.apply(Event::Snapshot(snap.clone()));
        assert_eq!(
            s.apply(Event::Key(KeyAction::TogglePlay)),
            vec![Effect::Send(PlayerCommand::Resume)]
        );
        snap.state = PlayState::Stopped;
        s.apply(Event::Snapshot(snap));
        assert_eq!(
            s.apply(Event::Key(KeyAction::TogglePlay)),
            vec![Effect::Send(PlayerCommand::Resume)]
        );
    }

    #[test]
    fn seek_and_volume_keys_clamp() {
        let mut s = state();
        s.apply(Event::Position(Duration::from_secs(10)));
        assert_eq!(
            s.apply(Event::Key(KeyAction::SeekBy(5))),
            vec![Effect::Send(PlayerCommand::Seek(Duration::from_secs(15)))]
        );
        assert_eq!(
            s.apply(Event::Key(KeyAction::SeekBy(-30))),
            vec![Effect::Send(PlayerCommand::Seek(Duration::ZERO))]
        );
        s.apply(Event::Position(Duration::from_secs(1000)));
        let d = s.snapshot.duration.expect("duration");
        assert_eq!(
            s.apply(Event::Key(KeyAction::SeekBy(30))),
            vec![Effect::Send(PlayerCommand::Seek(d))]
        );
        assert_eq!(
            s.apply(Event::Key(KeyAction::VolumeBy(5))),
            vec![Effect::Send(PlayerCommand::SetVolume(55))]
        );
        s.snapshot.volume_percent = 3;
        assert_eq!(
            s.apply(Event::Key(KeyAction::VolumeBy(-5))),
            vec![Effect::Send(PlayerCommand::SetVolume(0))]
        );
        s.snapshot.volume_percent = 98;
        assert_eq!(
            s.apply(Event::Key(KeyAction::VolumeBy(5))),
            vec![Effect::Send(PlayerCommand::SetVolume(100))]
        );
    }

    #[test]
    fn seek_fraction_uses_the_duration() {
        let mut s = state();
        let d = s.snapshot.duration.expect("duration");
        assert_eq!(
            s.apply(Event::SeekFraction(0.5)),
            vec![Effect::Send(PlayerCommand::Seek(d.mul_f32(0.5)))]
        );
        assert_eq!(
            s.apply(Event::SeekFraction(7.0)),
            vec![Effect::Send(PlayerCommand::Seek(d))]
        );
        let mut empty = UiState::new(Route::Home, PlayerSnapshot::default(), false);
        assert_eq!(empty.apply(Event::SeekFraction(0.5)), vec![]);
    }

    #[test]
    fn navigation_keys_and_close() {
        let mut s = state();
        s.apply(Event::Key(KeyAction::OpenSearch));
        assert!(matches!(s.route(), Route::Search(_)));
        s.apply(Event::Key(KeyAction::OpenSettings));
        assert_eq!(s.route(), &Route::Settings);
        s.apply(Event::ToggleQueue);
        s.apply(Event::Key(KeyAction::Close));
        assert!(!s.queue_open, "Esc closes the panel first");
        assert_eq!(s.route(), &Route::Settings);
        s.apply(Event::Key(KeyAction::Close));
        assert!(matches!(s.route(), Route::Search(_)));
        s.apply(Event::Forward);
        assert_eq!(s.route(), &Route::Settings);
    }

    #[test]
    fn lyrics_offset_keys_clamp_and_persist() {
        let mut s = state();
        let fx = s.apply(Event::Key(KeyAction::LyricsOffsetBy(50)));
        assert_eq!(
            fx,
            vec![Effect::SaveLyricsOffset {
                track_key: "mock-track-0".into(),
                offset_ms: 50
            }]
        );
        s.lyrics_offset_ms = 9_990;
        s.apply(Event::Key(KeyAction::LyricsOffsetBy(50)));
        assert_eq!(s.lyrics_offset_ms, 10_000);
        let fx = s.apply(Event::LyricsOffsetReset);
        assert_eq!(s.lyrics_offset_ms, 0);
        assert_eq!(fx.len(), 1);
        let mut none = UiState::new(Route::Home, PlayerSnapshot::default(), false);
        assert_eq!(
            none.apply(Event::Key(KeyAction::LyricsOffsetBy(50))),
            vec![]
        );
    }

    #[test]
    fn frame_readout_key_is_an_effect() {
        let mut s = state();
        assert_eq!(
            s.apply(Event::Key(KeyAction::ToggleFrameReadout)),
            vec![Effect::ToggleFrameReadout]
        );
    }

    #[test]
    fn the_lyric_scroll_follows_the_active_line_and_pauses_on_manual_scroll() {
        let mut s = state();
        s.apply(Event::LyricsReady {
            track: mock::track(0).id,
            lookup: lookup(Some(doc())),
            offset_ms: 0,
        });
        s.apply(Event::Position(Duration::from_secs(60)));
        for _ in 0..120 {
            s.apply(Event::Frame { dt_ms: 16.0 });
        }
        let active = s.active_lyric().expect("active");
        assert!(active > 5);
        assert!(s.lyric_scroll.is_settled());
        let target = lyrics_view::scroll_target(Some(active), s.lyric_viewport_h);
        assert!((s.lyric_scroll.value - target).abs() < 0.5);

        s.apply(Event::LyricsScrolled(120.0));
        assert!(!s.lyrics_following());
        let held = s.lyric_scroll.value;
        s.apply(Event::Position(Duration::from_secs(100)));
        s.apply(Event::Frame { dt_ms: 16.0 });
        assert!(
            (s.lyric_scroll.value - held).abs() < 0.5,
            "no auto-follow during the pause"
        );
        for _ in 0..300 {
            s.apply(Event::Frame { dt_ms: 16.0 });
        }
        assert!(s.lyrics_following(), "the 4 s pause ended");
        for _ in 0..120 {
            s.apply(Event::Frame { dt_ms: 16.0 });
        }
        let target = lyrics_view::scroll_target(s.active_lyric(), s.lyric_viewport_h);
        assert!(
            (s.lyric_scroll.value - target).abs() < 0.5,
            "glided back to the current line"
        );
        assert!((s.lyric_scroll.value - held).abs() > 1.0);

        s.apply(Event::LyricsScrolled(-200.0));
        assert!(!s.lyrics_following());
        s.apply(Event::LyricsFollow);
        assert!(s.lyrics_following(), "the pill returns at once");
    }

    #[test]
    fn unsynced_lyrics_never_scroll() {
        let mut s = state();
        let d = LyricsDoc {
            lines: vec![
                LyricLine {
                    start_ms: None,
                    end_ms: None,
                    text: "x".into(),
                    words: Vec::new(),
                };
                40
            ],
            granularity: Granularity::Unsynced,
            provider: ProviderId::new("A"),
            matched: None,
            instrumental: false,
        };
        s.apply(Event::LyricsReady {
            track: mock::track(0).id,
            lookup: lookup(Some(d)),
            offset_ms: 0,
        });
        s.apply(Event::Position(Duration::from_secs(90)));
        s.apply(Event::Frame { dt_ms: 16.0 });
        assert_eq!(s.active_lyric(), None);
        assert!(s.lyric_scroll.value.abs() < f32::EPSILON);
    }
}

#[cfg(test)]
mod screen_tests {
    use super::*;
    use crate::mock;
    use nemo_core::segment::{Segment, SegmentCategory};

    fn state() -> UiState {
        let mut s = UiState::new(Route::Home, PlayerSnapshot::default(), false);
        let queue: Vec<Track> = (0..4).map(mock::track).collect();
        s.apply(Event::Snapshot(PlayerSnapshot {
            state: PlayState::Playing,
            current: Some(queue[1].clone()),
            queue,
            queue_index: Some(1),
            ..PlayerSnapshot::default()
        }));
        s
    }

    #[test]
    fn a_failure_is_a_calm_toast_that_expires() {
        let mut s = state();
        s.apply(Event::Player(PlayerEvent::Failed {
            message: "Could not play this song.".into(),
        }));
        assert_eq!(s.toast.as_ref().map(|t| t.undo), Some(false));
        for _ in 0..500 {
            s.apply(Event::Frame { dt_ms: 16.0 });
        }
        assert!(s.toast.is_none());
    }

    #[test]
    fn a_skipped_segment_offers_undo() {
        let mut s = state();
        s.apply(Event::Player(PlayerEvent::SegmentSkipped(Segment {
            start: Duration::from_secs(42),
            end: Duration::from_secs(60),
            category: SegmentCategory::Sponsor,
            id: "x".into(),
        })));
        let t = s.toast.clone().expect("toast");
        assert!(t.undo && t.text.contains("(0:42)"));
        assert_eq!(
            s.apply(Event::UndoSkip),
            vec![Effect::Send(PlayerCommand::UndoSkip)]
        );
        assert!(s.toast.is_none());
    }

    #[test]
    fn the_notice_needs_its_checkbox_and_reports_the_setting() {
        let mut s = state();
        assert!(s.tos_sheet_visible() && !s.youtube_enabled());
        assert_eq!(s.apply(Event::TosContinue), vec![]);
        s.apply(Event::TosCheck(true));
        let fx = s.apply(Event::TosContinue);
        assert!(matches!(&fx[..], [Effect::SettingsChanged(x)] if x.tos_acknowledged));
        assert!(!s.tos_sheet_visible() && s.youtube_enabled());
    }

    #[test]
    fn not_now_hides_the_sheet_but_keeps_youtube_off() {
        let mut s = state();
        s.apply(Event::TosLater);
        assert!(!s.tos_sheet_visible() && !s.youtube_enabled());
        s.apply(Event::TosReview);
        assert!(s.tos_sheet_visible());
    }

    #[test]
    fn sponsorblock_switch_sends_the_player_command_and_the_callback() {
        let mut s = state();
        let fx = s.apply(Event::Setting(SettingChange::SponsorBlock(true)));
        assert_eq!(fx[0], Effect::Send(PlayerCommand::SetSkipSegments(true)));
        assert!(matches!(&fx[1], Effect::SettingsChanged(x) if x.sponsorblock));
        let fx = s.apply(Event::Setting(SettingChange::ReduceMotion(true)));
        assert!(s.reduced_motion && fx.len() == 1);
    }

    #[test]
    fn queue_clear_removes_from_the_end_and_keeps_the_current_track() {
        let mut s = state();
        let fx = s.apply(Event::Queue(QueueOp::Clear));
        assert_eq!(
            fx,
            vec![
                Effect::Send(PlayerCommand::RemoveAt(3)),
                Effect::Send(PlayerCommand::RemoveAt(2)),
                Effect::Send(PlayerCommand::RemoveAt(0)),
            ]
        );
        let fx = s.apply(Event::Queue(QueueOp::PlayNext(3)));
        assert_eq!(fx.len(), 2);
        assert_eq!(s.apply(Event::Queue(QueueOp::PlayNext(1))), vec![]);
    }

    #[test]
    fn search_events_debounce_then_run_and_enter_plays() {
        let mut s = state();
        let fx = s.apply(Event::SearchInput("lofi".into()));
        let Effect::Debounce(seq) = fx[0] else {
            panic!("expected a debounce, got {fx:?}");
        };
        let fx = s.apply(Event::SearchDebounced(seq));
        assert_eq!(fx, vec![Effect::RunSearch("lofi".into())]);
        let results = Arc::new(SearchResults {
            songs: vec![mock::track(5)],
            ..SearchResults::default()
        });
        s.apply(Event::SearchDone {
            query: "lofi".into(),
            result: Ok(results),
        });
        let fx = s.apply(Event::SearchSubmit);
        assert!(matches!(
            &fx[..],
            [Effect::Send(PlayerCommand::SetQueue { start: 0, .. })]
        ));
    }

    #[test]
    fn search_shortcuts_focus_the_box_and_esc_closes_things_in_order() {
        let mut s = state();
        let fx = s.apply(Event::Key(KeyAction::OpenSearch));
        assert!(fx.contains(&Effect::FocusSearch) && s.text_focused);
        s.apply(Event::OpenMenu(mock::track(2)));
        s.apply(Event::ToggleSignalPath);
        s.apply(Event::ToggleQueue);
        s.apply(Event::Key(KeyAction::Close));
        assert!(s.menu.is_none() && s.signal_open && s.queue_open);
        s.apply(Event::Key(KeyAction::Close));
        assert!(!s.signal_open && s.queue_open);
        s.apply(Event::Key(KeyAction::Close));
        assert!(!s.queue_open);
    }

    #[test]
    fn recent_tracks_and_menu_actions() {
        let mut s = state();
        assert_eq!(s.recent.len(), 1);
        s.apply(Event::OpenMenu(mock::track(2)));
        let fx = s.apply(Event::GoToArtist(mock::track(2)));
        assert!(s.menu.is_none());
        assert!(matches!(
            &fx[..],
            [Effect::Resolve {
                kind: ResolveKind::Artist,
                ..
            }]
        ));
        s.apply(Event::Go(Route::Queue));
        assert!(
            s.queue_open && *s.route() == Route::Home,
            "queue is a panel"
        );
    }
}
