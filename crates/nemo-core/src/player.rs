//! Commands sent to the player and events it reports.
//!
//! The UI talks to the player only through these types (`structure.md`).

use std::time::Duration;

use crate::audio::SignalPath;
use crate::effects::{
    EffectsSettings, EffectsStatus, TransitionKind, TransitionSettings, TransitionSkipReason,
};
use crate::segment::Segment;
use crate::track::{Track, TrackId};

/// Playback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayState {
    /// Nothing is playing and the queue position is reset.
    #[default]
    Stopped,
    /// Audio is flowing to the device.
    Playing,
    /// Paused by the user.
    Paused,
    /// Waiting for data. Not an error.
    Buffering,
}

/// What happens when the queue reaches its end (or the current track ends).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RepeatMode {
    /// Stop after the last track.
    #[default]
    Off,
    /// Start again from the first track after the last one.
    All,
    /// Play the current track again and again.
    One,
}

impl RepeatMode {
    /// The mode a press of the repeat button selects: Off, All, One, Off.
    #[must_use]
    pub const fn cycled(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }
}

/// A request to the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerCommand {
    /// Play this track now. It becomes the whole queue.
    Play(Track),
    /// Replace the queue and start at `start` (an index into `tracks`).
    SetQueue {
        /// The new queue.
        tracks: Vec<Track>,
        /// Index of the track to start with.
        start: usize,
    },
    /// Add a track to the end of the queue.
    Enqueue(Track),
    /// Insert a track right after the current one.
    PlayNext(Track),
    /// Remove the queue entry at this index.
    RemoveAt(usize),
    /// Pause playback.
    Pause,
    /// Resume after a pause.
    Resume,
    /// Stop playback.
    Stop,
    /// Jump to a position in the current track.
    Seek(Duration),
    /// Go to the next track in the queue.
    Next,
    /// Go to the previous track in the queue.
    Previous,
    /// Set the volume in percent (0 to 100). At 100 the player applies no gain at all.
    SetVolume(u8),
    /// Switch SponsorBlock skipping on or off.
    SetSkipSegments(bool),
    /// Undo the last SponsorBlock skip: seek back and ignore that segment for this play.
    UndoSkip,
    /// Choose what happens at the end of the queue or of the current track.
    SetRepeat(RepeatMode),
    /// Hand playback to a remote device: stop decoding and release the output device. The
    /// queue and its position stay; Next, Previous and queue edits still move through it and
    /// report [`PlayerEvent::TrackChanged`], without decoding anything.
    Park,
    /// Take playback back from a remote device: reopen the output device and play the current
    /// track from `at`.
    Unpark {
        /// Where to resume in the current track (the remote device's position).
        at: Duration,
    },
    /// Play another audio track of the current video: `Some` names its language as a BCP 47
    /// tag, `None` asks for the video's original track. The player resolves the current track
    /// again with that language in its stream request and reopens it at the current position;
    /// the tracks that follow are resolved with it too. A local track ignores the command.
    SwitchAudioTrack(Option<String>),
    /// Apply these effects settings. The player answers with [`PlayerEvent::EffectsChanged`],
    /// carrying the settings it applies.
    SetEffects(EffectsSettings),
    /// Use these song-transition settings from the next song change on.
    SetTransition(TransitionSettings),
}

/// Something that happened in the player. Names are past tense.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    /// The playback state changed.
    StateChanged(PlayState),
    /// A different track became current.
    TrackChanged(TrackId),
    /// The real duration of the current track became known.
    DurationKnown(Duration),
    /// The real signal path changed (new track, device or mode).
    SignalPathChanged(SignalPath),
    /// The queue content or position changed. Read the new queue from the snapshot.
    QueueChanged,
    /// The volume changed (percent).
    VolumeChanged(u8),
    /// The repeat mode changed.
    RepeatChanged(RepeatMode),
    /// A SponsorBlock segment was skipped.
    SegmentSkipped(Segment),
    /// The audio callback ran out of data. `total` counts underruns since start.
    UnderrunsCounted {
        /// Underruns since the stream started.
        total: u64,
    },
    /// Playback failed. The message holds no secrets, URLs or file contents.
    Failed {
        /// Short, user-safe description.
        message: String,
    },
    /// The player was parked (`true`, a remote device plays) or unparked (`false`).
    ParkedChanged(bool),
    /// The effects the player applies changed (after a [`PlayerCommand::SetEffects`]).
    EffectsChanged(EffectsStatus),
    /// A transition into the next song began.
    TransitionStarted {
        /// The transition used for this pair of songs.
        kind: TransitionKind,
        /// Its length in seconds.
        seconds: u8,
    },
    /// The set transition did not run; the songs joined gaplessly. Not a failure.
    TransitionSkipped(TransitionSkipReason),
}

/// The player state at one moment. Cheap to clone; the UI reads it after a [`PlayerEvent`].
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerSnapshot {
    /// Playback state.
    pub state: PlayState,
    /// The current track, if any.
    pub current: Option<Track>,
    /// The whole queue, including the current track.
    pub queue: Vec<Track>,
    /// Index of the current track in `queue`.
    pub queue_index: Option<usize>,
    /// Real duration of the current track, if known.
    pub duration: Option<Duration>,
    /// Volume in percent (0 to 100).
    pub volume_percent: u8,
    /// The real signal path of the current track, if one is playing.
    pub signal_path: Option<SignalPath>,
    /// Underruns since the stream started.
    pub underruns: u64,
    /// Whether SponsorBlock skipping is on.
    pub skip_segments: bool,
    /// The repeat mode.
    pub repeat: RepeatMode,
    /// The current track is a live stream: it has no length and cannot be seeked.
    pub live: bool,
    /// The effects as the player applies them.
    pub effects: EffectsStatus,
    /// The song-transition settings in use.
    pub transition: TransitionSettings,
}

impl Default for PlayerSnapshot {
    fn default() -> Self {
        Self {
            state: PlayState::Stopped,
            current: None,
            queue: Vec::new(),
            queue_index: None,
            duration: None,
            volume_percent: 100,
            signal_path: None,
            underruns: 0,
            skip_segments: false,
            repeat: RepeatMode::Off,
            live: false,
            effects: EffectsStatus::default(),
            transition: TransitionSettings::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EffectsStatus, PlayState, PlayerSnapshot, RepeatMode, TransitionSettings};

    #[test]
    fn repeat_cycles_off_all_one_off() {
        let mut m = RepeatMode::default();
        assert_eq!(m, RepeatMode::Off);
        m = m.cycled();
        assert_eq!(m, RepeatMode::All);
        m = m.cycled();
        assert_eq!(m, RepeatMode::One);
        assert_eq!(m.cycled(), RepeatMode::Off);
    }

    #[test]
    fn a_new_snapshot_is_stopped_at_full_volume() {
        let s = PlayerSnapshot::default();
        assert_eq!(s.state, PlayState::Stopped);
        assert_eq!(s.volume_percent, 100);
        assert!(s.current.is_none() && s.queue.is_empty() && !s.skip_segments);
        assert!(!s.live);
        assert_eq!(s.effects, EffectsStatus::default());
        assert_eq!(s.transition, TransitionSettings::default());
    }
}
