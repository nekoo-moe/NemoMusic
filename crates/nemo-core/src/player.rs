//! Commands sent to the player and events it reports.
//!
//! The UI talks to the player only through these types (`structure.md`).

use std::time::Duration;

use crate::audio::SignalPath;
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PlayState, PlayerSnapshot};

    #[test]
    fn a_new_snapshot_is_stopped_at_full_volume() {
        let s = PlayerSnapshot::default();
        assert_eq!(s.state, PlayState::Stopped);
        assert_eq!(s.volume_percent, 100);
        assert!(s.current.is_none() && s.queue.is_empty() && !s.skip_segments);
    }
}
