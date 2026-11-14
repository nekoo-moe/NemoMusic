//! The private reducer steps behind [`UiState::apply`]: navigation, keys, settings, queue, frames.

use std::time::Duration;

use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent, PlayerSnapshot};

use crate::home;
use crate::keys::KeyAction;
use crate::lyrics_view;
use crate::route::Route;
use crate::settings::SettingChange;

use super::{Effect, FAILURE_TOAST_MS, LyricsState, QueueOp, SKIP_TOAST_MS, Toast, UiState};

impl UiState {
    pub(super) fn go(&mut self, route: Route, fx: &mut Vec<Effect>) {
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

    pub(super) fn player_event(&mut self, event: &PlayerEvent) {
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

    pub(super) fn setting(&mut self, change: SettingChange, fx: &mut Vec<Effect>) {
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

    pub(super) fn queue_op(&mut self, op: QueueOp, fx: &mut Vec<Effect>) {
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

    pub(super) fn key(&mut self, action: KeyAction, fx: &mut Vec<Effect>) {
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

    pub(super) fn set_offset(&mut self, offset_ms: i64, fx: &mut Vec<Effect>) {
        if let Some(t) = &self.snapshot.current {
            self.lyrics_offset_ms = offset_ms;
            fx.push(Effect::SaveLyricsOffset {
                track_key: t.id.0.clone(),
                offset_ms,
            });
        }
    }

    pub(super) fn snapshot(&mut self, s: PlayerSnapshot, fx: &mut Vec<Effect>) {
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

    pub(super) fn frame(&mut self, dt_ms: f32) {
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

    pub(super) fn retarget_scroll(&mut self) {
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
