//! What a signed-in SoundCloud account may change (soundcloud-account spec, task 7): like and
//! unlike, unfollow, create a playlist, add or remove a track, rename.
//!
//! Every method makes one request on the account's behalf and answers only after the server
//! did, so the screen can confirm a change only when it happened. Nothing is retried. Follow is
//! not here: SoundCloud's web app signs that one request with a secret of its own script, and the
//! owner has not decided whether to copy that. Implementations never log a token, a title or an
//! id; [`WriteError`] texts are fixed.

use crate::BoxFuture;
use crate::catalog::BrowseId;
use crate::track::TrackId;

/// A change that did not happen. The texts are fixed and user-safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WriteError {
    /// Nobody is signed in to SoundCloud.
    #[error("sign in to SoundCloud first")]
    NotSignedIn,
    /// SoundCloud ended the sign-in.
    #[error("SoundCloud ended this sign-in")]
    SessionEnded,
    /// SoundCloud refused the change.
    #[error("SoundCloud refused this change")]
    Refused,
    /// The item is gone or private.
    #[error("the item was not found on SoundCloud")]
    NotFound,
    /// SoundCloud asked to slow down.
    #[error("SoundCloud asked to slow down")]
    RateLimited,
    /// The network failed, the request timed out or SoundCloud answered with an error. The
    /// change may or may not have happened; it is not repeated.
    #[error("could not reach SoundCloud")]
    Network,
    /// SoundCloud is switched off.
    #[error("SoundCloud is switched off")]
    Off,
    /// The id or the text is not something SoundCloud takes (not a SoundCloud item, an empty
    /// or too long title).
    #[error("this is not something SoundCloud can take")]
    Invalid,
    /// The playlist already has (or does not have) the track: nothing was sent.
    #[error("nothing to change")]
    Unchanged,
}

/// A playlist that was just created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPlaylist {
    /// Its page id.
    pub id: BrowseId,
}

/// The writes of one SoundCloud account.
pub trait SourceWrites: Send + Sync {
    /// Like (`liked`) or unlike a track.
    fn set_track_liked<'a>(
        &'a self,
        track: &'a TrackId,
        liked: bool,
    ) -> BoxFuture<'a, Result<(), WriteError>>;

    /// Like or unlike a playlist or an album.
    fn set_playlist_liked<'a>(
        &'a self,
        playlist: &'a BrowseId,
        liked: bool,
    ) -> BoxFuture<'a, Result<(), WriteError>>;

    /// Stop following a user.
    fn unfollow<'a>(&'a self, user: &'a BrowseId) -> BoxFuture<'a, Result<(), WriteError>>;

    /// Create a private playlist with `title`, holding `first` when given.
    fn create_playlist<'a>(
        &'a self,
        title: &'a str,
        first: Option<&'a TrackId>,
    ) -> BoxFuture<'a, Result<NewPlaylist, WriteError>>;

    /// Add `track` at the end of the playlist. [`WriteError::Unchanged`] when it is in it.
    fn add_to_playlist<'a>(
        &'a self,
        playlist: &'a BrowseId,
        track: &'a TrackId,
    ) -> BoxFuture<'a, Result<(), WriteError>>;

    /// Remove `track` from the playlist. [`WriteError::Unchanged`] when it is not in it.
    fn remove_from_playlist<'a>(
        &'a self,
        playlist: &'a BrowseId,
        track: &'a TrackId,
    ) -> BoxFuture<'a, Result<(), WriteError>>;

    /// Give the playlist a new title.
    fn rename_playlist<'a>(
        &'a self,
        playlist: &'a BrowseId,
        title: &'a str,
    ) -> BoxFuture<'a, Result<(), WriteError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_texts_are_fixed_and_hold_no_detail() {
        for e in [
            WriteError::NotSignedIn,
            WriteError::SessionEnded,
            WriteError::Refused,
            WriteError::NotFound,
            WriteError::RateLimited,
            WriteError::Network,
            WriteError::Off,
            WriteError::Invalid,
            WriteError::Unchanged,
        ] {
            let text = e.to_string();
            assert!(!text.contains("OAuth") && !text.contains("http"), "{text}");
            assert!(!text.chars().any(|c| c.is_ascii_digit()), "{text}");
        }
    }
}
