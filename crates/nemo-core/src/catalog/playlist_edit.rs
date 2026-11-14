//! Editing the signed-in user's own playlists on YouTube Music (account-actions).

use super::{BrowseId, CatalogError};
use crate::BoxFuture;
use crate::track::VideoId;

/// Id of one entry in a playlist. The same song can appear twice; this tells the two apart.
/// Treat it as untrusted text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetVideoId(pub String);

/// Who can see a playlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Privacy {
    /// Only the owner.
    Private,
    /// Anyone with the link.
    Unlisted,
    /// Everyone.
    Public,
}

/// What a playlist page needs for editing. Present on a playlist page only when the signed-in
/// user owns the playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistEditInfo {
    /// Current privacy.
    pub privacy: Privacy,
    /// Current description, if any.
    pub description: Option<String>,
    /// One entry id per track, in the same order as the page's `tracks`.
    pub set_video_ids: Vec<SetVideoId>,
}

/// Edits playlists the signed-in user owns. Every change is sent once: no retry loop, no queue.
/// The caller updates its view only after the future returns `Ok`.
pub trait PlaylistEditor: Send + Sync {
    /// Create a playlist holding the first `videos` in one call. Returns its id.
    ///
    /// # Errors
    /// Returns a [`CatalogError`] when YouTube Music refuses or cannot be reached.
    fn create<'a>(
        &'a self,
        title: &'a str,
        privacy: Privacy,
        videos: &'a [VideoId],
    ) -> BoxFuture<'a, Result<BrowseId, CatalogError>>;

    /// Rename a playlist.
    ///
    /// # Errors
    /// Returns a [`CatalogError`] when YouTube Music refuses or cannot be reached.
    fn rename<'a>(
        &'a self,
        id: &'a BrowseId,
        title: &'a str,
    ) -> BoxFuture<'a, Result<(), CatalogError>>;

    /// Replace the description of a playlist.
    ///
    /// # Errors
    /// Returns a [`CatalogError`] when YouTube Music refuses or cannot be reached.
    fn set_description<'a>(
        &'a self,
        id: &'a BrowseId,
        text: &'a str,
    ) -> BoxFuture<'a, Result<(), CatalogError>>;

    /// Change who can see a playlist.
    ///
    /// # Errors
    /// Returns a [`CatalogError`] when YouTube Music refuses or cannot be reached.
    fn set_privacy<'a>(
        &'a self,
        id: &'a BrowseId,
        privacy: Privacy,
    ) -> BoxFuture<'a, Result<(), CatalogError>>;

    /// Delete a playlist.
    ///
    /// # Errors
    /// Returns a [`CatalogError`] when YouTube Music refuses or cannot be reached.
    fn delete<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<(), CatalogError>>;

    /// Add songs to the end of a playlist.
    ///
    /// # Errors
    /// Returns a [`CatalogError`] when YouTube Music refuses or cannot be reached.
    fn add_tracks<'a>(
        &'a self,
        id: &'a BrowseId,
        videos: &'a [VideoId],
    ) -> BoxFuture<'a, Result<(), CatalogError>>;

    /// Remove one entry from a playlist.
    ///
    /// # Errors
    /// Returns a [`CatalogError`] when YouTube Music refuses or cannot be reached.
    fn remove_track<'a>(
        &'a self,
        id: &'a BrowseId,
        video: &'a VideoId,
        entry: &'a SetVideoId,
    ) -> BoxFuture<'a, Result<(), CatalogError>>;

    /// Move one entry before `before`, or to the end when `before` is `None`.
    ///
    /// # Errors
    /// Returns a [`CatalogError`] when YouTube Music refuses or cannot be reached.
    fn move_track<'a>(
        &'a self,
        id: &'a BrowseId,
        entry: &'a SetVideoId,
        before: Option<&'a SetVideoId>,
    ) -> BoxFuture<'a, Result<(), CatalogError>>;
}
