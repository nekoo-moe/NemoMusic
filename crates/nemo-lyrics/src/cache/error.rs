//! The error type of the lyrics cache.

/// A cache operation failed.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    /// SQLite reported an error.
    #[error("lyrics cache database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The cache directory could not be created.
    #[error("could not create the lyrics cache directory: {0}")]
    Io(#[from] std::io::Error),
    /// A stored body could not be encoded or decoded.
    #[error("lyrics cache body error: {0}")]
    Body(String),
}
