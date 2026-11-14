//! The UI state reducer. Pure: [`UiState::apply`] takes an [`Event`], changes the state and
//! returns [`Effect`]s (player commands, loads) for the GUI shell to run. No iced types here, so
//! the behaviour is unit-tested without a window.

use std::time::Duration;

use nemo_core::player::{PlayState, PlayerCommand, PlayerSnapshot};
use nemo_core::track::Track;

use crate::lyrics_view::{self, FOLLOW_PAUSE_MS};
use crate::motion::{Crossfade, Spring};
use crate::palette::Palette;
use crate::route::{Nav, Route};
use crate::search::{SearchState, Submit};
use crate::settings::UiSettings;

mod event;
mod reduce;
#[cfg(test)]
mod screen_tests;
#[cfg(test)]
mod tests;

pub use event::{
    Effect, Event, FAILURE_TOAST_MS, LyricsState, QueueOp, ResolveKind, SKIP_TOAST_MS, Toast,
};

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
}
