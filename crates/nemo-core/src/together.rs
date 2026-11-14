//! The contract of Listen Together: the room, its members and its events.
//!
//! No I/O here. `nemo-together` implements [`RoomControl`]; the UI creates, joins and runs a
//! room through it. Playback while a room is live goes through the [`RemotePlayback`]
//! supertrait. [`RoomRole`] comes from the `remote` module.

use crate::BoxFuture;
use crate::remote::{RemoteKind, RemotePlayback, RoomRole};

/// The connection to the room server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LinkState {
    /// No room.
    #[default]
    Off,
    /// Connecting for the first time.
    Connecting,
    /// Connected to the room.
    Connected,
    /// The connection dropped; trying again.
    Reconnecting {
        /// This attempt, from 1.
        attempt: u8,
        /// The most attempts before giving up.
        of: u8,
    },
    /// The connection failed and the room was left.
    Failed,
}

/// A member's id as the room server gives it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberId(pub String);

/// One member of a room, or one waiting to join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// The member's id.
    pub id: MemberId,
    /// The display name the member chose.
    pub name: String,
    /// The member hosts the room.
    pub is_host: bool,
    /// The member is connected now.
    pub connected: bool,
}

/// The room as it stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoomStatus {
    /// The connection to the server.
    pub link: LinkState,
    /// The room code, while in a room.
    pub code: Option<String>,
    /// This app's role, while in a room.
    pub role: Option<RoomRole>,
    /// The members, host first.
    pub members: Vec<Member>,
    /// Members waiting for the host's approval.
    pub pending: Vec<Member>,
    /// The server host in use, while in a room.
    pub server: Option<String>,
    /// The host lets everyone in without asking.
    pub auto_approve: bool,
}

/// Something that happened in the room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoomEvent {
    /// The status changed; read it again with [`RoomControl::status`].
    StatusChanged,
    /// Someone asks to join (host only).
    JoinRequested(Member),
    /// The host rejected this app's request to join.
    JoinRejected {
        /// The host's reason, as received.
        reason: String,
    },
    /// The host removed this app from the room.
    Kicked {
        /// The host's reason, as received.
        reason: String,
    },
    /// Another member hosts the room now.
    HostChanged {
        /// The new host.
        new_host: Member,
    },
    /// The room's track cannot be played here.
    CannotPlay {
        /// The track title.
        title: String,
    },
    /// The server sent a message for the user, shown as received.
    ServerMessage(String),
    /// This app left the room.
    Left,
}

/// Room events, read without blocking.
pub trait RoomEventReceiver: Send {
    /// The next event, if one is waiting. Never blocks.
    fn try_next(&mut self) -> Option<RoomEvent>;
}

/// A room request failed. The texts hold no display name, room code or session token.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RoomError {
    /// The server address is not allowed (`wss://`, or `ws://` on the local network only).
    #[error("the server address is not allowed")]
    BadAddress,
    /// The server cannot be reached.
    #[error("cannot reach {host}")]
    Unreachable {
        /// The server host the user typed.
        host: String,
    },
    /// The server refused the request.
    #[error("the server refused: {message}")]
    Refused {
        /// The server's text, as received.
        message: String,
    },
    /// No room has this code.
    #[error("the room was not found")]
    RoomNotFound,
    /// The room is full.
    #[error("the room is full")]
    RoomFull,
    /// The host removed this app from the room.
    #[error("removed from the room")]
    Kicked,
    /// Only the host can do this.
    #[error("only the host can do this")]
    NotHost,
    /// The server sent a message that could not be read.
    #[error("the server sent an unreadable message")]
    BadMessage,
    /// Another remote playback (Cast) runs.
    #[error("another remote playback is running")]
    Busy(RemoteKind),
    /// Local storage (the block list) failed.
    #[error("room storage failed")]
    Storage,
}

/// A Listen Together room: create, join, run and leave it.
pub trait RoomControl: RemotePlayback {
    /// The room as it stands.
    fn status(&self) -> RoomStatus;

    /// A receiver of room events from now on.
    fn subscribe_room(&self) -> Box<dyn RoomEventReceiver>;

    /// Create a room on `server` and host it as `display_name`.
    ///
    /// # Errors
    /// A [`RoomError`] when the address is not allowed, the server cannot be reached or
    /// refuses, or Cast runs.
    fn create<'a>(
        &'a self,
        server: &'a str,
        display_name: &'a str,
    ) -> BoxFuture<'a, Result<(), RoomError>>;

    /// Ask to join the room `code` on `server` as `display_name`.
    ///
    /// # Errors
    /// A [`RoomError`] when the address is not allowed, the server cannot be reached, the room
    /// is not found or full, the host rejects the request, or Cast runs.
    fn join<'a>(
        &'a self,
        server: &'a str,
        code: &'a str,
        display_name: &'a str,
    ) -> BoxFuture<'a, Result<(), RoomError>>;

    /// Let a waiting member in (host only).
    fn approve(&self, member: &MemberId);

    /// Turn a waiting member away with `reason` (host only).
    fn reject(&self, member: &MemberId, reason: &str);

    /// Remove a member with `reason` (host only).
    fn kick(&self, member: &MemberId, reason: &str);

    /// Let everyone in without asking, or ask again (host only).
    fn set_auto_approve(&self, on: bool);

    /// Never let a member with this display name in again; the list is stored.
    fn block_name(&self, name: &str);

    /// Leave the room; playback continues here.
    fn leave(&self) -> BoxFuture<'_, ()>;
}

#[cfg(test)]
mod tests {
    use super::{LinkState, RoomError, RoomStatus};
    use crate::remote::RemoteKind;

    #[test]
    fn errors_name_no_member_room_code_or_token() {
        let all = [
            RoomError::BadAddress,
            RoomError::Unreachable {
                host: "rooms.example.org".into(),
            },
            RoomError::Refused {
                message: "server is restarting".into(),
            },
            RoomError::RoomNotFound,
            RoomError::RoomFull,
            RoomError::Kicked,
            RoomError::NotHost,
            RoomError::BadMessage,
            RoomError::Busy(RemoteKind::Cast),
            RoomError::Storage,
        ];
        for e in all {
            let text = e.to_string();
            assert!(!text.is_empty());
            for bad in ["Alice", "ABC123", "token", "session"] {
                assert!(!text.contains(bad), "{text:?} contains {bad:?}");
            }
        }
        let host = RoomError::Unreachable {
            host: "rooms.example.org".into(),
        };
        assert_eq!(host.to_string(), "cannot reach rooms.example.org");
    }

    #[test]
    fn no_room_by_default() {
        let s = RoomStatus::default();
        assert_eq!(s.link, LinkState::Off);
        assert!(s.code.is_none() && s.role.is_none() && s.server.is_none());
        assert!(s.members.is_empty() && s.pending.is_empty() && !s.auto_approve);
    }
}
