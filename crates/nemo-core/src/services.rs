//! Service contracts that connect the UI, the player and the network crates.
//!
//! `nemo-app` supplies the real implementations. Every async method is runtime-agnostic: the
//! implementation runs its work on its own runtime and returns a future that any executor can
//! poll.

use std::io::{Read, Seek};
use std::sync::Arc;
use std::time::Duration;

use crate::BoxFuture;
use crate::catalog::Catalog;
use crate::lyrics::{LyricsDoc, LyricsQuery, ProviderId};
use crate::player::{PlayerCommand, PlayerEvent, PlayerSnapshot};
use crate::stream::{ResolvedStream, StreamError};
use crate::track::Artwork;

/// A seekable stream of bytes, for example an HTTP resource read with range requests.
/// Reads may block: the decode thread calls them, never the audio callback.
pub trait ByteSource: Read + Seek + Send + Sync {
    /// Total size in bytes, if the source knows it.
    fn byte_len(&self) -> Option<u64>;
}

/// Opens a resolved stream for reading.
pub trait StreamOpener: Send + Sync {
    /// Open the stream. This call may block; call it from a worker thread.
    ///
    /// # Errors
    /// Returns a [`StreamError`] when the stream cannot be opened.
    fn open(&self, stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError>;
}

/// Receives player events without blocking. The UI polls it once per frame.
pub trait EventReceiver: Send {
    /// The next event, or `None` when there is none right now.
    fn try_next(&mut self) -> Option<PlayerEvent>;
}

/// Control and read access to the player.
pub trait PlayerControl: Send + Sync {
    /// Send a command. Never blocks.
    fn send(&self, command: PlayerCommand);

    /// Position in the current track at the speakers: frames written minus output latency.
    /// A cheap atomic read, safe to call every frame.
    fn position(&self) -> Duration;

    /// The player state now.
    fn snapshot(&self) -> PlayerSnapshot;

    /// A new event receiver. Each receiver gets every event from the moment it is created.
    fn subscribe(&self) -> Box<dyn EventReceiver>;
}

/// The result of a lyrics lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsLookup {
    /// The best lyrics found, if any.
    pub doc: Option<LyricsDoc>,
    /// Providers that were asked, in chain order. Shown when nothing was found.
    pub tried: Vec<ProviderId>,
}

/// Finds lyrics through the provider chain and keeps per-track offsets.
pub trait LyricsService: Send + Sync {
    /// Look up lyrics. Failures of single providers do not fail the lookup.
    fn lookup<'a>(&'a self, query: &'a LyricsQuery) -> BoxFuture<'a, LyricsLookup>;

    /// The user's timing offset for a track, in milliseconds (0 when none is stored).
    fn offset_ms(&self, track_key: &str) -> i64;

    /// Store the user's timing offset for a track.
    fn set_offset_ms(&self, track_key: &str, offset_ms: i64);
}

/// Loading cover art failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    /// There is no artwork to load.
    #[error("no artwork")]
    Missing,
    /// The network request failed.
    #[error("network error: {0}")]
    Network(String),
    /// A local file could not be read.
    #[error("could not read artwork: {0}")]
    Read(String),
}

/// Loads cover art bytes (JPEG, PNG or WebP). Implementations cache on disk.
pub trait ImageSource: Send + Sync {
    /// Load the image for a slot `target_px` wide, picking the size with
    /// [`pick_thumbnail`](crate::track::pick_thumbnail) so a cover is never upscaled.
    fn image<'a>(
        &'a self,
        artwork: &'a Artwork,
        target_px: u32,
    ) -> BoxFuture<'a, Result<Vec<u8>, ImageError>>;
}

/// Everything the UI needs from the rest of the app.
#[derive(Clone)]
pub struct Backend {
    /// Search and browse.
    pub catalog: Arc<dyn Catalog>,
    /// Playback control.
    pub player: Arc<dyn PlayerControl>,
    /// Lyrics.
    pub lyrics: Arc<dyn LyricsService>,
    /// Cover art.
    pub images: Arc<dyn ImageSource>,
}
