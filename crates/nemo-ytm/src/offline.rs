//! The offline cache: songs saved for playback with no network (offline-cache design).
//!
//! Everything lives in one folder that `nemo-app` gives (`AppPaths.offline`): the SQLite index
//! `offline.db` and three subfolders, `blobs/` (saved songs), `tmp/` (downloads in progress, as
//! `.part` files) and `art/` (covers). Files are plain, with no encryption (owner decision),
//! and every file name is a random opaque name from [`opaque_name`], so a folder browse shows
//! no video id or title. Nothing here logs a path, URL or video id.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use nemo_core::secret::Secret;

mod store;

#[cfg(test)]
mod tests;

/// Source of the current time, so tests can move it.
pub trait Clock: Send + Sync {
    /// Seconds since the Unix epoch.
    fn now_secs(&self) -> i64;
}

/// The system clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_secs(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
    }
}

/// An offline cache operation failed. Texts are fixed: they hold no path, URL or video id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OfflineError {
    /// The index could not be opened, read or written.
    #[error("the offline index failed")]
    Db,
    /// A folder or file of the cache could not be created, read or written.
    #[error("a file of the offline cache could not be read or written")]
    Io,
    /// The system gave no random bytes for a file name.
    #[error("no file name could be made for the offline cache")]
    Random,
}

impl From<rusqlite::Error> for OfflineError {
    fn from(_: rusqlite::Error) -> Self {
        // Dropped on purpose: SQLite errors can quote values such as a video id.
        Self::Db
    }
}

impl From<std::io::Error> for OfflineError {
    fn from(_: std::io::Error) -> Self {
        // Dropped on purpose: I/O errors can name the path.
        Self::Io
    }
}

/// Length of an opaque file name: 16 random bytes as lowercase hex.
pub const OPAQUE_NAME_LEN: usize = 32;

/// A new random file name for the cache: 32 lowercase hex characters, unrelated to the song.
///
/// # Errors
/// Returns [`OfflineError::Random`] when the system gives no random bytes.
pub fn opaque_name() -> Result<String, OfflineError> {
    use std::fmt::Write as _;

    let mut bytes = [0_u8; OPAQUE_NAME_LEN / 2];
    getrandom::fill(&mut bytes).map_err(|_| OfflineError::Random)?;
    let mut name = String::with_capacity(OPAQUE_NAME_LEN);
    for byte in bytes {
        // Writing to a `String` cannot fail.
        let _ = write!(name, "{byte:02x}");
    }
    Ok(name)
}

/// Whether `name` has the shape [`opaque_name`] makes. Paths are built only from such names.
#[must_use]
pub fn is_opaque_name(name: &str) -> bool {
    name.len() == OPAQUE_NAME_LEN
        && name
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Which folder of the cache a file belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// A saved song, in `blobs/`.
    Song,
    /// A download in progress, in `tmp/` with the `.part` extension.
    Part,
    /// A cover, in `art/`.
    Art,
}

/// The offline cache: the index and the folders.
pub struct OfflineCache {
    root: PathBuf,
    store: store::Store,
}

impl std::fmt::Debug for OfflineCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OfflineCache").finish_non_exhaustive()
    }
}

impl OfflineCache {
    /// Open (or create) the cache in `dir`: the folders, then `offline.db`, running every
    /// migration it has not seen yet. `clock` stamps the migrations.
    ///
    /// # Errors
    /// Returns an [`OfflineError`] when a folder or the index cannot be created or migrated.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the design's signature: later offline-cache tasks keep the clock in the cache"
    )]
    pub fn open(dir: &Path, clock: Arc<dyn Clock>) -> Result<Self, OfflineError> {
        for folder in store::FOLDERS {
            std::fs::create_dir_all(dir.join(folder))?;
        }
        let store = store::Store::open(&dir.join(store::DB_FILE), clock.as_ref())?;
        Ok(Self {
            root: dir.to_path_buf(),
            store,
        })
    }

    /// Where a cache file with the opaque `name` lives. `None` when `name` is not an opaque
    /// name, so no path can carry a video id, a title or `..`.
    #[must_use]
    pub fn file_path(&self, kind: FileKind, name: &str) -> Option<Secret<PathBuf>> {
        if !is_opaque_name(name) {
            return None;
        }
        let path = match kind {
            FileKind::Song => self.root.join(store::BLOBS).join(name),
            FileKind::Part => self.root.join(store::TMP).join(format!("{name}.part")),
            FileKind::Art => self.root.join(store::ART).join(name),
        };
        Some(Secret::new(path))
    }

    /// How many songs the index lists as saved.
    ///
    /// # Errors
    /// Returns [`OfflineError::Db`] when the index cannot be read.
    pub fn saved_count(&self) -> Result<u64, OfflineError> {
        self.store.saved_count()
    }
}
