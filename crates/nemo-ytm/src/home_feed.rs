//! Home feed and account header of a signed-in session.
//!
//! `ytmapi-rs` 0.3.4 has no home-feed or account-menu query, so this module adds the two
//! requests through its public `Query` trait and reads the answers with small tolerant parsers.
//! A shelf item that cannot be understood is dropped; the rest of the feed still shows.

use std::borrow::Cow;

use nemo_core::account::AccountInfo;
use nemo_core::catalog::{Shelf, ShelfItem};
use nemo_core::track::Thumbnail;
use serde_json::{Map, Value, json};
use ytmapi_rs::auth::AuthToken;
use ytmapi_rs::parse::{ParseFrom, ProcessedResult};
use ytmapi_rs::query::{PostMethod, PostQuery, Query};

use crate::catalog::{
    RawAlbum, RawArtist, RawPlaylist, RawSong, map_album, map_artist, map_playlist, map_song,
    map_thumbnails,
};

/// Most shelves kept from the home feed.
const MAX_SHELVES: usize = 12;
/// Most items kept per shelf.
const MAX_ITEMS: usize = 30;

/// The answer is read from the raw JSON, so nothing is parsed by `ytmapi-rs`.
#[derive(Debug)]
pub struct Unparsed;

impl ParseFrom<HomeQuery> for Unparsed {
    fn parse_from(_: ProcessedResult<HomeQuery>) -> ytmapi_rs::Result<Self> {
        Ok(Self)
    }
}

impl ParseFrom<AccountMenuQuery> for Unparsed {
    fn parse_from(_: ProcessedResult<AccountMenuQuery>) -> ytmapi_rs::Result<Self> {
        Ok(Self)
    }
}

/// `browse` request for the account's home feed.
pub struct HomeQuery;

impl<A: AuthToken> Query<A> for HomeQuery {
    type Output = Unparsed;
    type Method = PostMethod;
}

impl PostQuery for HomeQuery {
    fn header(&self) -> Map<String, Value> {
        Map::from_iter([("browseId".to_owned(), json!("FEmusic_home"))])
    }
    fn params(&self) -> Vec<(&str, Cow<'_, str>)> {
        Vec::new()
    }
    fn path(&self) -> &'static str {
        "browse"
    }
}

/// Request for the account menu, whose header holds the display name and picture.
pub struct AccountMenuQuery;

impl<A: AuthToken> Query<A> for AccountMenuQuery {
    type Output = Unparsed;
    type Method = PostMethod;
}

impl PostQuery for AccountMenuQuery {
    fn header(&self) -> Map<String, Value> {
        Map::new()
    }
    fn params(&self) -> Vec<(&str, Cow<'_, str>)> {
        Vec::new()
    }
    fn path(&self) -> &'static str {
        "account/account_menu"
    }
}

// ---------------------------------------------------------------------------------------------
// JSON helpers
// ---------------------------------------------------------------------------------------------

/// First value stored under `key` anywhere inside `value` (depth-first).
fn find_key<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map
            .get(key)
            .or_else(|| map.values().find_map(|v| find_key(v, key))),
        Value::Array(items) => items.iter().find_map(|v| find_key(v, key)),
        _ => None,
    }
}

fn text_of(value: &Value) -> Option<String> {
    let text = value
        .pointer("/runs/0/text")
        .or_else(|| value.get("simpleText"))?
        .as_str()?
        .trim();
    (!text.is_empty()).then(|| text.to_owned())
}

fn run_texts(value: &Value) -> Vec<String> {
    value
        .get("runs")
        .and_then(Value::as_array)
        .map(|runs| {
            runs.iter()
                .filter_map(|r| r.get("text").and_then(Value::as_str))
                .map(str::trim)
                .filter(|t| !t.is_empty() && *t != "•" && *t != "&" && *t != ",")
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn thumbnails_of(value: &Value) -> Vec<Thumbnail> {
    let list: Vec<(String, u64, u64)> = find_key(value, "thumbnails")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|t| {
                    Some((
                        t.get("url")?.as_str()?.to_owned(),
                        t.get("width").and_then(Value::as_u64).unwrap_or(0),
                        t.get("height").and_then(Value::as_u64).unwrap_or(0),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    map_thumbnails(&list)
}

// ---------------------------------------------------------------------------------------------
// Account header
// ---------------------------------------------------------------------------------------------

/// Read the account name, handle and picture from an account-menu answer.
///
/// Returns `None` when the answer has no signed-in header, which is what YouTube Music sends for
/// a session that is not signed in.
#[must_use]
pub fn parse_account(json: &str) -> Option<AccountInfo> {
    let root: Value = serde_json::from_str(json).ok()?;
    let header = find_key(&root, "activeAccountHeaderRenderer")?;
    let name = header.get("accountName").and_then(text_of)?;
    Some(AccountInfo {
        name,
        handle: header
            .get("channelHandle")
            .and_then(text_of)
            .filter(|h| h.starts_with('@')),
        avatar: header
            .get("accountPhoto")
            .map(thumbnails_of)
            .unwrap_or_default(),
    })
}

// ---------------------------------------------------------------------------------------------
// Home feed
// ---------------------------------------------------------------------------------------------

fn is_type_label(text: &str) -> bool {
    matches!(
        text,
        "Song" | "Video" | "Album" | "Single" | "EP" | "Playlist" | "Artist" | "Podcast"
    )
}

fn is_noise(text: &str) -> bool {
    is_type_label(text) || text.ends_with(" views") || text.ends_with(" plays")
}

fn two_row_item(item: &Value) -> Option<ShelfItem> {
    let title = item.get("title").and_then(text_of)?;
    let thumbnails = thumbnails_of(item);
    let subtitle: Vec<String> = item
        .get("subtitle")
        .map(run_texts)
        .unwrap_or_default()
        .into_iter()
        .filter(|t| !is_noise(t))
        .collect();
    let nav = item.get("navigationEndpoint")?;
    if let Some(video_id) = nav
        .pointer("/watchEndpoint/videoId")
        .and_then(Value::as_str)
    {
        return map_song(&RawSong {
            video_id: video_id.to_owned(),
            title,
            artists: subtitle,
            album: None,
            duration: None,
            thumbnails,
        })
        .map(ShelfItem::Song);
    }
    if let Some(id) = nav
        .pointer("/browseEndpoint/browseId")
        .and_then(Value::as_str)
    {
        if id.starts_with("MPRE") {
            let year = subtitle
                .iter()
                .find(|t| crate::catalog::parse_year(t).is_some())
                .cloned();
            let artists = subtitle
                .into_iter()
                .filter(|t| crate::catalog::parse_year(t).is_none())
                .collect();
            return map_album(&RawAlbum {
                id: id.to_owned(),
                title,
                artists,
                year,
                thumbnails,
            })
            .map(ShelfItem::Album);
        }
        if id.starts_with("UC") {
            return map_artist(&RawArtist {
                id: id.to_owned(),
                name: title,
                thumbnails,
            })
            .map(ShelfItem::Artist);
        }
        if id.starts_with("VL") {
            return map_playlist(&RawPlaylist {
                id: id.to_owned(),
                title,
                author: subtitle.last().cloned(),
                thumbnails,
            })
            .map(ShelfItem::Playlist);
        }
        return None;
    }
    let playlist = nav
        .pointer("/watchPlaylistEndpoint/playlistId")
        .and_then(Value::as_str)?;
    map_playlist(&RawPlaylist {
        id: format!("VL{playlist}"),
        title,
        author: subtitle.last().cloned(),
        thumbnails,
    })
    .map(ShelfItem::Playlist)
}

fn list_item(item: &Value) -> Option<ShelfItem> {
    let columns = item.get("flexColumns")?.as_array()?;
    let column = |index: usize| {
        columns
            .get(index)?
            .get("musicResponsiveListItemFlexColumnRenderer")?
            .get("text")
    };
    let title = column(0).and_then(text_of)?;
    let video_id = find_key(item, "videoId")?.as_str()?.to_owned();
    let artists = column(1)
        .map(run_texts)
        .unwrap_or_default()
        .into_iter()
        .filter(|t| !is_noise(t))
        .take(1)
        .collect();
    map_song(&RawSong {
        video_id,
        title,
        artists,
        album: None,
        duration: None,
        thumbnails: thumbnails_of(item),
    })
    .map(ShelfItem::Song)
}

fn shelf_item(entry: &Value) -> Option<ShelfItem> {
    entry
        .get("musicTwoRowItemRenderer")
        .and_then(two_row_item)
        .or_else(|| {
            entry
                .get("musicResponsiveListItemRenderer")
                .and_then(list_item)
        })
}

/// Read the shelves of a home-feed answer. Unknown shelf kinds and items are skipped.
#[must_use]
pub fn parse_home(json: &str) -> Vec<Shelf> {
    let Ok(root) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(sections) = find_key(&root, "sectionListRenderer")
        .and_then(|s| s.get("contents"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    sections
        .iter()
        .filter_map(|section| section.get("musicCarouselShelfRenderer"))
        .filter_map(|shelf| {
            let title = shelf
                .pointer("/header/musicCarouselShelfBasicHeaderRenderer/title")
                .and_then(text_of)?;
            let items: Vec<ShelfItem> = shelf
                .get("contents")?
                .as_array()?
                .iter()
                .filter_map(shelf_item)
                .take(MAX_ITEMS)
                .collect();
            (!items.is_empty()).then_some(Shelf { title, items })
        })
        .take(MAX_SHELVES)
        .collect()
}

#[cfg(test)]
#[allow(
    clippy::assert_is_empty,
    clippy::needless_pass_by_value,
    reason = "readable test assertions and helpers"
)]
mod tests {
    use super::*;

    const ID: &str = "abcdefghijk";

    fn runs(texts: &[&str]) -> Value {
        json!({ "runs": texts.iter().map(|t| json!({ "text": t })).collect::<Vec<_>>() })
    }

    fn thumbs() -> Value {
        json!({ "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
            { "url": "https://img.invalid/a", "width": 120, "height": 120 }
        ] } } })
    }

    fn feed(shelves: Vec<Value>) -> String {
        json!({ "contents": { "singleColumnBrowseResultsRenderer": { "tabs": [ {
            "tabRenderer": { "content": { "sectionListRenderer": { "contents": shelves } } }
        } ] } } })
        .to_string()
    }

    fn shelf(title: &str, items: Vec<Value>) -> Value {
        json!({ "musicCarouselShelfRenderer": {
            "header": { "musicCarouselShelfBasicHeaderRenderer": { "title": runs(&[title]) } },
            "contents": items
        } })
    }

    fn two_row(title: &str, subtitle: &[&str], endpoint: &Value) -> Value {
        json!({ "musicTwoRowItemRenderer": {
            "title": runs(&[title]),
            "subtitle": runs(subtitle),
            "thumbnailRenderer": thumbs(),
            "navigationEndpoint": endpoint
        } })
    }

    #[test]
    fn account_header_gives_name_handle_and_picture() {
        let answer = json!({ "actions": [ { "openPopupAction": { "popup": {
            "multiPageMenuRenderer": { "header": { "activeAccountHeaderRenderer": {
                "accountName": runs(&["Test Person"]),
                "channelHandle": runs(&["@testperson"]),
                "accountPhoto": { "thumbnails": [
                    { "url": "https://img.invalid/p", "width": 48, "height": 48 }
                ] }
            } } }
        } } } ] })
        .to_string();
        let info = parse_account(&answer).expect("header");
        assert_eq!(info.name, "Test Person");
        assert_eq!(info.handle.as_deref(), Some("@testperson"));
        assert_eq!(info.avatar.len(), 1);
    }

    #[test]
    fn an_answer_without_the_header_is_not_signed_in() {
        assert_eq!(parse_account("{}"), None);
        assert_eq!(parse_account("not json"), None);
    }

    #[test]
    fn home_items_map_to_songs_albums_artists_and_playlists() {
        let song = two_row(
            "A Song",
            &["Song", " • ", "Artist One", " • ", "1.2M views"],
            &json!({ "watchEndpoint": { "videoId": ID } }),
        );
        let album = two_row(
            "An Album",
            &["Album", " • ", "Artist One", " • ", "2019"],
            &json!({ "browseEndpoint": { "browseId": "MPREb_synthetic1" } }),
        );
        let artist = two_row(
            "An Artist",
            &["Artist"],
            &json!({ "browseEndpoint": { "browseId": "UCsynthetic1" } }),
        );
        let playlist = two_row(
            "A Mix",
            &["Playlist", " • ", "Someone"],
            &json!({ "browseEndpoint": { "browseId": "VLPLsynthetic1" } }),
        );
        let mix = two_row(
            "Radio",
            &["Playlist"],
            &json!({ "watchPlaylistEndpoint": { "playlistId": "RDsynthetic1" } }),
        );
        let shelves = parse_home(&feed(vec![shelf(
            "Listen again",
            vec![song, album, artist, playlist, mix],
        )]));
        assert_eq!(shelves.len(), 1);
        assert_eq!(shelves[0].title, "Listen again");
        let items = &shelves[0].items;
        assert_eq!(items.len(), 5);
        match &items[0] {
            ShelfItem::Song(t) => {
                assert_eq!(t.title, "A Song");
                assert_eq!(t.artists, vec!["Artist One".to_owned()]);
            }
            other => panic!("{other:?}"),
        }
        match &items[1] {
            ShelfItem::Album(a) => {
                assert_eq!(a.year, Some(2019));
                assert_eq!(a.artists, vec!["Artist One".to_owned()]);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(&items[2], ShelfItem::Artist(a) if a.name == "An Artist"));
        assert!(
            matches!(&items[3], ShelfItem::Playlist(p) if p.author.as_deref() == Some("Someone"))
        );
        assert!(matches!(&items[4], ShelfItem::Playlist(p) if p.id.0 == "VLRDsynthetic1"));
    }

    #[test]
    fn quick_picks_rows_become_songs() {
        let row = json!({ "musicResponsiveListItemRenderer": {
            "thumbnail": thumbs(),
            "flexColumns": [
                { "musicResponsiveListItemFlexColumnRenderer": { "text": runs(&["Quick Song"]) } },
                { "musicResponsiveListItemFlexColumnRenderer": { "text": runs(&["Artist Two"]) } }
            ],
            "overlay": { "x": { "playNavigationEndpoint": { "watchEndpoint": { "videoId": ID } } } }
        } });
        let shelves = parse_home(&feed(vec![shelf("Quick picks", vec![row])]));
        match &shelves[0].items[0] {
            ShelfItem::Song(t) => assert_eq!(t.artists, vec!["Artist Two".to_owned()]),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn unusable_items_and_empty_shelves_are_dropped() {
        let bad_id = two_row(
            "Bad",
            &[],
            &json!({ "watchEndpoint": { "videoId": "short" } }),
        );
        let no_nav = json!({ "musicTwoRowItemRenderer": { "title": runs(&["No nav"]) } });
        let other = two_row(
            "Other",
            &[],
            &json!({ "browseEndpoint": { "browseId": "FEother" } }),
        );
        let shelves = parse_home(&feed(vec![
            shelf("Empty", vec![bad_id, no_nav, other]),
            json!({ "musicDescriptionShelfRenderer": {} }),
        ]));
        assert!(shelves.is_empty());
        assert!(parse_home("{}").is_empty());
        assert!(parse_home("nope").is_empty());
    }
}
