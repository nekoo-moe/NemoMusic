//! Parsers for a channel page (`POST /youtubei/v1/browse`, `WEB` client).
//!
//! Shapes follow the spike fixtures `tests/fixtures/web/browse-channel.json` (header and tab
//! endpoints) and `browse-videos.json` (uploads as `richGridRenderer` -> `richItemRenderer` ->
//! `lockupViewModel`). The tab `params` are read from the tab endpoints of the first answer and
//! never built or kept as constants. The playlists tab has no fixture: its parser looks for
//! playlist `lockupViewModel` items anywhere in the answer, the shape the search fixture shows,
//! and gives an empty list when it finds none. Only the first page of each tab is read.

use nemo_core::catalog::{
    BrowseId, CatalogError, ChannelRef, ChannelSummary, PlaylistSummary, VideoSummary,
};
use nemo_core::track::{Thumbnail, VideoId};
use serde_json::Value;

use super::parse::{channel_id, length, lockup, text, thumbnails, video_track};

/// The `contentType` of a video lockup.
const LOCKUP_VIDEO: &str = "LOCKUP_CONTENT_TYPE_VIDEO";
/// How deep the search for lockups goes.
const MAX_DEPTH: usize = 24;
/// Longest tab `params` accepted, in bytes.
const MAX_PARAMS_BYTES: usize = 512;

/// A channel page's header and the `params` of the tabs this app reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelPage {
    /// Name, subscribers and picture.
    pub summary: ChannelSummary,
    /// `params` of the Videos tab, if the page has one.
    pub videos_params: Option<String>,
    /// `params` of the Playlists tab, if the page has one.
    pub playlists_params: Option<String>,
}

/// `params` are copied from the answer into the next request, so they must be plain.
fn valid_params(params: &str) -> bool {
    !params.is_empty()
        && params.len() <= MAX_PARAMS_BYTES
        && params.bytes().all(|b| b.is_ascii_graphic())
}

/// The `params` of the tab whose page address ends in `/<name>`, e.g. `/videos`.
fn tab_params(tabs: &[Value], name: &str) -> Option<String> {
    tabs.iter()
        .filter_map(|t| t.pointer("/tabRenderer/endpoint"))
        .find(|e| {
            e.pointer("/commandMetadata/webCommandMetadata/url")
                .and_then(Value::as_str)
                .is_some_and(|u| u.rsplit('/').next() == Some(name))
        })
        .and_then(|e| e.pointer("/browseEndpoint/params")?.as_str())
        .filter(|p| valid_params(p))
        .map(str::to_owned)
}

fn subscribers(header: &Value) -> Option<String> {
    header
        .pointer("/metadata/contentMetadataViewModel/metadataRows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| row.get("metadataParts")?.as_array())
        .flatten()
        .filter_map(|part| part.pointer("/text/content")?.as_str())
        .find(|t| t.contains("subscriber"))
        .map(str::to_owned)
}

fn avatar(root: &Value, header: Option<&Value>) -> Vec<Thumbnail> {
    let from_header = header.and_then(|h| {
        h.pointer("/image/decoratedAvatarViewModel/avatar/avatarViewModel/image/sources")
    });
    let found = thumbnails(from_header);
    if found.is_empty() {
        thumbnails(root.pointer("/metadata/channelMetadataRenderer/avatar/thumbnails"))
    } else {
        found
    }
}

/// Parse the first answer for a channel: the header and the tab `params`.
///
/// # Errors
/// [`CatalogError::Parse`] when the text is not JSON or holds no channel id and name.
pub fn parse_channel(text_in: &str) -> Result<ChannelPage, CatalogError> {
    let root = json(text_in, "channel")?;
    let meta = root.pointer("/metadata/channelMetadataRenderer");
    let header = root.pointer("/header/pageHeaderRenderer/content/pageHeaderViewModel");
    let id = meta
        .and_then(|m| m.get("externalId")?.as_str())
        .and_then(channel_id)
        .ok_or_else(|| CatalogError::Parse("channel answer has no channel id".to_owned()))?;
    let name = header
        .and_then(|h| {
            h.pointer("/title/dynamicTextViewModel/text/content")?
                .as_str()
        })
        .or_else(|| meta.and_then(|m| m.get("title")?.as_str()))
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .ok_or_else(|| CatalogError::Parse("channel answer has no name".to_owned()))?;
    let tabs = root
        .pointer("/contents/twoColumnBrowseResultsRenderer/tabs")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    Ok(ChannelPage {
        summary: ChannelSummary {
            id,
            name: name.to_owned(),
            subscribers: header.and_then(subscribers),
            thumbnails: avatar(&root, header),
        },
        videos_params: tab_params(tabs, "videos"),
        playlists_params: tab_params(tabs, "playlists"),
    })
}

/// Push every `lockupViewModel` under `value` onto `out`, in answer order, down to
/// [`MAX_DEPTH`].
fn collect_lockups<'a>(value: &'a Value, depth: usize, out: &mut Vec<&'a Value>) {
    if depth > MAX_DEPTH {
        return;
    }
    match value {
        Value::Object(map) => {
            if let Some(l) = map.get("lockupViewModel") {
                out.push(l);
            }
            for (key, v) in map {
                if key != "lockupViewModel" {
                    collect_lockups(v, depth + 1, out);
                }
            }
        }
        Value::Array(list) => list.iter().for_each(|v| collect_lockups(v, depth + 1, out)),
        _ => {}
    }
}

/// The answer `text_in` as JSON; `what` names it in the error.
fn json(text_in: &str, what: &str) -> Result<Value, CatalogError> {
    serde_json::from_str(text_in)
        .map_err(|_| CatalogError::Parse(format!("{what} answer is not JSON")))
}

/// Every `lockupViewModel` in `root`, in answer order.
fn lockups(root: &Value) -> Vec<&Value> {
    let mut found = Vec::new();
    collect_lockups(root, 0, &mut found);
    found
}

/// The badge texts on the thumbnail, such as `1:13` or `LIVE`.
fn badge_texts(l: &Value) -> Vec<&str> {
    l.pointer("/contentImage/thumbnailViewModel/overlays")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|o| {
            o.pointer("/thumbnailBottomOverlayViewModel/badges")?
                .as_array()
        })
        .flatten()
        .filter_map(|b| b.pointer("/thumbnailBadgeViewModel/text")?.as_str())
        .collect()
}

fn upload(l: &Value, channel: &ChannelRef) -> Option<VideoSummary> {
    if l.get("contentType")?.as_str()? != LOCKUP_VIDEO {
        return None;
    }
    let video_id = VideoId::parse(l.get("contentId")?.as_str()?).ok()?;
    let meta = l.pointer("/metadata/lockupMetadataViewModel")?;
    let title = text(meta.get("title")?)?;
    let badges = badge_texts(l);
    let live = badges.iter().any(|t| t.trim().eq_ignore_ascii_case("live"));
    let duration = if live {
        None
    } else {
        badges.iter().find_map(|t| length(t))
    };
    let views = meta
        .pointer("/metadata/contentMetadataViewModel/metadataRows/0/metadataParts/0/text")
        .and_then(text);
    let thumbs = thumbnails(l.pointer("/contentImage/thumbnailViewModel/image/sources"));
    Some(VideoSummary {
        track: video_track(
            video_id,
            title,
            vec![channel.name.clone()],
            duration,
            thumbs,
        ),
        channel: Some(channel.clone()),
        live,
        views,
    })
}

/// Parse the Videos tab answer into uploads, in the order the page lists them. Items that miss
/// a field the result needs are left out.
///
/// # Errors
/// [`CatalogError::Parse`] when the text is not JSON.
pub fn parse_uploads(
    text_in: &str,
    channel: &ChannelRef,
) -> Result<Vec<VideoSummary>, CatalogError> {
    let root = json(text_in, "channel videos")?;
    Ok(lockups(&root)
        .into_iter()
        .filter_map(|l| upload(l, channel))
        .collect())
}

/// Parse the Playlists tab answer. The playlists belong to the channel, so no author is set.
///
/// # Errors
/// [`CatalogError::Parse`] when the text is not JSON.
pub fn parse_playlists(text_in: &str) -> Result<Vec<PlaylistSummary>, CatalogError> {
    let mut seen: Vec<BrowseId> = Vec::new();
    let mut out = Vec::new();
    let root = json(text_in, "channel playlists")?;
    for l in lockups(&root) {
        let Some(mut playlist) = lockup(l) else {
            continue;
        };
        if !seen.contains(&playlist.id) {
            seen.push(playlist.id.clone());
            playlist.author = None;
            out.push(playlist);
        }
    }
    Ok(out)
}
