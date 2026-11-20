//! [`SoundcloudWrites`]: the account's writes ([`SourceWrites`]) over [`ScClient::write`], the one
//! function every write goes through. Each method sends one request and answers after the server
//! did. Nothing is retried, and nothing here logs a token, a title or an id.

use std::sync::Arc;

use nemo_core::BoxFuture;
use nemo_core::catalog::BrowseId;
use nemo_core::source_writes::{NewPlaylist, SourceWrites, WriteError};
use nemo_core::track::TrackId;
use serde_json::Value;

use crate::client::{ClientError, ScClient};
use crate::map::{Kind, browse_id, parse_browse_id, parse_track_id};
use crate::write::{WriteCall, clean_title, created_id, track_ids};

/// The writes of the account the client is signed in as.
pub struct SoundcloudWrites {
    client: Arc<ScClient>,
}

impl std::fmt::Debug for SoundcloudWrites {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundcloudWrites").finish_non_exhaustive()
    }
}

/// A [`ClientError`] of a write as the error the screen words.
const fn failure(e: ClientError) -> WriteError {
    match e {
        ClientError::NotSignedIn => WriteError::NotSignedIn,
        ClientError::SessionEnded => WriteError::SessionEnded,
        ClientError::RateLimited => WriteError::RateLimited,
        ClientError::NotFound => WriteError::NotFound,
        ClientError::Off => WriteError::Off,
        ClientError::Refused => WriteError::Refused,
        ClientError::Network
        | ClientError::Timeout
        | ClientError::Parse
        | ClientError::NoClientId => WriteError::Network,
    }
}

/// The numeric id of a playlist page id.
fn playlist_id(id: &BrowseId) -> Result<u64, WriteError> {
    match parse_browse_id(id) {
        Some((Kind::Playlist, n)) => Ok(n),
        _ => Err(WriteError::Invalid),
    }
}

fn track_number(id: &TrackId) -> Result<u64, WriteError> {
    parse_track_id(id).ok_or(WriteError::Invalid)
}

impl SoundcloudWrites {
    /// Writes through `client`, which holds the session.
    #[must_use]
    pub const fn new(client: Arc<ScClient>) -> Self {
        Self { client }
    }

    fn me(&self) -> Result<u64, WriteError> {
        self.client.auth().user_id().ok_or(WriteError::NotSignedIn)
    }

    async fn send(&self, call: WriteCall) -> Result<Vec<u8>, WriteError> {
        self.client.write(&call).await.map_err(failure)
    }

    /// The playlist's track ids as they are on SoundCloud now (also tracks that are not playable
    /// here), so a change never drops one.
    async fn current_tracks(&self, playlist: u64) -> Result<Vec<u64>, WriteError> {
        let body = self
            .client
            .get(&format!("/playlists/{playlist}"), &[])
            .await
            .map_err(failure)?;
        let value: Value = serde_json::from_slice(&body).map_err(|_| WriteError::Network)?;
        track_ids(&value).ok_or(WriteError::Network)
    }

    async fn change_tracks(
        &self,
        playlist: &BrowseId,
        track: &TrackId,
        add: bool,
    ) -> Result<(), WriteError> {
        let (playlist, track) = (playlist_id(playlist)?, track_number(track)?);
        self.me()?;
        let mut tracks = self.current_tracks(playlist).await?;
        let present = tracks.contains(&track);
        if present == add {
            return Err(WriteError::Unchanged);
        }
        if add {
            tracks.push(track);
        } else {
            tracks.retain(|t| *t != track);
        }
        self.send(WriteCall::set_tracks(playlist, &tracks)).await?;
        Ok(())
    }
}

impl SourceWrites for SoundcloudWrites {
    fn set_track_liked<'a>(
        &'a self,
        track: &'a TrackId,
        liked: bool,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        Box::pin(async move {
            let track = track_number(track)?;
            let call = WriteCall::track_like(self.me()?, track, liked);
            self.send(call).await.map(|_| ())
        })
    }

    fn set_playlist_liked<'a>(
        &'a self,
        playlist: &'a BrowseId,
        liked: bool,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        Box::pin(async move {
            let playlist = playlist_id(playlist)?;
            let call = WriteCall::playlist_like(self.me()?, playlist, liked);
            self.send(call).await.map(|_| ())
        })
    }

    fn unfollow<'a>(&'a self, user: &'a BrowseId) -> BoxFuture<'a, Result<(), WriteError>> {
        Box::pin(async move {
            let Some((Kind::User, id)) = parse_browse_id(user) else {
                return Err(WriteError::Invalid);
            };
            self.me()?;
            self.send(WriteCall::unfollow(id)).await.map(|_| ())
        })
    }

    fn create_playlist<'a>(
        &'a self,
        title: &'a str,
        first: Option<&'a TrackId>,
    ) -> BoxFuture<'a, Result<NewPlaylist, WriteError>> {
        Box::pin(async move {
            let title = clean_title(title).ok_or(WriteError::Invalid)?;
            let tracks: Vec<u64> = match first {
                Some(t) => vec![track_number(t)?],
                None => Vec::new(),
            };
            self.me()?;
            let body = self
                .send(WriteCall::create_playlist(&title, &tracks))
                .await?;
            // The server answered 2xx: the playlist exists. Its id is what the screen opens.
            let value: Value = serde_json::from_slice(&body).map_err(|_| WriteError::Network)?;
            let id = created_id(&value).ok_or(WriteError::Network)?;
            Ok(NewPlaylist {
                id: browse_id(Kind::Playlist, id),
            })
        })
    }

    fn add_to_playlist<'a>(
        &'a self,
        playlist: &'a BrowseId,
        track: &'a TrackId,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        Box::pin(self.change_tracks(playlist, track, true))
    }

    fn remove_from_playlist<'a>(
        &'a self,
        playlist: &'a BrowseId,
        track: &'a TrackId,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        Box::pin(self.change_tracks(playlist, track, false))
    }

    fn rename_playlist<'a>(
        &'a self,
        playlist: &'a BrowseId,
        title: &'a str,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        Box::pin(async move {
            let playlist = playlist_id(playlist)?;
            let title = clean_title(title).ok_or(WriteError::Invalid)?;
            self.me()?;
            self.send(WriteCall::rename(playlist, &title))
                .await
                .map(|_| ())
        })
    }
}

#[cfg(test)]
mod tests;
