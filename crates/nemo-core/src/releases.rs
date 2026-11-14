//! New releases of the artists the signed-in account follows.
//!
//! No I/O here. `nemo-ytm` implements [`ReleaseFeed`] and keeps the notices on this computer;
//! the UI lists them and `nemo-app` sends one grouped OS notification for the ones not yet
//! announced.

use std::time::SystemTime;

use crate::BoxFuture;
use crate::catalog::{AlbumSummary, BrowseId, CatalogError};

/// One new release found by a release check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseNotice {
    /// The album or single.
    pub album: AlbumSummary,
    /// When the check found it.
    pub found_at: SystemTime,
    /// Whether the user has seen it in the release list.
    pub seen: bool,
}

/// The stored release notices of the active account.
pub trait ReleaseFeed: Send + Sync {
    /// Every stored notice.
    ///
    /// # Errors
    /// A [`CatalogError`] when the notices cannot be read.
    fn list(&self) -> BoxFuture<'_, Result<Vec<ReleaseNotice>, CatalogError>>;

    /// Mark the notices of these albums as seen.
    ///
    /// # Errors
    /// A [`CatalogError`] when the change cannot be stored.
    fn mark_seen<'a>(&'a self, ids: &'a [BrowseId]) -> BoxFuture<'a, Result<(), CatalogError>>;

    /// The new notices not yet sent to the OS, for one grouped OS notification; marks them as
    /// sent, so each is announced at most once.
    ///
    /// # Errors
    /// A [`CatalogError`] when the notices cannot be read or the change cannot be stored.
    fn take_unannounced(&self) -> BoxFuture<'_, Result<Vec<ReleaseNotice>, CatalogError>>;
}
