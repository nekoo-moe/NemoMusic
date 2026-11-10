//! The UI state reducer. Pure: [`UiState::apply`] takes an [`Event`], changes the state and
//! returns [`Effect`]s (player commands, loads) for the GUI shell to run. No iced types here, so
//! the behaviour is unit-tested without a window.

use std::time::Duration;

use nemo_core::lyrics::{LyricsDoc, ProviderId};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent, PlayerSnapshot};
use nemo_core::services::LyricsLookup;
use nemo_core::track::{Track, TrackId};

use crate::keys::KeyAction;
use crate::lyrics_view::{self, FOLLOW_PAUSE_MS};
use crate::motion::{Crossfade, Spring};
use crate::palette::Palette;
use crate::route::{Nav, Route};

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
    /// A text field gained or lost focus.
    TextFocus(bool),
    /// Reset the lyrics offset to zero.
    LyricsOffsetReset,
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
    /// Text field focused.
    pub text_focused: bool,
    /// Honour reduced motion.
    pub reduced_motion: bool,
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
            text_focused: false,
            reduced_motion,
            last_active: None,
        }
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
    }

    /// Apply an event; returns the effects to run.
    pub fn apply(&mut self, event: Event) -> Vec<Effect> {
        let mut fx = Vec::new();
        match event {
            Event::Key(action) => self.key(action, &mut fx),
            Event::Player(_) => fx.push(Effect::RefreshSnapshot),
            Event::Snapshot(s) => self.snapshot(s, &mut fx),
            Event::Position(p) => self.position = p,
            Event::Frame { dt_ms } => self.frame(dt_ms),
            Event::Go(route) => {
                self.nav.go(route);
            }
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
            Event::TextFocus(on) => self.text_focused = on,
            Event::LyricsOffsetReset => self.set_offset(0, &mut fx),
        }
        fx
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
            KeyAction::OpenSearch => self.nav.go(Route::Search(String::new())),
            KeyAction::OpenSettings => self.nav.go(Route::Settings),
            KeyAction::Close => {
                if self.queue_open {
                    self.queue_open = false;
                } else {
                    self.nav.back();
                }
            }
            KeyAction::ToggleFrameReadout => fx.push(Effect::ToggleFrameReadout),
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
            self.lyrics = LyricsState::Loading;
            fx.push(Effect::LoadCover(track.clone()));
            fx.push(Effect::LoadLyrics(track));
        } else {
            self.lyrics = LyricsState::Idle;
            self.accent
                .retarget(Palette::fallback(), self.reduced_motion);
        }
    }

    fn frame(&mut self, dt_ms: f32) {
        let dt_ms = dt_ms.clamp(0.0, 100.0);
        self.accent.advance(dt_ms);
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
        self.lyric_scroll.target = lyrics_view::scroll_target(active, self.lyric_viewport_h);
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
