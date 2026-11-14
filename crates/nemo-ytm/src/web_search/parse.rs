//! Parsers for the web search answer (`POST /youtubei/v1/search`, `WEB` client).
//!
//! Shapes follow the spike fixture `tests/fixtures/web/search.json`: `itemSectionRenderer`
//! sections hold `videoRenderer`, `officialCardViewModel` (a channel card) and
//! `lockupViewModel` items. Shelves, shorts and anything else are skipped. Items that miss a
//! field the result needs are left out rather than guessed at.

use std::time::Duration;

use nemo_core::audio::Origin;
use nemo_core::catalog::{
    BrowseId, CatalogError, ChannelRef, ChannelSummary, PlaylistSummary, VideoSummary,
};
use nemo_core::track::{Artwork, Thumbnail, Track, TrackId, VideoId};
use serde_json::Value;

/// The badge style of a video that is live now.
const LIVE_NOW: &str = "BADGE_STYLE_TYPE_LIVE_NOW";
/// The `contentType` of a playlist lockup.
const LOCKUP_PLAYLIST: &str = "LOCKUP_CONTENT_TYPE_PLAYLIST";
/// Longest browse or playlist id accepted, in bytes.
const MAX_ID_BYTES: usize = 128;
/// How deep the search for a channel id inside a card goes.
const MAX_DEPTH: usize = 24;

/// What one web search answered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WebResults {
    /// Videos, live streams included, in answer order.
    pub videos: Vec<VideoSummary>,
    /// Channels.
    pub channels: Vec<ChannelSummary>,
    /// Playlists.
    pub playlists: Vec<PlaylistSummary>,
}

/// Parse a web search answer.
///
/// # Errors
/// [`CatalogError::Parse`] when the text is not JSON or has no result list.
pub fn parse_search(text: &str) -> Result<WebResults, CatalogError> {
    let root: Value = serde_json::from_str(text)
        .map_err(|_| CatalogError::Parse("web search answer is not JSON".to_owned()))?;
    let sections = root
        .pointer(
            "/contents/twoColumnSearchResultsRenderer/primaryContents/sectionListRenderer/contents",
        )
        .and_then(Value::as_array)
        .ok_or_else(|| CatalogError::Parse("web search answer has no results".to_owned()))?;
    let mut out = WebResults::default();
    let items = sections
        .iter()
        .filter_map(|s| s.pointer("/itemSectionRenderer/contents")?.as_array())
        .flatten();
    for item in items {
        if let Some(video) = item.get("videoRenderer").and_then(video) {
            out.videos.push(video);
        } else if let Some(channel) = item.get("officialCardViewModel").and_then(card) {
            out.channels.push(channel);
        } else if let Some(playlist) = item.get("lockupViewModel").and_then(lockup) {
            out.playlists.push(playlist);
        }
    }
    Ok(out)
}

/// The text of a `simpleText`, `runs` or `content` field, trimmed; `None` when empty.
fn text(value: &Value) -> Option<String> {
    let joined = if let Some(simple) = value.get("simpleText").and_then(Value::as_str) {
        simple.to_owned()
    } else if let Some(runs) = value.get("runs").and_then(Value::as_array) {
        runs.iter()
            .filter_map(|r| r.get("text").and_then(Value::as_str))
            .collect()
    } else {
        value.get("content").and_then(Value::as_str)?.to_owned()
    };
    let trimmed = joined.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// A browse or playlist id made only of letters, digits, `-` and `_`.
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn channel_id(id: &str) -> Option<BrowseId> {
    (id.starts_with("UC") && valid_id(id)).then(|| BrowseId(id.to_owned()))
}

/// Thumbnails from a list of `{url, width, height}`; entries without a URL are skipped.
fn thumbnails(list: Option<&Value>) -> Vec<Thumbnail> {
    list.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|t| {
            let url = t.get("url")?.as_str()?;
            let size = |key: &str| {
                t.get(key)
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok())
                    .unwrap_or(0)
            };
            url.starts_with("https://").then(|| Thumbnail {
                url: url.to_owned(),
                width: size("width"),
                height: size("height"),
            })
        })
        .collect()
}

/// `"1:01:14"` or `"3:05"` as a duration.
fn length(text: &str) -> Option<Duration> {
    let mut secs: u64 = 0;
    let mut parts = 0;
    for part in text.trim().split(':') {
        let n: u64 = part.parse().ok()?;
        secs = secs.checked_mul(60)?.checked_add(n)?;
        parts += 1;
    }
    (1..=3).contains(&parts).then(|| Duration::from_secs(secs))
}

/// The channel named by the first run of a byline.
fn byline(value: &Value) -> Option<ChannelRef> {
    let run = value.pointer("/runs/0")?;
    let id = run
        .pointer("/navigationEndpoint/browseEndpoint/browseId")?
        .as_str()
        .and_then(channel_id)?;
    let name = run.get("text")?.as_str()?.trim();
    (!name.is_empty()).then(|| ChannelRef {
        id,
        name: name.to_owned(),
    })
}

fn video(r: &Value) -> Option<VideoSummary> {
    let video_id = VideoId::parse(r.get("videoId")?.as_str()?).ok()?;
    let title = text(r.get("title")?)?;
    let channel = r
        .get("longBylineText")
        .and_then(byline)
        .or_else(|| r.get("ownerText").and_then(byline));
    let live = r
        .get("badges")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|b| {
            b.pointer("/metadataBadgeRenderer/style")
                .and_then(Value::as_str)
                == Some(LIVE_NOW)
        });
    let duration = if live {
        None
    } else {
        r.get("lengthText").and_then(text).and_then(|t| length(&t))
    };
    let thumbs = thumbnails(r.pointer("/thumbnail/thumbnails"));
    let track = Track {
        id: TrackId(video_id.as_str().to_owned()),
        title,
        artists: channel.iter().map(|c| c.name.clone()).collect(),
        album: None,
        duration,
        origin: Origin::YoutubeMusic,
        video_id: Some(video_id),
        local_path: None,
        artwork: if thumbs.is_empty() {
            Artwork::None
        } else {
            Artwork::Remote(thumbs)
        },
    };
    Some(VideoSummary {
        track,
        channel,
        live,
        views: r.get("viewCountText").and_then(text),
    })
}

/// The first `browseEndpoint.browseId` that names a channel, anywhere under `value`.
fn find_channel_id(value: &Value, depth: usize) -> Option<BrowseId> {
    if depth > MAX_DEPTH {
        return None;
    }
    match value {
        Value::Object(map) => {
            if let Some(id) = map
                .get("browseEndpoint")
                .and_then(|e| e.get("browseId"))
                .and_then(Value::as_str)
                .and_then(channel_id)
            {
                return Some(id);
            }
            map.values().find_map(|v| find_channel_id(v, depth + 1))
        }
        Value::Array(list) => list.iter().find_map(|v| find_channel_id(v, depth + 1)),
        _ => None,
    }
}

/// A channel card. The spike saw the channel id in the card's header; a card without one is
/// left out, since a channel cannot be opened without it.
fn card(c: &Value) -> Option<ChannelSummary> {
    let header = c.get("header")?;
    let page = header.get("pageHeaderViewModel")?;
    let name = page
        .pointer("/title/dynamicTextViewModel/text/content")?
        .as_str()?
        .trim();
    if name.is_empty() {
        return None;
    }
    let id = find_channel_id(header, 0)?;
    let subscribers = page
        .pointer("/metadata/contentMetadataViewModel/metadataRows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| row.get("metadataParts")?.as_array())
        .flatten()
        .filter_map(|part| part.pointer("/text/content")?.as_str())
        .find(|t| t.contains("subscriber"))
        .map(str::to_owned);
    Some(ChannelSummary {
        id,
        name: name.to_owned(),
        subscribers,
        thumbnails: thumbnails(page.pointer("/image/contentPreviewImageViewModel/image/sources")),
    })
}

/// A playlist lockup; other lockups (videos, mixes in shelves) are not playlists here.
fn lockup(l: &Value) -> Option<PlaylistSummary> {
    if l.get("contentType")?.as_str()? != LOCKUP_PLAYLIST {
        return None;
    }
    let id = l.get("contentId")?.as_str()?;
    if !valid_id(id) {
        return None;
    }
    let meta = l.pointer("/metadata/lockupMetadataViewModel")?;
    let title = meta.pointer("/title/content")?.as_str()?.trim();
    if title.is_empty() {
        return None;
    }
    let author = meta
        .pointer("/metadata/contentMetadataViewModel/metadataRows/0/metadataParts/0/text/content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(str::to_owned);
    Some(PlaylistSummary {
        id: BrowseId(id.to_owned()),
        title: title.to_owned(),
        author,
        thumbnails: thumbnails(l.pointer(
            "/contentImage/collectionThumbnailViewModel/primaryThumbnail/thumbnailViewModel/image/sources",
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_parse_hours_minutes_and_seconds() {
        assert_eq!(length("1:01:14"), Some(Duration::from_secs(3674)));
        assert_eq!(length("3:05"), Some(Duration::from_secs(185)));
        assert_eq!(length("LIVE"), None);
        assert_eq!(length("1:2:3:4"), None);
        assert_eq!(length(""), None);
    }

    #[test]
    fn channel_ids_must_look_like_channel_ids() {
        assert!(channel_id("UCSJ4gkVC6NrvII8umztf0Ow").is_some());
        assert!(channel_id("VLPLxyz").is_none());
        assert!(channel_id("UC bad/id").is_none());
    }
}
