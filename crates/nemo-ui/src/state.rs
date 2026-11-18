//! The UI state reducer. Pure: [`UiState::apply`] takes an [`Event`], changes the state and
//! returns [`Effect`]s (player commands, loads) for the GUI shell to run. No iced types here, so
//! the behaviour is unit-tested without a window.

use std::time::Duration;

use nemo_core::player::{PlayState, PlayerCommand, PlayerSnapshot};
use nemo_core::track::Track;

use crate::lyrics_view::clock::Timeline;
use crate::lyrics_view::metrics::Metrics;
use crate::lyrics_view::scroll::Cascade;
use crate::lyrics_view::{self, FOLLOW_PAUSE_MS, enter, layout};
use crate::motion::Crossfade;
use crate::palette::Palette;
use crate::route::{Nav, Route};
use crate::search::{SearchState, Submit};
use crate::settings::UiSettings;
use nemo_core::lyrics::Granularity;

/// A change of the playback position larger than this is a seek: the lyrics jump, no cascade.
const SEEK_JUMP: Duration = Duration::from_secs(1);

#[cfg(test)]
mod calm_tests;
#[cfg(test)]
mod cascade_tests;
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
    pub lyric_scroll: Cascade,
    /// Remaining time of the manual-scroll pause, ms (0 = following).
    pub follow_pause_ms: f32,
    /// Lyrics viewport height, px.
    pub lyric_viewport_h: f32,
    /// Type size and spacing of the lyrics where they are shown (full window or right pane).
    pub lyric_metrics: Metrics,
    /// Wrapped row counts of the lyric lines, so scroll anchoring uses the real heights.
    pub lyric_rows: Vec<layout::Rows>,
    /// Top of every lyric slot (see [`layout::tops`]), kept in step with the rows and metrics.
    pub lyric_tops: Vec<f32>,
    /// Counts the documents shown; the list widget re-shapes its lines when it changes.
    pub lyrics_rev: u64,
    /// When each line of the current document starts and stops.
    pub lyric_timeline: Timeline,
    /// Time since the lyrics arrived, ms: the list slides in over [`enter::ENTER_MS`].
    pub lyric_enter_ms: f32,
    /// Time spent loading the lyrics, ms: it drives the shimmer of the loading line.
    pub lyric_loading_ms: f32,
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
    /// The first-run setup flow (startup-flow spec).
    pub setup: crate::setup::Setup,
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
    /// Lyrics Timing popover (offset tools) of the full-screen lyrics open.
    pub timing_open: bool,
    motion_option: bool,
    /// The position jumped since the last frame (a seek): the scroll jumps too.
    seeked: bool,
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
            lyric_scroll: Cascade::new(0.0),
            follow_pause_ms: 0.0,
            lyric_viewport_h: 480.0,
            lyric_metrics: Metrics::full(1366.0),
            lyric_rows: Vec::new(),
            lyric_tops: vec![0.0],
            lyrics_rev: 0,
            lyric_timeline: Timeline::default(),
            lyric_enter_ms: enter::ENTER_MS,
            lyric_loading_ms: 0.0,
            text_focused: false,
            reduced_motion,
            settings: UiSettings::default(),
            tos_checked: false,
            tos_dismissed: false,
            setup: crate::setup::Setup::default(),
            search: SearchState::default(),
            recent: Vec::new(),
            toast: None,
            menu: None,
            signal_open: false,
            timing_open: false,
            motion_option: reduced_motion,
            last_active: None,
            seeked: false,
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

    /// Open the setup flow when `on` (first run).
    #[must_use]
    pub fn with_setup(mut self, on: bool) -> Self {
        if on {
            self.setup.open();
        }
        self
    }

    /// `true` while the first-run notice covers the window. The setup flow shows the notice
    /// itself, so the sheet waits while the flow is open.
    #[must_use]
    pub fn tos_sheet_visible(&self) -> bool {
        !self.settings.tos_acknowledged && !self.tos_dismissed && !self.setup.active()
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

    /// What the synced lyrics centre on now: the line drawn as active, and the intro or gap
    /// note (lyrics-polish). Empty without synced lyrics.
    #[must_use]
    pub fn lyric_focus(&self) -> lyrics_view::note::Focus {
        match &self.lyrics {
            LyricsState::Ready(doc) => lyrics_view::note::focus(
                doc,
                i64::try_from(self.position.as_millis()).unwrap_or(i64::MAX),
                self.lyrics_offset_ms,
                self.reduced_motion,
            ),
            _ => lyrics_view::note::Focus::default(),
        }
    }

    /// The audio and scroll clocks of the current lyrics at the playback position (the track
    /// offset included); `None` without lyrics.
    #[must_use]
    pub fn lyric_clocks(&self) -> Option<lyrics_view::clock::Clocks> {
        let LyricsState::Ready(doc) = &self.lyrics else {
            return None;
        };
        let pos = i64::try_from(self.position.as_millis()).unwrap_or(i64::MAX);
        Some(lyrics_view::clock::Clocks::new(
            doc.granularity,
            pos,
            self.lyrics_offset_ms,
        ))
    }

    /// `true` while following the current line (no manual-scroll pause).
    #[must_use]
    pub fn lyrics_following(&self) -> bool {
        self.follow_pause_ms <= 0.0
    }

    /// `true` while the lyrics move: the loading shimmer, the list sliding in, the scroll
    /// cascade, and (only while a song plays) a line that is lit or dimming and the intro or gap
    /// note filling. Paused with nothing lit, it is `false`: the lyrics ask for no frame.
    #[must_use]
    pub fn lyrics_frames(&self) -> bool {
        match &self.lyrics {
            // The shimmer moves only while a song plays; paused, the line stands still.
            LyricsState::Loading => !self.reduced_motion && self.is_playing(),
            LyricsState::Ready(doc) if doc.granularity != Granularity::Unsynced => {
                let playing = self.is_playing()
                    && self.lyric_clocks().is_some_and(|c| {
                        self.lyric_timeline.busy(&c) || self.lyric_focus().note.is_some()
                    });
                playing || !self.lyric_scroll.is_settled() || self.lyric_enter_ms < enter::ENTER_MS
            }
            _ => false,
        }
    }

    /// Whether the GUI must subscribe to frames. Idle (paused, nothing animating) is `false`, so
    /// nothing redraws.
    #[must_use]
    pub fn needs_frames(&self) -> bool {
        self.is_playing()
            || !self.accent.is_done()
            || self.lyrics_frames()
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
            Event::Position(p) => {
                // A seek is a jump of more than a second; playback moves a frame at a time.
                if p.abs_diff(self.position) > SEEK_JUMP {
                    self.seeked = true;
                }
                self.position = p;
            }
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
                    self.lyrics_rev += 1;
                    // A found document slides in (reduced motion: it is just there).
                    self.lyric_enter_ms = if self.reduced_motion {
                        enter::ENTER_MS
                    } else {
                        0.0
                    };
                    self.lyric_timeline = match &self.lyrics {
                        LyricsState::Ready(doc) => Timeline::new(doc),
                        _ => Timeline::default(),
                    };
                    self.lyric_rows.clear();
                    self.rebuild_tops();
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
                let v = (self.lyric_scroll.target + px).max(self.scroll_floor());
                self.lyric_scroll.snap(v);
            }
            Event::LyricsFollow => {
                self.follow_pause_ms = 0.0;
                self.retarget_scroll();
            }
            Event::LyricsViewport(h) => {
                if (self.lyric_viewport_h - h).abs() > 0.5 {
                    self.lyric_viewport_h = h;
                    // A resize is not a line change: no cascade.
                    self.retarget_scroll();
                    self.lyric_scroll.snap(self.lyric_scroll.target);
                }
            }
            Event::LyricsMetrics(m) => {
                if self.lyric_metrics != m {
                    self.lyric_metrics = m;
                    self.rebuild_tops();
                    self.retarget_scroll();
                    self.lyric_scroll.snap(self.lyric_scroll.target);
                }
            }
            Event::LyricsRows(rows) => {
                if self.lyric_rows != rows {
                    self.lyric_rows = rows;
                    self.rebuild_tops();
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
            Event::Setup(ev) => self.setup_event(ev, &mut fx),
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
            Event::SearchFilter(kind) => {
                if let Some((query, kind)) = self.search.pick(kind) {
                    fx.push(Effect::RunFilteredSearch { query, kind });
                }
            }
            Event::SearchFilterDone {
                query,
                kind,
                result,
            } => self.search.filter_done(&query, kind, result),
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
            Event::ToggleSignalPath => {
                self.signal_open = !self.signal_open;
                self.timing_open = false;
            }
            Event::ToggleLyricsTiming => {
                self.timing_open = !self.timing_open;
                self.signal_open = false;
            }
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
