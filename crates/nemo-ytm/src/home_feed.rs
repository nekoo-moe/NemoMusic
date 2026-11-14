//! Account header and the flat home feed of the older `Catalog::home` contract.
//!
//! `ytmapi-rs` 0.3.4 has no account-menu query, so this module adds it through its public
//! `Query` trait. The richer home page lives in `crate::feeds`; [`legacy_shelves`] flattens it
//! for callers that only know [`Shelf`].

use std::borrow::Cow;

use nemo_core::account::AccountInfo;
use nemo_core::catalog::{FeedItem, HomeFeed, Shelf, ShelfItem};
use serde_json::{Map, Value};
use ytmapi_rs::auth::AuthToken;
use ytmapi_rs::parse::{ParseFrom, ProcessedResult};
use ytmapi_rs::query::{PostMethod, PostQuery, Query};

use crate::feeds::{find_key, parse_home_page, text_of, thumbnails_of};
use crate::raw::Unparsed;

impl ParseFrom<AccountMenuQuery> for Unparsed {
    fn parse_from(_: ProcessedResult<AccountMenuQuery>) -> ytmapi_rs::Result<Self> {
        Ok(Self)
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

/// Flatten a home page into the old shelf type: songs and videos become songs; podcasts,
/// episodes and mood tiles are left out.
#[must_use]
pub fn legacy_shelves(feed: &HomeFeed) -> Vec<Shelf> {
    feed.shelves
        .iter()
        .filter_map(|shelf| {
            let items: Vec<ShelfItem> = shelf
                .items
                .iter()
                .filter_map(|item| match item {
                    FeedItem::Song(t) | FeedItem::Video(t) => Some(ShelfItem::Song(t.clone())),
                    FeedItem::Album(a) => Some(ShelfItem::Album(a.clone())),
                    FeedItem::Artist(a) => Some(ShelfItem::Artist(a.clone())),
                    FeedItem::Playlist(p) => Some(ShelfItem::Playlist(p.clone())),
                    FeedItem::Podcast(_) | FeedItem::Episode(_) | FeedItem::Mood(_) => None,
                })
                .collect();
            (!items.is_empty()).then(|| Shelf {
                title: shelf.title.clone(),
                items,
            })
        })
        .collect()
}

/// The shelves of a first home page, in the old shape.
#[must_use]
pub fn parse_home(json: &str) -> Vec<Shelf> {
    legacy_shelves(&parse_home_page(json).0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn runs(texts: &[&str]) -> Value {
        json!({ "runs": texts.iter().map(|t| json!({ "text": t })).collect::<Vec<_>>() })
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
    fn the_old_shelf_shape_keeps_music_and_drops_podcasts() {
        let card = |title: &str, endpoint: &Value| {
            json!({ "musicTwoRowItemRenderer": {
                "title": runs(&[title]),
                "subtitle": runs(&["x"]),
                "navigationEndpoint": endpoint
            } })
        };
        let answer = json!({ "contents": { "singleColumnBrowseResultsRenderer": { "tabs": [ {
            "tabRenderer": { "content": { "sectionListRenderer": { "contents": [
                { "musicCarouselShelfRenderer": {
                    "header": { "musicCarouselShelfBasicHeaderRenderer": { "title": runs(&["Mixed"]) } },
                    "contents": [
                        card("A Mix", &json!({ "browseEndpoint": { "browseId": "VLPLsynthetic1" } })),
                        card("A Show", &json!({ "browseEndpoint": { "browseId": "MPSPsynthetic1" } })),
                    ]
                } },
                { "musicCarouselShelfRenderer": {
                    "header": { "musicCarouselShelfBasicHeaderRenderer": { "title": runs(&["Shows"]) } },
                    "contents": [
                        card("Only Show", &json!({ "browseEndpoint": { "browseId": "MPSPsynthetic2" } })),
                    ]
                } }
            ] } } }
        } ] } } })
        .to_string();
        let shelves = parse_home(&answer);
        assert_eq!(shelves.len(), 1, "a shelf of only podcasts is dropped");
        assert_eq!(shelves[0].title, "Mixed");
        assert_eq!(shelves[0].items.len(), 1);
        assert_eq!(parse_home("nope").len(), 0);
    }
}
