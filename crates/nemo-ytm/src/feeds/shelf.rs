//! Shelves and the home, explore, mood and chart pages.

use nemo_core::catalog::{
    AlbumSummary, Explore, FeedItem, FeedShelf, HomeFeed, MoodChip, ShelfKind,
};
use serde_json::Value;

use super::item::{feed_item, mood_id};
use super::json::{find_key, text_of};
use super::{MAX_ITEMS, MAX_SHELVES};

fn shelf_of(section: &Value) -> Option<FeedShelf> {
    let (title, subtitle, entries) = if let Some(s) = section
        .get("musicCarouselShelfRenderer")
        .or_else(|| section.get("musicImmersiveCarouselShelfRenderer"))
    {
        let header = s.get("header").and_then(|h| {
            h.get("musicCarouselShelfBasicHeaderRenderer")
                .or_else(|| h.get("musicImmersiveCarouselShelfBasicHeaderRenderer"))
        })?;
        (
            header.get("title").and_then(text_of)?,
            header.get("strapline").and_then(text_of),
            s.get("contents")?.as_array()?,
        )
    } else if let Some(s) = section.get("musicShelfRenderer") {
        (
            s.get("title").and_then(text_of)?,
            None,
            s.get("contents")?.as_array()?,
        )
    } else {
        let s = section.get("gridRenderer")?;
        (
            s.pointer("/header/gridHeaderRenderer/title")
                .and_then(text_of)?,
            None,
            s.get("items")?.as_array()?,
        )
    };
    let items: Vec<FeedItem> = entries
        .iter()
        .filter_map(feed_item)
        .take(MAX_ITEMS)
        .collect();
    if items.is_empty() {
        return None;
    }
    let kind = if items.iter().all(|i| matches!(i, FeedItem::Mood(_))) {
        ShelfKind::Chips
    } else if entries
        .iter()
        .any(|e| e.get("musicResponsiveListItemRenderer").is_some())
        && items
            .iter()
            .all(|i| matches!(i, FeedItem::Song(_) | FeedItem::Video(_)))
    {
        ShelfKind::QuickPicks
    } else {
        ShelfKind::Cards
    };
    Some(FeedShelf {
        title,
        subtitle,
        kind,
        items,
    })
}

/// Shelves of a list of page sections. Sections of unknown kinds are skipped.
pub(crate) fn parse_sections(sections: &[Value]) -> Vec<FeedShelf> {
    sections
        .iter()
        .filter_map(shelf_of)
        .take(MAX_SHELVES)
        .collect()
}

/// The `contents` of the page's section list (first page or continuation).
pub(super) fn section_contents(root: &Value) -> &[Value] {
    find_key(root, "sectionListContinuation")
        .or_else(|| find_key(root, "sectionListRenderer"))
        .and_then(|s| s.get("contents"))
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Where the next-part token of a home page lives.
pub(super) const HOME_SCOPES: [&str; 2] = ["sectionListContinuation", "sectionListRenderer"];
/// Where the next-part token of a list page lives, most specific first.
pub(super) const LIST_SCOPES: [&str; 7] = [
    "musicPlaylistShelfRenderer",
    "musicPlaylistShelfContinuation",
    "musicShelfContinuation",
    "gridContinuation",
    "musicShelfRenderer",
    "gridRenderer",
    "appendContinuationItemsAction",
];

/// The token of the next part of a long list, looked for only inside the first container named
/// in `scopes`, so a token of some other part of the page is never taken.
pub(crate) fn continuation_token(root: &Value, scopes: &[&str]) -> Option<String> {
    let scope = scopes.iter().find_map(|key| find_key(root, key))?;
    find_key(scope, "nextContinuationData")
        .and_then(|d| d.get("continuation"))
        .or_else(|| find_key(scope, "continuationCommand").and_then(|c| c.get("token")))
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty() && t.len() <= 4096)
        .map(str::to_owned)
}
pub(super) fn parse_root(json: &str) -> Option<Value> {
    serde_json::from_str(json).ok()
}

// ---------------------------------------------------------------------------------------------
// Home, explore, moods
// ---------------------------------------------------------------------------------------------

/// The mood chips above the home feed.
fn parse_chips(root: &Value) -> Vec<MoodChip> {
    find_key(root, "chipCloudRenderer")
        .and_then(|c| c.get("chips"))
        .and_then(Value::as_array)
        .map(|chips| {
            chips
                .iter()
                .filter_map(|chip| {
                    let chip = chip.get("chipCloudChipRenderer")?;
                    let title = chip.get("text").and_then(text_of)?;
                    let browse = chip.pointer("/navigationEndpoint/browseEndpoint")?;
                    let params = browse.get("params")?.as_str()?;
                    let id = mood_id(browse.get("browseId")?.as_str()?, Some(params))?;
                    Some(MoodChip { title, id })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// One page of the home feed (first page or continuation) and the token of the next one.
pub(crate) fn parse_home_page(json: &str) -> (HomeFeed, Option<String>) {
    let Some(root) = parse_root(json) else {
        return (HomeFeed::default(), None);
    };
    let feed = HomeFeed {
        chips: parse_chips(&root),
        shelves: parse_sections(section_contents(&root)),
    };
    (feed, continuation_token(&root, &HOME_SCOPES))
}

/// Sections of a mood or genre page.
pub(crate) fn parse_mood_page(json: &str) -> Vec<FeedShelf> {
    parse_root(json)
        .map(|root| parse_sections(section_contents(&root)))
        .unwrap_or_default()
}

/// The Explore page: the first shelf made only of albums is "new releases", tiles that open a
/// mood page are the moods, everything else is kept as shelves.
pub(crate) fn parse_explore(json: &str) -> Explore {
    let Some(root) = parse_root(json) else {
        return Explore::default();
    };
    let mut explore = Explore::default();
    let mut have_releases = false;
    for shelf in parse_sections(section_contents(&root)) {
        let albums: Vec<AlbumSummary> = shelf
            .items
            .iter()
            .filter_map(|i| match i {
                FeedItem::Album(a) => Some(a.clone()),
                _ => None,
            })
            .collect();
        if shelf.kind == ShelfKind::Chips {
            for item in shelf.items {
                if let FeedItem::Mood(mood) = item
                    && mood.id.0.starts_with("FEmusic_moods_and_genres_category")
                {
                    explore.moods.push(mood);
                }
            }
        } else if !have_releases && !albums.is_empty() && albums.len() == shelf.items.len() {
            have_releases = true;
            explore.new_releases = albums;
        } else {
            explore.shelves.push(shelf);
        }
    }
    explore
}

/// The chart shelves of the charts page.
pub(crate) fn parse_charts(json: &str) -> Vec<FeedShelf> {
    parse_mood_page(json)
}
