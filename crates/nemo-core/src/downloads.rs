//! Offline cache contracts: saving songs for playback with no network (offline-cache design).
//!
//! `nemo-ytm` implements [`DownloadService`] and [`OfflineStore`]; `nemo-app` owns the shared
//! [`NetworkMode`] flag. Saved songs play only inside `NemoMusic`. Nothing here does I/O, and no
//! type prints a file path, URL or video id in its error text.

use std::path::PathBuf;

use crate::BoxFuture;
use crate::audio::StreamInfo;
use crate::catalog::BrowseId;
use crate::secret::Secret;
use crate::segment::Segment;
use crate::track::{Track, VideoId};

/// Why a song is in the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaveReason {
    /// The user saved it.
    User,
    /// Smart downloads kept it because it is played often. Evicted first.
    Smart,
}

/// An album or playlist that was saved as a whole.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CollectionRef {
    /// An album page.
    Album(BrowseId),
    /// A playlist page.
    Playlist(BrowseId),
}

/// Why saving a song failed. Shown to the user as plain text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FailReason {
    /// The size limit of the cache is reached.
    LimitReached,
    /// The disk has too little free space.
    DiskFull,
    /// YouTube no longer offers the song.
    NotAvailable,
    /// The network request failed.
    Network,
    /// YouTube asked the app to slow down.
    RateLimited,
    /// The saved file did not match what the server sent.
    Damaged,
}

/// Where one song stands in the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SaveState {
    /// Not in the cache and not queued.
    #[default]
    NotSaved,
    /// Waiting in the queue.
    Queued,
    /// Being downloaded.
    Saving {
        /// Bytes written so far.
        done: u64,
        /// Total bytes, once the server said.
        total: Option<u64>,
    },
    /// In the cache and playable with no network.
    Saved,
    /// Saving failed; the user may retry.
    Failed(FailReason),
}

/// Songs to save, optionally as one album or playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRequest {
    /// The songs, in order.
    pub tracks: Vec<Track>,
    /// The album or playlist they belong to, when the user saved one as a whole.
    pub collection: Option<CollectionRef>,
}

/// How much space the cache uses, for the storage view.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StorageReport {
    /// Bytes used by saved songs and covers.
    pub used_bytes: u64,
    /// Free bytes on the disk that holds the cache.
    pub free_bytes: u64,
    /// The size limit the user set.
    pub limit_bytes: u64,
    /// Bytes used by smart downloads.
    pub smart_bytes: u64,
    /// Bytes used by covers.
    pub art_bytes: u64,
    /// Each saved album or playlist with its title and bytes.
    pub collections: Vec<(CollectionRef, String, u64)>,
}

/// Why a song left the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RemovedReason {
    /// The user removed it.
    User,
    /// YouTube removed or blocked the song.
    RemovedByYoutube,
    /// The saved file was damaged.
    Damaged,
    /// Smart downloads made room for a song played more often.
    SmartEviction,
}

/// Why the download queue stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PauseReason {
    /// The user paused it.
    User,
    /// Five downloads in a row failed; the user resumes by hand.
    RepeatedFailures,
}

/// Something changed in the cache. The UI updates its marks from these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadEvent {
    /// A song changed state.
    State {
        /// The song.
        video: VideoId,
        /// Its new state.
        state: SaveState,
    },
    /// A song left the cache.
    Removed {
        /// The song.
        video: VideoId,
        /// Why.
        reason: RemovedReason,
    },
    /// The queue stopped.
    QueuePaused(PauseReason),
}

/// A cache operation failed. Messages hold no URL, file name or video id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DownloadError {
    /// The cache index could not be read or written.
    #[error("the offline index failed")]
    Db,
    /// A cache file could not be read or written.
    #[error("a file of the offline cache could not be read or written")]
    Io,
    /// The stream address could not be found.
    #[error("the song could not be found for saving")]
    Resolve,
    /// The download request failed.
    #[error("the download failed")]
    Http,
    /// The size limit of the cache is reached.
    #[error("the offline cache is full")]
    LimitReached,
    /// The disk has too little free space.
    #[error("the disk is full")]
    DiskFull,
    /// A saved file is damaged.
    #[error("a saved song is damaged")]
    Damaged,
}

/// Receives cache events without blocking. The UI polls it once per frame.
pub trait DownloadReceiver: Send {
    /// The next event, or `None` when there is none right now.
    fn try_next(&mut self) -> Option<DownloadEvent>;
}

/// Saves songs for offline playback, one at a time and slowly.
pub trait DownloadService: Send + Sync {
    /// Queue songs for saving.
    ///
    /// # Errors
    /// Returns a [`DownloadError`] when the queue cannot be stored.
    fn save(&self, request: SaveRequest) -> BoxFuture<'_, Result<(), DownloadError>>;

    /// The state of one song. Cheap; safe to call every frame.
    fn state(&self, video: &VideoId) -> SaveState;

    /// Stop downloading at once. The partial file is kept.
    fn pause(&self);

    /// Continue the queue from where it stopped.
    fn resume(&self);

    /// Try one failed song again.
    fn retry(&self, video: &VideoId);

    /// Remove one song from the cache.
    fn remove_track(&self, video: &VideoId);

    /// Remove an album or playlist. Songs another collection or the user still holds stay.
    fn remove_collection(&self, collection: &CollectionRef);

    /// Remove every saved song. Returns the freed bytes.
    ///
    /// # Errors
    /// Returns a [`DownloadError`] when a file or the index cannot be removed.
    fn clear_all(&self) -> BoxFuture<'_, Result<u64, DownloadError>>;

    /// How much space the cache uses.
    fn storage(&self) -> StorageReport;

    /// Set the size limit in bytes.
    fn set_limit(&self, bytes: u64);

    /// Switch smart downloads on or off.
    fn set_smart(&self, on: bool);

    /// A new event receiver. Each receiver gets every event from the moment it is created.
    fn subscribe(&self) -> Box<dyn DownloadReceiver>;
}

/// A saved song, ready to open. The path is a [`Secret`] so it never reaches logs or the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedAudio {
    /// The cached file. Only the player's prepare step exposes it.
    pub path: Secret<PathBuf>,
    /// Codec and bitrate of the cached file, as YouTube served it.
    pub info: StreamInfo,
}

/// Read access to the cache for the player, the segment source and the play hub.
pub trait OfflineStore: Send + Sync {
    /// The saved file of a song, if it is saved.
    fn open(&self, video: &VideoId) -> Option<CachedAudio>;

    /// The `SponsorBlock` segments stored with a saved song.
    fn segments(&self, video: &VideoId) -> Option<Vec<Segment>>;

    /// The saved file is missing or unreadable: mark the song as not saved.
    fn mark_unsaved(&self, video: &VideoId);

    /// Count one finished play, for smart downloads.
    fn record_play(&self, video: &VideoId);

    /// Remove the whole cache (songs, covers, index, play counts, queue). Returns the number of
    /// files removed.
    ///
    /// # Errors
    /// Returns a [`DownloadError`] when a file or the index cannot be removed.
    fn wipe(&self) -> Result<u64, DownloadError>;
}

/// The app-wide offline switch. When on, nothing asks a YouTube host.
pub trait NetworkMode: Send + Sync {
    /// Whether offline mode is on.
    fn is_offline(&self) -> bool;

    /// Switch offline mode on or off.
    fn set_offline(&self, on: bool);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{CachedAudio, DownloadError};
    use crate::audio::{Codec, Origin, StreamInfo};
    use crate::secret::Secret;

    #[test]
    fn cached_audio_debug_prints_no_path() {
        let audio = CachedAudio {
            path: Secret::new(PathBuf::from("C:/cache/blobs/dQw4w9WgXcQ.webm")),
            info: StreamInfo {
                origin: Origin::YoutubeMusic,
                codec: Codec::Opus,
                bitrate_kbps: Some(160),
                format: None,
            },
        };
        let text = format!("{audio:?}");
        assert!(
            !text.contains("blobs") && !text.contains("dQw4w9WgXcQ"),
            "{text}"
        );
        assert!(text.contains("[redacted]"), "{text}");
    }

    #[test]
    fn error_texts_hold_no_url_file_name_or_id() {
        for err in [
            DownloadError::Db,
            DownloadError::Io,
            DownloadError::Resolve,
            DownloadError::Http,
            DownloadError::LimitReached,
            DownloadError::DiskFull,
            DownloadError::Damaged,
        ] {
            let text = err.to_string();
            assert!(
                !text.contains("://") && !text.contains('/') && !text.contains('\\'),
                "{text}"
            );
            assert!(!text.contains('.'), "{text}");
        }
    }
}
