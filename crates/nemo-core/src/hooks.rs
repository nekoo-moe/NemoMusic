//! Player-event hooks shared by the services that watch playback (integrations, listening-stats).
//!
//! Pure, no I/O. [`PlayRule`] decides whether a play counts, the same way for Last.fm scrobbles
//! and for local listening stats, so both agree. [`PlayTracker`] turns player events and position
//! samples into [`PlayHookEvent`]s with the real heard time, which every [`PlayObserver`] reads.

use std::time::{Duration, Instant, SystemTime};

use crate::player::{PlayState, PlayerEvent, PlayerSnapshot};
use crate::track::Track;

/// A position change larger than this beyond the expected advance is a jump (a seek), not
/// playback. Covers the jitter of position samples taken every 500 ms.
const JUMP_TOLERANCE: Duration = Duration::from_secs(1);

/// One play of a track that ended: when it started, how long it was really heard, and whether
/// it counts under [`PlayRule`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedPlay {
    /// The track that was played.
    pub track: Track,
    /// Wall-clock time the play started.
    pub started_at: SystemTime,
    /// Time heard while playing; paused time and seeks are not counted.
    pub heard: Duration,
    /// The track length, if known.
    pub duration: Option<Duration>,
    /// Whether the play counts ([`PlayRule::counts`]).
    pub counts: bool,
}

/// A playback event for the observers, built by [`PlayTracker`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayHookEvent {
    /// A play of a track started.
    TrackStarted {
        /// The track.
        track: Track,
        /// Wall-clock time the play started.
        started_at: SystemTime,
    },
    /// The playback state changed.
    StateChanged {
        /// The new state.
        state: PlayState,
        /// The last known position in the track.
        position: Duration,
    },
    /// The position jumped (a seek forward or back); no heard time was added for it.
    PositionJumped {
        /// The new position.
        position: Duration,
    },
    /// The real length of the current track became known.
    DurationKnown(Duration),
    /// A play ended: the track changed (also on repeat one) or the tracker was flushed.
    TrackEnded(FinishedPlay),
}

/// Receives [`PlayHookEvent`]s from the hub. Must not block: push into an own channel and do
/// the work elsewhere.
pub trait PlayObserver: Send + Sync {
    /// Handle one event.
    fn on_event(&self, event: &PlayHookEvent);
}

#[derive(Debug)]
struct OpenPlay {
    track: Track,
    started_at: SystemTime,
    heard: Duration,
    duration: Option<Duration>,
}

/// Turns [`PlayerEvent`]s and position samples into [`PlayHookEvent`]s.
///
/// Heard time grows only while [`PlayState::Playing`], by the smaller of the position advance
/// and the wall time spent playing, so it never exceeds wall time. A forward jump beyond that
/// time is a seek and a backward jump resyncs; both report [`PlayHookEvent::PositionJumped`]
/// and add nothing. `TrackChanged` closes the old play and opens a new one, also when the same
/// track repeats; [`flush`](Self::flush) closes the last one.
#[derive(Debug, Default)]
pub struct PlayTracker {
    state: PlayState,
    play: Option<OpenPlay>,
    last_position: Option<Duration>,
    /// Start of the playing interval not yet credited; `Some` only while playing.
    playing_since: Option<Instant>,
    /// Playing time since the last sample from intervals that already ended (a pause).
    banked: Duration,
}

impl PlayTracker {
    /// A tracker with no open play, stopped.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one player event. `snap` is the snapshot read after it (it names the new track),
    /// `now` the monotonic time and `wall` the wall-clock time of the event.
    pub fn on_player_event(
        &mut self,
        ev: &PlayerEvent,
        snap: &PlayerSnapshot,
        now: Instant,
        wall: SystemTime,
        out: &mut Vec<PlayHookEvent>,
    ) {
        match ev {
            PlayerEvent::TrackChanged(id) => {
                self.close(now, out);
                let track = snap
                    .current
                    .iter()
                    .chain(&snap.queue)
                    .find(|t| &t.id == id)
                    .cloned();
                if let Some(track) = track {
                    self.last_position = Some(Duration::ZERO);
                    out.push(PlayHookEvent::TrackStarted {
                        track: track.clone(),
                        started_at: wall,
                    });
                    self.play = Some(OpenPlay {
                        duration: track.duration,
                        track,
                        started_at: wall,
                        heard: Duration::ZERO,
                    });
                }
            }
            PlayerEvent::StateChanged(state) => self.set_state(*state, now, out),
            PlayerEvent::DurationKnown(d) => {
                if let Some(play) = &mut self.play {
                    play.duration = Some(*d);
                }
                out.push(PlayHookEvent::DurationKnown(*d));
            }
            _ => {}
        }
    }

    /// Feed one position sample of the current track, taken at `now`.
    pub fn tick(&mut self, position: Duration, now: Instant, out: &mut Vec<PlayHookEvent>) {
        let playing = self.take_playing_time(now);
        if let Some(last) = self.last_position {
            let jumped_forward =
                position > last.saturating_add(playing).saturating_add(JUMP_TOLERANCE);
            let jumped_back = position.saturating_add(JUMP_TOLERANCE) < last;
            if jumped_forward || jumped_back {
                out.push(PlayHookEvent::PositionJumped { position });
            } else if let Some(play) = &mut self.play {
                play.heard = play
                    .heard
                    .saturating_add(position.saturating_sub(last).min(playing));
            }
        }
        self.last_position = Some(position);
    }

    /// Close the open play, if any (at exit).
    pub fn flush(&mut self, now: Instant, out: &mut Vec<PlayHookEvent>) {
        self.close(now, out);
    }

    fn set_state(&mut self, state: PlayState, now: Instant, out: &mut Vec<PlayHookEvent>) {
        let was_playing = self.state == PlayState::Playing;
        let is_playing = state == PlayState::Playing;
        if was_playing && !is_playing {
            if let Some(since) = self.playing_since.take() {
                self.banked = self
                    .banked
                    .saturating_add(now.saturating_duration_since(since));
            }
        } else if is_playing && !was_playing {
            self.playing_since = Some(now);
        }
        self.state = state;
        out.push(PlayHookEvent::StateChanged {
            state,
            position: self.last_position.unwrap_or_default(),
        });
    }

    /// Playing time since the last sample; restarts the count.
    fn take_playing_time(&mut self, now: Instant) -> Duration {
        let mut time = std::mem::take(&mut self.banked);
        if let Some(since) = self.playing_since {
            time = time.saturating_add(now.saturating_duration_since(since));
            self.playing_since = Some(now);
        }
        time
    }

    /// End the open play. Playing time since the last sample counts up to the track's end.
    fn close(&mut self, now: Instant, out: &mut Vec<PlayHookEvent>) {
        let playing = self.take_playing_time(now);
        let last = self.last_position.take();
        let Some(mut play) = self.play.take() else {
            return;
        };
        let rest = match (last, play.duration) {
            (Some(p), Some(d)) => d.saturating_sub(p),
            (Some(_), None) => playing,
            (None, _) => Duration::ZERO,
        };
        play.heard = play.heard.saturating_add(playing.min(rest));
        out.push(PlayHookEvent::TrackEnded(FinishedPlay {
            counts: PlayRule::counts(play.duration, play.heard),
            track: play.track,
            started_at: play.started_at,
            heard: play.heard,
            duration: play.duration,
        }));
    }
}

/// When a play counts: Last.fm's rule, also used for local listening stats.
///
/// A track longer than [`MIN_TRACK`](Self::MIN_TRACK) counts once it has been heard for half its
/// length or [`MAX_THRESHOLD`](Self::MAX_THRESHOLD), whichever comes first. A track of unknown
/// length counts at `MAX_THRESHOLD`. Paused time is not heard time; the caller measures `heard`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayRule;

impl PlayRule {
    /// 30 s: a track must be longer than this to count at all.
    pub const MIN_TRACK: Duration = Duration::from_secs(30);
    /// 4 min: a play never needs more heard time than this.
    pub const MAX_THRESHOLD: Duration = Duration::from_mins(4);

    /// Heard time after which a track of this length counts: `min(duration / 2, 4 min)`, or
    /// 4 min when the length is unknown. `None` when the track is 30 s or shorter, which never
    /// counts.
    #[must_use]
    pub fn threshold(duration: Option<Duration>) -> Option<Duration> {
        match duration {
            None => Some(Self::MAX_THRESHOLD),
            Some(d) if d <= Self::MIN_TRACK => None,
            Some(d) => Some((d / 2).min(Self::MAX_THRESHOLD)),
        }
    }

    /// Whether a play of a track of this length that was heard for `heard` counts.
    #[must_use]
    pub fn counts(duration: Option<Duration>, heard: Duration) -> bool {
        Self::threshold(duration).is_some_and(|t| heard >= t)
    }
}

#[cfg(test)]
mod tracker_tests;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::PlayRule;

    const fn s(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    #[test]
    fn threshold_table() {
        let cases = [
            (Some(s(29)), None),
            (Some(s(30)), None),
            (Some(s(31)), Some(Duration::from_millis(15_500))),
            (Some(s(8 * 60)), Some(s(240))),
            (Some(s(10 * 60)), Some(s(240))),
            (None, Some(s(240))),
        ];
        for (duration, want) in cases {
            assert_eq!(PlayRule::threshold(duration), want, "{duration:?}");
        }
    }

    #[test]
    fn counts_table() {
        let cases = [
            // 29 s and 30 s tracks never count, even heard in full.
            (Some(s(29)), s(29), false),
            (Some(s(30)), s(30), false),
            // 31 s: half is 15.5 s.
            (Some(s(31)), Duration::from_millis(15_499), false),
            (Some(s(31)), Duration::from_millis(15_500), true),
            // 8 min: half is 4 min, which is also the cap.
            (Some(s(8 * 60)), s(239), false),
            (Some(s(8 * 60)), s(240), true),
            // 20 min: the 4 min cap comes first.
            (Some(s(20 * 60)), s(240), true),
            // Unknown length: 4 min.
            (None, s(239), false),
            (None, s(240), true),
        ];
        for (duration, heard, want) in cases {
            assert_eq!(
                PlayRule::counts(duration, heard),
                want,
                "{duration:?} heard {heard:?}"
            );
        }
    }
}
