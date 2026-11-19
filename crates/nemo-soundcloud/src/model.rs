//! The parts of SoundCloud's JSON answers the mapping reads.
//!
//! Only the fields below are parsed. Everything else in an answer (names, birth dates, cities,
//! counters of private accounts) is ignored by `serde`, so it is never held. A list is read item
//! by item: one item that does not fit its type is skipped, not the whole page.

use serde::Deserialize;
use serde_json::Value;

use crate::client::ClientError;

/// The uploader named inside a track or playlist.
#[derive(Debug, Clone, Deserialize)]
pub struct ScUserRef {
    pub username: Option<String>,
    pub avatar_url: Option<String>,
}

/// The publisher data of a track.
#[derive(Debug, Clone, Deserialize)]
pub struct ScPublisher {
    pub artist: Option<String>,
    pub album_title: Option<String>,
}

/// A track. A stub (`{id, kind, policy}` inside a playlist) has no `title`.
#[derive(Debug, Clone, Deserialize)]
pub struct ScTrack {
    pub id: u64,
    pub title: Option<String>,
    /// Length of what plays, in milliseconds (a preview is short).
    pub duration: Option<u64>,
    /// Length of the whole track, in milliseconds.
    pub full_duration: Option<u64>,
    pub artwork_url: Option<String>,
    pub permalink_url: Option<String>,
    pub user: Option<ScUserRef>,
    pub publisher_metadata: Option<ScPublisher>,
}

/// A playlist or an album (`is_album`).
#[derive(Debug, Clone, Deserialize)]
pub struct ScPlaylist {
    pub id: u64,
    pub title: Option<String>,
    pub artwork_url: Option<String>,
    pub user: Option<ScUserRef>,
    #[serde(default)]
    pub is_album: bool,
    pub release_date: Option<String>,
    pub display_date: Option<String>,
    pub track_count: Option<u32>,
    /// Full tracks and stubs, as raw values (read one by one).
    #[serde(default)]
    pub tracks: Vec<Value>,
}

/// A user.
#[derive(Debug, Clone, Deserialize)]
pub struct ScUser {
    pub id: u64,
    pub username: Option<String>,
    pub avatar_url: Option<String>,
    pub description: Option<String>,
}

/// One item of a list.
#[derive(Debug, Clone)]
pub enum Item {
    Track(ScTrack),
    Playlist(ScPlaylist),
    User(ScUser),
}

/// Read one item by its `kind`. `None` for another kind or an item that does not fit.
pub fn parse_item(value: Value) -> Option<Item> {
    let kind = value.get("kind")?.as_str()?.to_owned();
    match kind.as_str() {
        "track" => serde_json::from_value(value).ok().map(Item::Track),
        "playlist" => serde_json::from_value(value).ok().map(Item::Playlist),
        "user" => serde_json::from_value(value).ok().map(Item::User),
        _ => None,
    }
}

/// The items of a list answer: `{"collection": [...]}` or a plain array (`/tracks?ids=`).
///
/// # Errors
/// [`ClientError::Parse`] when the body is neither.
pub fn parse_list(body: &[u8]) -> Result<Vec<Value>, ClientError> {
    match serde_json::from_slice::<Value>(body) {
        Ok(Value::Array(items)) => Ok(items),
        Ok(Value::Object(mut map)) => match map.remove("collection") {
            Some(Value::Array(items)) => Ok(items),
            _ => Err(ClientError::Parse),
        },
        _ => Err(ClientError::Parse),
    }
}

/// Read one object of type `T`.
///
/// # Errors
/// [`ClientError::Parse`] when the body does not fit. The `serde_json` text is dropped: it can
/// quote a value.
pub fn parse_one<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, ClientError> {
    serde_json::from_slice(body).map_err(|_| ClientError::Parse)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn items_are_read_by_kind_and_odd_ones_are_skipped() {
        let track = json!({"kind": "track", "id": 1, "title": "T", "unknown": [1, 2]});
        assert!(matches!(parse_item(track), Some(Item::Track(t)) if t.id == 1));
        assert!(matches!(
            parse_item(json!({"kind": "user", "id": 2})),
            Some(Item::User(_))
        ));
        assert!(parse_item(json!({"kind": "system-playlist", "id": 3})).is_none());
        assert!(parse_item(json!({"kind": "track", "title": "no id"})).is_none());
        assert!(parse_item(json!({"id": 1})).is_none());
        assert!(parse_item(json!("text")).is_none());
    }

    #[test]
    fn lists_come_as_a_collection_or_an_array() {
        assert_eq!(
            parse_list(br#"{"collection":[1,2],"next_href":null}"#).map(|v| v.len()),
            Ok(2)
        );
        assert_eq!(parse_list(b"[1,2,3]").map(|v| v.len()), Ok(3));
        assert_eq!(parse_list(b"{}"), Err(ClientError::Parse));
        assert_eq!(parse_list(b"not json"), Err(ClientError::Parse));
    }

    #[test]
    fn a_parse_error_never_quotes_the_body() {
        let err =
            parse_one::<ScUser>(br#"{"id":"secret-value"}"#).expect_err("id must be a number");
        assert_eq!(err.to_string(), "unexpected answer from SoundCloud");
    }

    #[test]
    fn fields_that_identify_a_person_are_not_parsed() {
        let user: ScUser = parse_one(
            br#"{"id":1,"username":"u","first_name":"A","date_of_birth":"x","city":"c"}"#,
        )
        .expect("user");
        let shown = format!("{user:?}");
        assert!(!shown.contains("first_name") && !shown.contains("city"));
    }
}
