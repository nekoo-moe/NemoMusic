//! Single feed items: cards, rows, buttons, episodes, and mood ids.

use nemo_core::catalog::{BrowseId, EpisodeSummary, FeedItem, MoodChip, PodcastSummary};
use nemo_core::track::{Thumbnail, Track};
use serde_json::Value;

use super::json::{find_key, item_thumbnails, joined_text, text_of};
use super::runs::Runs;
use super::{GREY_OUT, MAX_PARAMS};
use crate::catalog::{
    RawAlbum, RawArtist, RawPlaylist, RawSong, map_album, map_artist, map_playlist, map_song,
    parse_duration,
};

/// What a watch endpoint points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MediaKind {
    Audio,
    Video,
    Episode,
}

fn media_kind(music_video_type: Option<&str>) -> MediaKind {
    match music_video_type {
        Some(t) if t.contains("PODCAST_EPISODE") => MediaKind::Episode,
        Some(t) if t.contains("OMV") || t.contains("UGC") || t.contains("OFFICIAL_SOURCE") => {
            MediaKind::Video
        }
        _ => MediaKind::Audio,
    }
}

fn greyed_out(item: &Value) -> bool {
    item.get("musicItemRendererDisplayPolicy")
        .and_then(Value::as_str)
        == Some(GREY_OUT)
}

fn media_item(track: Track, kind: MediaKind) -> FeedItem {
    match kind {
        MediaKind::Audio => FeedItem::Song(track),
        MediaKind::Video => FeedItem::Video(track),
        MediaKind::Episode => FeedItem::Episode(EpisodeSummary {
            track,
            description: None,
            published: None,
        }),
    }
}

/// A page-id based item (album, artist, playlist, podcast), shared by cards and rows.
fn browse_item(
    id: &str,
    page_type: Option<&str>,
    title: String,
    sub: &Runs,
    thumbnails: Vec<Thumbnail>,
) -> Option<FeedItem> {
    let page_type = page_type.unwrap_or("");
    if page_type.contains("PODCAST_SHOW") || id.starts_with("MPSP") {
        return Some(FeedItem::Podcast(PodcastSummary {
            id: BrowseId(id.to_owned()),
            title,
            author: sub.artists.first().or_else(|| sub.names().next()).cloned(),
            thumbnails,
        }));
    }
    if page_type.contains("ALBUM") || id.starts_with("MPRE") {
        return map_album(&RawAlbum {
            id: id.to_owned(),
            title,
            artists: sub.artist_names(),
            year: sub.year_text(),
            thumbnails,
        })
        .map(FeedItem::Album);
    }
    if page_type.contains("PLAYLIST") || id.starts_with("VL") {
        return map_playlist(&RawPlaylist {
            id: id.to_owned(),
            title,
            author: sub.artists.first().or_else(|| sub.names().last()).cloned(),
            thumbnails,
        })
        .map(FeedItem::Playlist);
    }
    if page_type.contains("ARTIST") || page_type.contains("USER_CHANNEL") || id.starts_with("UC") {
        return map_artist(&RawArtist {
            id: id.to_owned(),
            name: title,
            thumbnails,
        })
        .map(FeedItem::Artist);
    }
    None
}

fn page_type_of(browse: &Value) -> Option<&str> {
    browse
        .pointer("/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType")
        .and_then(Value::as_str)
}

/// A card (`musicTwoRowItemRenderer`).
pub(super) fn two_row_item(item: &Value) -> Option<FeedItem> {
    if greyed_out(item) {
        return None;
    }
    let title = item.get("title").and_then(text_of)?;
    let thumbnails = item_thumbnails(item);
    let sub = Runs::from_value(item.get("subtitle"));
    let nav = item.get("navigationEndpoint")?;
    if let Some(watch) = nav.get("watchEndpoint") {
        let video_id = watch.get("videoId")?.as_str()?;
        let kind = media_kind(
            watch
                .pointer(
                    "/watchEndpointMusicSupportedConfigs/watchEndpointMusicConfig/musicVideoType",
                )
                .and_then(Value::as_str),
        );
        return map_song(&RawSong {
            video_id: video_id.to_owned(),
            title,
            artists: sub.artist_names(),
            album: sub.album.clone(),
            duration: None,
            thumbnails,
        })
        .map(|t| media_item(t, kind));
    }
    if let Some(browse) = nav.get("browseEndpoint") {
        let id = browse.get("browseId")?.as_str()?;
        return browse_item(id, page_type_of(browse), title, &sub, thumbnails);
    }
    let playlist = nav
        .pointer("/watchPlaylistEndpoint/playlistId")
        .and_then(Value::as_str)?;
    map_playlist(&RawPlaylist {
        id: format!("VL{playlist}"),
        title,
        author: sub.names().last().cloned(),
        thumbnails,
    })
    .map(FeedItem::Playlist)
}

fn flex_text(row: &Value, index: usize) -> Option<&Value> {
    row.get("flexColumns")?
        .as_array()?
        .get(index)?
        .get("musicResponsiveListItemFlexColumnRenderer")?
        .get("text")
}

/// The runs of each column after the title.
fn columns(row: &Value) -> Vec<Runs> {
    let count = row
        .get("flexColumns")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    (1..count)
        .map(|i| Runs::from_value(flex_text(row, i)))
        .collect()
}

/// All columns after the title as one [`Runs`].
fn detail_runs(row: &Value) -> Runs {
    let mut all = Runs::default();
    for part in columns(row) {
        all.artists.extend(part.artists);
        if all.album.is_none() {
            all.album = part.album;
        }
        all.plain.extend(part.plain);
    }
    all
}

fn row_duration(row: &Value, details: &Runs) -> Option<String> {
    row.pointer("/fixedColumns/0/musicResponsiveListItemFixedColumnRenderer/text")
        .and_then(text_of)
        .or_else(|| {
            details
                .plain
                .iter()
                .find(|t| parse_duration(t).is_some())
                .cloned()
        })
}

/// A playable row (`musicResponsiveListItemRenderer` with a video id).
pub(crate) fn song_row(row: &Value) -> Option<(Track, MediaKind)> {
    if greyed_out(row) {
        return None;
    }
    let title = flex_text(row, 0).and_then(text_of)?;
    let video_id = row
        .pointer("/playlistItemData/videoId")
        .or_else(|| find_key(row, "videoId"))?
        .as_str()?;
    let cols = columns(row);
    let mut artists: Vec<String> = cols.iter().flat_map(|c| c.artists.clone()).collect();
    if artists.is_empty() {
        artists = cols
            .first()
            .map(|c| c.names().cloned().collect())
            .unwrap_or_default();
    }
    artists.truncate(4);
    let details = detail_runs(row);
    let track = map_song(&RawSong {
        video_id: video_id.to_owned(),
        title,
        artists,
        album: details.album.clone(),
        duration: row_duration(row, &details),
        thumbnails: item_thumbnails(row),
    })?;
    let kind = media_kind(find_key(row, "musicVideoType").and_then(Value::as_str));
    Some((track, kind))
}

/// A row (`musicResponsiveListItemRenderer`): a song or video, or a linked page.
fn row_item(row: &Value) -> Option<FeedItem> {
    if greyed_out(row) {
        return None;
    }
    if let Some(browse) = row.pointer("/navigationEndpoint/browseEndpoint") {
        let id = browse.get("browseId")?.as_str()?;
        let title = flex_text(row, 0).and_then(text_of)?;
        let sub = detail_runs(row);
        return browse_item(id, page_type_of(browse), title, &sub, item_thumbnails(row));
    }
    let (track, kind) = song_row(row)?;
    Some(media_item(track, kind))
}
/// A mood or genre tile (`musicNavigationButtonRenderer`).
fn nav_button_item(button: &Value) -> Option<FeedItem> {
    let title = button.get("buttonText").and_then(text_of)?;
    let browse = button.pointer("/clickCommand/browseEndpoint")?;
    let id = browse.get("browseId")?.as_str()?;
    let params = browse.get("params").and_then(Value::as_str);
    let id = mood_id(id, params)?;
    Some(FeedItem::Mood(MoodChip { title, id }))
}

/// A podcast episode row (`musicMultiRowListItemRenderer`).
pub(super) fn episode_item(item: &Value) -> Option<EpisodeSummary> {
    let title = item.get("title").and_then(text_of)?;
    let video_id = find_key(item, "videoId")?.as_str()?;
    let track = map_song(&RawSong {
        video_id: video_id.to_owned(),
        title,
        artists: Vec::new(),
        album: None,
        duration: None,
        thumbnails: item_thumbnails(item),
    })?;
    Some(EpisodeSummary {
        track,
        description: item.get("description").and_then(joined_text),
        published: item.get("subtitle").and_then(text_of),
    })
}
/// One entry of a shelf, whatever its renderer.
pub(super) fn feed_item(entry: &Value) -> Option<FeedItem> {
    if let Some(card) = entry.get("musicTwoRowItemRenderer") {
        return two_row_item(card);
    }
    if let Some(row) = entry.get("musicResponsiveListItemRenderer") {
        return row_item(row);
    }
    if let Some(button) = entry.get("musicNavigationButtonRenderer") {
        return nav_button_item(button);
    }
    entry
        .get("musicMultiRowListItemRenderer")
        .and_then(episode_item)
        .map(FeedItem::Episode)
}

// ---------------------------------------------------------------------------------------------
// Mood ids
// ---------------------------------------------------------------------------------------------

fn safe_id(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 128
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn safe_params(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_PARAMS
        && text.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'%' | b'=' | b'+' | b'/' | b'.')
        })
}

/// The opaque id of a mood: the page to ask and its `params`, joined by `|`.
pub(crate) fn mood_id(browse_id: &str, params: Option<&str>) -> Option<BrowseId> {
    if !safe_id(browse_id) {
        return None;
    }
    match params {
        Some(p) if safe_params(p) => Some(BrowseId(format!("{browse_id}|{p}"))),
        Some(_) => None,
        None => Some(BrowseId(browse_id.to_owned())),
    }
}

/// Split a mood id back into `(browse id, params)`. `None` when it is not one of ours.
pub(crate) fn split_mood_id(id: &BrowseId) -> Option<(&str, Option<&str>)> {
    match id.0.split_once('|') {
        Some((page, params)) if safe_id(page) && safe_params(params) => Some((page, Some(params))),
        Some(_) => None,
        None => safe_id(&id.0).then_some((id.0.as_str(), None)),
    }
}
