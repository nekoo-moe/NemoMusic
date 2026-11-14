//! The library service the UI uses: folders, scan, track details, local playlists, M3U and
//! tag edits. `nemo-library` implements [`LocalLibraryService`]. No I/O here.

use std::path::{Path, PathBuf};

use crate::BoxFuture;
use crate::track::{Track, TrackId};

/// Why a listed file cannot be played.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnplayableReason {
    /// The player has no decoder for its format.
    UnsupportedFormat,
    /// Decoding failed.
    DecodeFailed,
    /// The file could not be read.
    Unreadable,
}

/// Whether a library track can be played.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TrackStatus {
    /// The player can decode it.
    #[default]
    Playable,
    /// Listed with its file name, but it cannot be played.
    Unplayable(UnplayableReason),
    /// The file is gone since the last scan.
    Missing,
}

/// Everything the library knows about one local track, for the details view and tag edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTrackDetails {
    /// The track as the catalog lists it.
    pub track: Track,
    /// Album artist, if tagged.
    pub album_artist: Option<String>,
    /// Disc number, if tagged.
    pub disc: Option<u32>,
    /// Track number, if tagged.
    pub number: Option<u32>,
    /// Year, if tagged.
    pub year: Option<u32>,
    /// Genre, if tagged.
    pub genre: Option<String>,
    /// Whether it can be played.
    pub status: TrackStatus,
}

/// Id of a local playlist in the library database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalPlaylistId(pub i64);

/// One entry of a local playlist: a local file (`path`), or a streamed song saved by
/// playlist-import or a stats recap (`path: None` and its `track`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistEntry {
    /// The local file, if the entry is one.
    pub path: Option<PathBuf>,
    /// The track, when the library or the saved song knows it.
    pub track: Option<Track>,
    /// The local file is gone; the entry stays and the player skips it.
    pub missing: bool,
}

/// A local playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPlaylist {
    /// Its id.
    pub id: LocalPlaylistId,
    /// Its name.
    pub name: String,
    /// Its entries, in order.
    pub entries: Vec<PlaylistEntry>,
}

/// The result of an M3U or M3U8 import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct M3uImport {
    /// The new playlist.
    pub playlist: LocalPlaylistId,
    /// Entries added.
    pub added: usize,
    /// Lines that could not be resolved and were skipped.
    pub skipped: usize,
}

/// A tag edit. `None` leaves a field unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TagEdit {
    /// New title.
    pub title: Option<String>,
    /// New artists.
    pub artists: Option<Vec<String>>,
    /// New album.
    pub album: Option<String>,
    /// New album artist.
    pub album_artist: Option<String>,
    /// New track number.
    pub number: Option<u32>,
    /// New disc number.
    pub disc: Option<u32>,
    /// New year.
    pub year: Option<u32>,
    /// New genre.
    pub genre: Option<String>,
}

/// One field a tag edit changes: the old and new value, shown before the user confirms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDiff {
    /// The field name, such as `"title"`.
    pub field: &'static str,
    /// The value now.
    pub old: String,
    /// The value after the edit.
    pub new: String,
}

/// What a finished scan changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScanSummary {
    /// Tracks added.
    pub added: usize,
    /// Tracks read again because their size or modified time changed.
    pub updated: usize,
    /// Tracks removed because their files are gone.
    pub removed: usize,
    /// Tracks listed as unplayable.
    pub unplayable: usize,
}

/// Progress of a background scan, for the library screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanEvent {
    /// A scan started.
    Started,
    /// Files seen so far; the found tracks are already in the catalog.
    Progress {
        /// Audio files seen so far.
        seen: usize,
    },
    /// The scan ended.
    Finished(ScanSummary),
    /// The library file was damaged: a copy was kept, a new one was made and a rescan runs.
    Rebuilt,
}

/// Receives scan events without blocking. The UI polls it once per frame.
pub trait ScanReceiver: Send {
    /// The next event, or `None` when there is none right now.
    fn try_next(&mut self) -> Option<ScanEvent>;
}

/// A library request failed. Texts hold no path and no tag value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LibraryError {
    /// The library database failed.
    #[error("the library database failed")]
    Db,
    /// A file could not be read or written, by its I/O error kind.
    #[error("a file could not be read or written ({0:?})")]
    Io(std::io::ErrorKind),
    /// A tag write failed and the file was left unchanged. The reason is fixed text.
    #[error("the tags were not written: {0}")]
    Tag(&'static str),
    /// A CUE sheet could not be read.
    #[error("the CUE sheet could not be read")]
    Cue,
    /// An M3U or M3U8 file could not be read or written.
    #[error("the playlist file could not be read or written")]
    M3u,
    /// The library file is damaged; a copy is kept and the library is rebuilt.
    #[error("the library file was damaged; a copy was kept and the library is being rebuilt")]
    Corrupt,
    /// Another library job is running; try again when it ends.
    #[error("the library is busy")]
    Busy,
}

/// The local library as the UI uses it.
pub trait LocalLibraryService: Send + Sync {
    /// The watched folders.
    fn folders(&self) -> Vec<PathBuf>;

    /// Watch a folder and scan it in the background.
    ///
    /// # Errors
    /// A [`LibraryError`] when the folder cannot be stored.
    fn add_folder(&self, path: PathBuf) -> BoxFuture<'_, Result<(), LibraryError>>;

    /// Stop watching a folder and remove its tracks from the library. No file is deleted.
    ///
    /// # Errors
    /// A [`LibraryError`] when the change cannot be stored.
    fn remove_folder<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<(), LibraryError>>;

    /// Scan every watched folder again.
    ///
    /// # Errors
    /// [`LibraryError::Busy`] while a scan runs, or another [`LibraryError`].
    fn rescan(&self) -> BoxFuture<'_, Result<ScanSummary, LibraryError>>;

    /// A new receiver of scan events.
    fn subscribe_scan(&self) -> Box<dyn ScanReceiver>;

    /// Everything the library knows about a track, or `None` for a track it does not hold.
    fn details(&self, id: &TrackId) -> Option<LocalTrackDetails>;

    /// Every local playlist.
    fn playlists(&self) -> Vec<LocalPlaylist>;

    /// Create an empty playlist.
    ///
    /// # Errors
    /// A [`LibraryError`] when it cannot be stored.
    fn create_playlist<'a>(
        &'a self,
        name: &'a str,
    ) -> BoxFuture<'a, Result<LocalPlaylistId, LibraryError>>;

    /// Rename a playlist.
    ///
    /// # Errors
    /// A [`LibraryError`] when it cannot be stored.
    fn rename_playlist<'a>(
        &'a self,
        id: LocalPlaylistId,
        name: &'a str,
    ) -> BoxFuture<'a, Result<(), LibraryError>>;

    /// Delete a playlist. The files of its entries are not touched.
    ///
    /// # Errors
    /// A [`LibraryError`] when it cannot be stored.
    fn delete_playlist(&self, id: LocalPlaylistId) -> BoxFuture<'_, Result<(), LibraryError>>;

    /// Move the entry at index `from` to index `to`.
    ///
    /// # Errors
    /// A [`LibraryError`] when it cannot be stored.
    fn move_entry(
        &self,
        id: LocalPlaylistId,
        from: usize,
        to: usize,
    ) -> BoxFuture<'_, Result<(), LibraryError>>;

    /// Add tracks to the end of a playlist (playlist-import, listening-stats recaps). Returns
    /// how many were added.
    ///
    /// # Errors
    /// A [`LibraryError`] when they cannot be stored.
    fn add_tracks(
        &self,
        id: LocalPlaylistId,
        tracks: Vec<Track>,
    ) -> BoxFuture<'_, Result<usize, LibraryError>>;

    /// Add local files to the end of a playlist. Returns how many were added.
    ///
    /// # Errors
    /// A [`LibraryError`] when they cannot be stored.
    fn add_files(
        &self,
        id: LocalPlaylistId,
        paths: Vec<PathBuf>,
    ) -> BoxFuture<'_, Result<usize, LibraryError>>;

    /// Remove the entry at `index`.
    ///
    /// # Errors
    /// A [`LibraryError`] when the change cannot be stored.
    fn remove_entry(
        &self,
        id: LocalPlaylistId,
        index: usize,
    ) -> BoxFuture<'_, Result<(), LibraryError>>;

    /// Import an M3U or M3U8 file as a new playlist; unresolved lines are skipped and counted.
    ///
    /// # Errors
    /// [`LibraryError::M3u`] when the file cannot be read, or another [`LibraryError`].
    fn import_m3u<'a>(&'a self, file: &'a Path) -> BoxFuture<'a, Result<M3uImport, LibraryError>>;

    /// Export a playlist as UTF-8 M3U8 to `to`. Entries without a path are left out.
    ///
    /// # Errors
    /// [`LibraryError::M3u`] when the file cannot be written, or another [`LibraryError`].
    fn export_m3u8<'a>(
        &'a self,
        id: LocalPlaylistId,
        to: &'a Path,
    ) -> BoxFuture<'a, Result<(), LibraryError>>;

    /// The changes a tag edit would make, for the user to confirm. Writes nothing.
    ///
    /// # Errors
    /// A [`LibraryError`] when the track or its tags cannot be read.
    fn preview_tag_edit<'a>(
        &'a self,
        id: &'a TrackId,
        edit: &'a TagEdit,
    ) -> BoxFuture<'a, Result<Vec<TagDiff>, LibraryError>>;

    /// Write a confirmed tag edit to the file and update the library. On failure the file is
    /// unchanged.
    ///
    /// # Errors
    /// [`LibraryError::Tag`] when the write fails, or another [`LibraryError`].
    fn apply_tag_edit<'a>(
        &'a self,
        id: &'a TrackId,
        edit: TagEdit,
    ) -> BoxFuture<'a, Result<(), LibraryError>>;

    /// Switch the background loudness scan of untagged files on or off.
    fn set_loudness_scan(&self, on: bool);
}
