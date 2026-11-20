//! The requests of the signed-in account's writes, as the web app sends them (read from its own
//! script, `a-v2.sndcdn.com`, on 2026-11-20): the endpoint table and the model `save` that wraps a
//! playlist's attributes as `{"playlist": {...}}` with the tracks as a list of ids.
//!
//! - like and unlike a track: `PUT` and `DELETE users/<me>/track_likes/<track>`
//! - like and unlike a playlist or album: `PUT` and `DELETE users/<me>/playlist_likes/<playlist>`
//! - unfollow: `DELETE me/followings/<user>`
//! - create a playlist: `POST playlists` with `{"playlist": {title, sharing, tracks}}`
//! - add or remove a track, rename: `PUT playlists/<id>` with the changed attributes only (the
//!   whole list of track ids for a track change, as the web app saves it)
//!
//! Follow (`POST me/followings/<user>`) is not built: the web app adds a `signature` made with a
//! secret of its own script. Every id here is a number, so no id can change the path. Titles go
//! into the JSON body through `serde_json`. Nothing here sends anything; [`ScClient::write`] does.
//!
//! [`ScClient::write`]: crate::ScClient::write

use serde_json::{Value, json};

use crate::http::ScMethod;

/// Longest playlist title, in characters.
pub const MAX_TITLE_CHARS: usize = 100;

/// One write: a method, an API path and the JSON body, if it has one.
#[derive(Debug, Clone, PartialEq)]
pub struct WriteCall {
    /// `PUT`, `POST` or `DELETE`.
    pub method: ScMethod,
    /// The path on the API host, starting with `/`. Digits and fixed words only.
    pub path: String,
    /// The JSON body.
    pub body: Option<Value>,
}

/// A title as SoundCloud takes it: trimmed, no control characters, 1 to [`MAX_TITLE_CHARS`]
/// characters. `None` for anything else.
#[must_use]
pub fn clean_title(title: &str) -> Option<String> {
    let text: String = title.chars().filter(|c| !c.is_control()).collect();
    let text = text.trim();
    let n = text.chars().count();
    (1..=MAX_TITLE_CHARS).contains(&n).then(|| text.to_owned())
}

impl WriteCall {
    fn new(method: ScMethod, path: String, body: Option<Value>) -> Self {
        Self { method, path, body }
    }

    /// Like (`liked`) or unlike a track.
    #[must_use]
    pub fn track_like(user: u64, track: u64, liked: bool) -> Self {
        let path = format!("/users/{user}/track_likes/{track}");
        Self::new(like_method(liked), path, None)
    }

    /// Like or unlike a playlist or an album.
    #[must_use]
    pub fn playlist_like(user: u64, playlist: u64, liked: bool) -> Self {
        let path = format!("/users/{user}/playlist_likes/{playlist}");
        Self::new(like_method(liked), path, None)
    }

    /// Stop following `user`.
    #[must_use]
    pub fn unfollow(user: u64) -> Self {
        Self::new(ScMethod::Delete, format!("/me/followings/{user}"), None)
    }

    /// A private playlist called `title` (already cleaned) with `tracks`.
    #[must_use]
    pub fn create_playlist(title: &str, tracks: &[u64]) -> Self {
        let body = json!({ "playlist": {
            "title": title,
            "sharing": "private",
            "tracks": tracks,
            "_resource_type": "playlist",
        }});
        Self::new(ScMethod::Post, "/playlists".to_owned(), Some(body))
    }

    /// The playlist's whole list of track ids, in order.
    #[must_use]
    pub fn set_tracks(playlist: u64, tracks: &[u64]) -> Self {
        let body = json!({ "playlist": { "tracks": tracks } });
        Self::new(ScMethod::Put, format!("/playlists/{playlist}"), Some(body))
    }

    /// A new `title` (already cleaned).
    #[must_use]
    pub fn rename(playlist: u64, title: &str) -> Self {
        let body = json!({ "playlist": { "title": title } });
        Self::new(ScMethod::Put, format!("/playlists/{playlist}"), Some(body))
    }
}

const fn like_method(liked: bool) -> ScMethod {
    if liked {
        ScMethod::Put
    } else {
        ScMethod::Delete
    }
}

/// The track ids of a playlist answer (`tracks[].id`), in order, including tracks the answer
/// holds only as an id. `None` when the answer has no such list.
#[must_use]
pub fn track_ids(playlist: &Value) -> Option<Vec<u64>> {
    playlist
        .get("tracks")?
        .as_array()?
        .iter()
        .map(|t| t.get("id").and_then(Value::as_u64))
        .collect()
}

/// The id of a playlist the server just created.
#[must_use]
pub fn created_id(answer: &Value) -> Option<u64> {
    answer.get("id").and_then(Value::as_u64)
}

#[cfg(test)]
mod tests;
