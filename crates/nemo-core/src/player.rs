//! Commands sent to the player and events it reports.
//!
//! The UI talks to the player only through these two types (`structure.md`).

use std::time::Duration;

use crate::audio::SignalPath;
use crate::track::{Track, TrackId};

/// Playback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    /// Nothing is playing and the queue position is reset.
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
    /// Start this track now.
    Play(Track),
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
}

/// Something that happened in the player. Names are past tense.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    /// The playback state changed.
    StateChanged(PlayState),
    /// A different track became current.
    TrackChanged(TrackId),
    /// The real signal path changed (new track, device or mode).
    SignalPathChanged(SignalPath),
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
