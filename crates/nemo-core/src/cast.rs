//! The contract of Cast and DLNA playback: the device picker, the cast session and its events.
//!
//! No I/O here. `nemo-cast` implements [`CastControl`]; the UI reads the picker and starts or
//! stops casting through it. Positions and commands while casting go through the
//! [`RemotePlayback`] supertrait.

use std::time::Duration;

use crate::BoxFuture;
use crate::remote::{RemoteBusy, RemotePlayback};
use crate::track::Track;

/// A device found on the local network. Named so it does not clash with the output-devices
/// `DeviceId` (ADR 0004, row 5).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CastDeviceId(pub String);

/// What kind of device a picker entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceKind {
    /// This computer's own speakers.
    ThisComputer,
    /// A Google Cast device.
    Cast,
    /// A DLNA (`UPnP`) media renderer.
    Dlna,
}

/// One entry of the device picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastDevice {
    /// The device id.
    pub id: CastDeviceId,
    /// The name the device announces.
    pub name: String,
    /// The device kind.
    pub kind: DeviceKind,
    /// The device is playing for another sender.
    pub busy: bool,
}

/// Where the device search stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Discovery {
    /// The picker is closed and nothing searches.
    #[default]
    Closed,
    /// Searching the local network.
    Searching,
    /// The search ran its time; the list is final until the picker opens again.
    Finished,
}

/// Something that happened while casting. Texts hold no stream URL or token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastEvent {
    /// The device list changed; read it again with [`CastControl::devices`].
    DevicesChanged,
    /// The cast session is up and the device plays.
    Connected,
    /// The device did not fetch the stream in time; the firewall may block it.
    FirewallHint,
    /// The device refused the stream's format (no transcoding is done).
    FormatRejected {
        /// The codec or container the device refused.
        format: String,
    },
    /// The device finished the current track.
    TrackEnded,
    /// Casting failed and stopped.
    Failed {
        /// Short, user-safe reason.
        reason: String,
    },
    /// The device left the session; playback can continue here.
    Disconnected,
}

/// Cast events, read without blocking.
pub trait CastEventReceiver: Send {
    /// The next event, if one is waiting. Never blocks.
    fn try_next(&mut self) -> Option<CastEvent>;
}

/// Casting failed. The texts hold no stream URL, token or account data.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CastError {
    /// No device was found on the local network.
    #[error("no devices found")]
    NoDevices,
    /// The device is busy.
    #[error("the device is busy")]
    Busy {
        /// Another app plays on it (taking over stops that app).
        by_other_sender: bool,
    },
    /// The device cannot be reached.
    #[error("the device cannot be reached")]
    Unreachable,
    /// The secure connection to the device failed, or its certificate changed.
    #[error("the secure connection to the device failed")]
    TlsFailed,
    /// The device refused the stream's format.
    #[error("the device cannot play {format}")]
    FormatRejected {
        /// The codec or container the device refused.
        format: String,
    },
    /// A song saved on this computer cannot be cast.
    #[error("saved songs cannot be cast")]
    CachedSong,
    /// The track cannot be played.
    #[error("the track is not available")]
    NotAvailable,
    /// The local stream server could not open its port.
    #[error("the stream server could not start")]
    ServerBind,
    /// The device reported an error.
    #[error("the device reported error {code}")]
    DeviceError {
        /// The device's error code.
        code: u32,
    },
    /// Another remote playback (a room) runs.
    #[error("another remote playback is running")]
    Remote(RemoteBusy),
}

/// Cast and DLNA playback: the device picker, starting and stopping a cast session.
pub trait CastControl: RemotePlayback {
    /// The picker opened: start searching the local network.
    fn open_picker(&self);

    /// The picker closed: stop searching.
    fn close_picker(&self);

    /// Where the search stands.
    fn discovery(&self) -> Discovery;

    /// The devices found so far, "This computer" first.
    fn devices(&self) -> Vec<CastDevice>;

    /// Cast `track` to `device` from `at`. With `take_over` a busy device is taken from the
    /// other sender.
    ///
    /// # Errors
    /// A [`CastError`] when the device is busy, unreachable, refuses the format, or another
    /// remote playback runs.
    fn cast_to<'a>(
        &'a self,
        device: &'a CastDeviceId,
        take_over: bool,
        track: &'a Track,
        at: Duration,
    ) -> BoxFuture<'a, Result<(), CastError>>;

    /// Stop casting. Returns the device's last position, so playback continues here from it.
    fn stop_casting(&self) -> BoxFuture<'_, Option<Duration>>;

    /// A receiver of cast events from now on.
    fn subscribe_cast(&self) -> Box<dyn CastEventReceiver>;
}

#[cfg(test)]
mod tests {
    use super::{CastError, Discovery};
    use crate::remote::{RemoteBusy, RemoteKind};

    #[test]
    fn errors_name_no_url_or_token() {
        let all = [
            CastError::NoDevices,
            CastError::Busy {
                by_other_sender: true,
            },
            CastError::Unreachable,
            CastError::TlsFailed,
            CastError::FormatRejected {
                format: "opus/webm".into(),
            },
            CastError::CachedSong,
            CastError::NotAvailable,
            CastError::ServerBind,
            CastError::DeviceError { code: 714 },
            CastError::Remote(RemoteBusy(RemoteKind::Room)),
        ];
        for e in all {
            let text = e.to_string();
            assert!(!text.is_empty());
            for bad in ["http", "://", "token", "googlevideo", "?", "="] {
                assert!(!text.contains(bad), "{text:?} contains {bad:?}");
            }
        }
        assert_eq!(
            CastError::FormatRejected {
                format: "opus/webm".into()
            }
            .to_string(),
            "the device cannot play opus/webm"
        );
    }

    #[test]
    fn discovery_starts_closed() {
        assert_eq!(Discovery::default(), Discovery::Closed);
    }
}
