//! Remote playback shared by Cast and Listen Together: one remote at a time, and how player
//! commands reach it.
//!
//! No I/O here. `nemo-cast` and `nemo-together` implement [`RemotePlayback`]; `nemo-app` claims
//! the [`RemoteSlot`] before either starts, and asks [`RemotePlayback::gate`] about every UI
//! command while one runs.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use crate::audio::StreamInfo;
use crate::player::{PlayState, PlayerCommand};

/// Which kind of remote playback runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RemoteKind {
    /// Casting to a device on the local network.
    Cast,
    /// A Listen Together room.
    Room,
}

/// This app's role in a Listen Together room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoomRole {
    /// The host controls playback for the room.
    Host,
    /// A guest follows the host.
    Guest,
}

/// Where playback happens now.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RemoteMode {
    /// On this computer.
    #[default]
    Local,
    /// On a Cast or DLNA device.
    Cast {
        /// The device's name, as it shows it.
        device: String,
    },
    /// In a Listen Together room.
    Room {
        /// This app's role in the room.
        role: RoomRole,
    },
}

/// The state of the remote playback, as the remote reports it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RemoteStatus {
    /// Where playback happens.
    pub mode: RemoteMode,
    /// The playback state.
    pub state: PlayState,
    /// The position in the current track, if known.
    pub position: Option<Duration>,
    /// The length of the current track, if known.
    pub length: Option<Duration>,
    /// The remote volume in percent (0 to 100), if it has one.
    pub volume: Option<u8>,
    /// Facts about the stream the remote plays, if known.
    pub source: Option<StreamInfo>,
}

/// What happens to a player command while a remote runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandGate {
    /// Send it to the local player as usual (for example queue moves of a parked player).
    Allow,
    /// Give it to the remote through [`RemotePlayback::handle`] instead.
    Forward,
    /// Refuse it and show the reason.
    Deny {
        /// Plain text for the user.
        reason: &'static str,
    },
}

/// The [`RemoteSlot`] is held by this kind of remote playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("another remote playback is active ({0:?})")]
pub struct RemoteBusy(pub RemoteKind);

const FREE: u8 = 0;

const fn code(kind: RemoteKind) -> u8 {
    match kind {
        RemoteKind::Cast => 1,
        RemoteKind::Room => 2,
    }
}

const fn kind_of(code: u8) -> Option<RemoteKind> {
    match code {
        1 => Some(RemoteKind::Cast),
        2 => Some(RemoteKind::Room),
        _ => None,
    }
}

/// The one place a remote playback may run: at most one Cast or room at a time. Clones share
/// the slot.
#[derive(Debug, Clone, Default)]
pub struct RemoteSlot(Arc<AtomicU8>);

/// Holds the [`RemoteSlot`]; dropping it frees the slot.
#[derive(Debug)]
pub struct RemoteLease(Arc<AtomicU8>);

impl RemoteSlot {
    /// A free slot.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Take the slot for `kind`. It stays taken until the returned lease is dropped.
    ///
    /// # Errors
    /// [`RemoteBusy`] with the holder's kind while any remote playback holds the slot, also one
    /// of the same kind.
    pub fn claim(&self, kind: RemoteKind) -> Result<RemoteLease, RemoteBusy> {
        match self
            .0
            .compare_exchange(FREE, code(kind), Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => Ok(RemoteLease(Arc::clone(&self.0))),
            Err(held) => Err(RemoteBusy(kind_of(held).unwrap_or(kind))),
        }
    }

    /// The kind that holds the slot now, if any.
    #[must_use]
    pub fn active(&self) -> Option<RemoteKind> {
        kind_of(self.0.load(Ordering::Acquire))
    }
}

impl Drop for RemoteLease {
    fn drop(&mut self) {
        self.0.store(FREE, Ordering::Release);
    }
}

/// A remote playback (a Cast session or a room) as `nemo-app` drives it.
pub trait RemotePlayback: Send + Sync {
    /// The remote's state now. Cheap; safe to call every frame.
    fn remote_status(&self) -> RemoteStatus;

    /// What happens to this command while the remote runs.
    fn gate(&self, command: &PlayerCommand) -> CommandGate;

    /// Carry out a command that [`gate`](Self::gate) answered with [`CommandGate::Forward`].
    fn handle(&self, command: PlayerCommand);
}

#[cfg(test)]
mod tests {
    use super::{RemoteBusy, RemoteKind, RemoteMode, RemoteSlot, RemoteStatus};

    #[test]
    fn a_free_slot_is_claimed_and_freed_on_drop() {
        let slot = RemoteSlot::new();
        assert_eq!(slot.active(), None);
        let lease = slot.claim(RemoteKind::Cast);
        assert!(lease.is_ok());
        assert_eq!(slot.active(), Some(RemoteKind::Cast));
        drop(lease);
        assert_eq!(slot.active(), None);
        assert!(slot.claim(RemoteKind::Room).is_ok());
    }

    #[test]
    fn a_claim_while_busy_names_the_holder() {
        let slot = RemoteSlot::new();
        let other = slot.clone();
        let _room = slot.claim(RemoteKind::Room);
        assert_eq!(
            other.claim(RemoteKind::Cast).map(drop),
            Err(RemoteBusy(RemoteKind::Room))
        );
        assert_eq!(
            other.claim(RemoteKind::Room).map(drop),
            Err(RemoteBusy(RemoteKind::Room))
        );
        assert_eq!(other.active(), Some(RemoteKind::Room));
    }

    #[test]
    fn a_failed_claim_leaves_the_holder_in_place() {
        let slot = RemoteSlot::new();
        let cast = slot.claim(RemoteKind::Cast);
        let refused = slot.claim(RemoteKind::Room);
        assert!(refused.is_err());
        drop(refused);
        assert_eq!(slot.active(), Some(RemoteKind::Cast));
        drop(cast);
        assert_eq!(slot.active(), None);
    }

    #[test]
    fn the_default_status_is_local() {
        assert_eq!(RemoteStatus::default().mode, RemoteMode::Local);
    }
}
