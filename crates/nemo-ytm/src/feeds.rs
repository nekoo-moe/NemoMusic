//! Tolerant readers for YouTube Music pages: home, explore, moods, library, history, playlists
//! and podcasts.
//!
//! `ytmapi-rs` 0.3.4 parses a fixed set of layouts and fails a whole page when one part is
//! unknown, so these readers work on the raw JSON instead. They look for the renderers they
//! know, classify each entry by its navigation endpoint (page type and id prefix, which do not
//! change with the account language) and drop what they cannot understand, so one odd entry
//! never hides the rest of a page. Layouts were checked against live answers on 2026-11-11 and
//! cross-checked with `ytmusicapi` (`parsers/browsing.py`, `parsers/library.py`,
//! `parsers/podcasts.py`, `mixins/explore.py`).

use std::time::Duration;

use nemo_core::catalog::{
    AlbumSummary, ArtistSummary, BrowseId, EpisodeSummary, Explore, FeedItem, FeedShelf,
    HistorySection, HomeFeed, MoodChip, PlaylistSummary, PodcastDetail, PodcastSummary, ShelfKind,
};
use nemo_core::track::{Thumbnail, Track};
use serde_json::Value;

use crate::catalog::{
    RawAlbum, RawArtist, RawPlaylist, RawSong, map_album, map_artist, map_playlist, map_song,
    map_thumbnails, parse_duration, parse_year,
};

/// Most shelves kept from one page.
const MAX_SHELVES: usize = 24;
/// Most items kept per shelf.
const MAX_ITEMS: usize = 60;
/// Longest `params` blob accepted in a mood id.
const MAX_PARAMS: usize = 512;
/// Most tracks kept from a watch playlist (a radio can be endless).
const MAX_WATCH_TRACKS: usize = 200;

const GREY_OUT: &str = "MUSIC_ITEM_RENDERER_DISPLAY_POLICY_GREY_OUT";

// ---------------------------------------------------------------------------------------------
// JSON helpers
// ---------------------------------------------------------------------------------------------

/// First value stored under `key` anywhere inside `value` (depth-first).
pub(crate) fn find_key<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map
            .get(key)
            .or_else(|| map.values().find_map(|v| find_key(v, key))),
        Value::Array(items) => items.iter().find_map(|v| find_key(v, key)),
        _ => None,
    }
}

/// Text of the first run (or `simpleText`), trimmed; `None` when empty.
pub(crate) fn text_of(value: &Value) -> Option<String> {
    let text = value
        .pointer("/runs/0/text")
        .or_else(|| value.get("simpleText"))?
        .as_str()?
        .trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// All runs joined, trimmed; `None` when empty.
fn joined_text(value: &Value) -> Option<String> {
    let runs = value.get("runs")?.as_array()?;
    let text: String = runs
        .iter()
        .filter_map(|r| r.get("text").and_then(Value::as_str))
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

fn runs_of(value: &Value) -> &[Value] {
    value
        .get("runs")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// All sizes of the first `thumbnails` list under `value`.
pub(crate) fn thumbnails_of(value: &Value) -> Vec<Thumbnail> {
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

/// The cover of an item: `thumbnailRenderer` (cards) or `thumbnail` (rows). Never the small
/// channel picture in `thumbnailCornerOverlay`.
fn item_thumbnails(item: &Value) -> Vec<Thumbnail> {
    item.get("thumbnailRenderer")
        .or_else(|| item.get("thumbnail"))
        .map(thumbnails_of)
        .unwrap_or_default()
}

fn is_separator(text: &str) -> bool {
    matches!(text, "•" | "·" | "&" | "," | "-" | "–" | "|")
}

fn is_type_label(text: &str) -> bool {
    matches!(
        text,
        "Song"
            | "Video"
            | "Album"
            | "Single"
            | "EP"
            | "Playlist"
            | "Artist"
            | "Podcast"
            | "Episode"
    )
}

fn has_digit(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_digit())
}

/// What the runs of a subtitle or row column hold.
#[derive(Debug, Default)]
struct Runs {
    /// Runs that link to an artist or channel (`UC...`).
    artists: Vec<String>,
    /// The run that links to an album (`MPRE...`).
    album: Option<String>,
    /// Runs without a link, separators removed.
    plain: Vec<String>,
}

impl Runs {
    fn read(runs: &[Value]) -> Self {
        let mut out = Self::default();
        for run in runs {
            let Some(text) = run.get("text").and_then(Value::as_str).map(str::trim) else {
                continue;
            };
            if text.is_empty() || is_separator(text) {
                continue;
            }
            let id = run
                .pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(Value::as_str);
            match id {
                Some(id) if id.starts_with("UC") => out.artists.push(text.to_owned()),
                Some(id) if id.starts_with("MPRE") => out.album = Some(text.to_owned()),
                Some(_) => {}
                None => out.plain.push(text.to_owned()),
            }
        }
        out
    }

    fn from_value(value: Option<&Value>) -> Self {
        value.map(|v| Self::read(runs_of(v))).unwrap_or_default()
    }

    /// Plain texts that can be a name: no digits, not a type label.
    fn names(&self) -> impl Iterator<Item = &String> {
        self.plain
            .iter()
            .filter(|t| !is_type_label(t) && !has_digit(t))
    }

    /// Artist names: linked ones, else plain name-like texts.
    fn artist_names(&self) -> Vec<String> {
        if self.artists.is_empty() {
            self.names().cloned().collect()
        } else {
            self.artists.clone()
        }
    }

    fn year_text(&self) -> Option<String> {
        self.plain.iter().find(|t| parse_year(t).is_some()).cloned()
    }
}

// ---------------------------------------------------------------------------------------------
// Items
// ---------------------------------------------------------------------------------------------

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
fn two_row_item(item: &Value) -> Option<FeedItem> {
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
fn episode_item(item: &Value) -> Option<EpisodeSummary> {
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
fn feed_item(entry: &Value) -> Option<FeedItem> {
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

// ---------------------------------------------------------------------------------------------
// Shelves
// ---------------------------------------------------------------------------------------------

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
fn section_contents(root: &Value) -> &[Value] {
    find_key(root, "sectionListContinuation")
        .or_else(|| find_key(root, "sectionListRenderer"))
        .and_then(|s| s.get("contents"))
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Where the next-part token of a home page lives.
const HOME_SCOPES: [&str; 2] = ["sectionListContinuation", "sectionListRenderer"];
/// Where the next-part token of a list page lives, most specific first.
const LIST_SCOPES: [&str; 7] = [
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
fn parse_root(json: &str) -> Option<Value> {
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

// ---------------------------------------------------------------------------------------------
// Lists: library, liked songs, playlist pages, history
// ---------------------------------------------------------------------------------------------

/// One part of a long list and the token of the next part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Page<T> {
    pub items: Vec<T>,
    pub next: Option<String>,
}

/// The entries of the list on this page: playlist shelf, plain shelf or grid, first page or
/// continuation.
fn list_entries(root: &Value) -> Vec<&Value> {
    let containers: [(&str, &str); 7] = [
        ("musicPlaylistShelfRenderer", "contents"),
        ("musicPlaylistShelfContinuation", "contents"),
        ("musicShelfContinuation", "contents"),
        ("gridContinuation", "items"),
        ("musicShelfRenderer", "contents"),
        ("gridRenderer", "items"),
        ("appendContinuationItemsAction", "continuationItems"),
    ];
    for (key, field) in containers {
        if let Some(entries) = find_key(root, key)
            .and_then(|c| c.get(field))
            .and_then(Value::as_array)
        {
            return entries.iter().collect();
        }
    }
    Vec::new()
}

fn list_page<T>(json: &str, pick: impl Fn(FeedItem) -> Option<T>) -> Page<T> {
    let Some(root) = parse_root(json) else {
        return Page {
            items: Vec::new(),
            next: None,
        };
    };
    Page {
        items: list_entries(&root)
            .into_iter()
            .filter_map(feed_item)
            .filter_map(pick)
            .collect(),
        next: continuation_token(&root, &LIST_SCOPES),
    }
}

/// The account's playlists.
pub(crate) fn parse_library_playlists(json: &str) -> Page<PlaylistSummary> {
    list_page(json, |i| match i {
        FeedItem::Playlist(p) => Some(p),
        _ => None,
    })
}

/// The account's saved albums.
pub(crate) fn parse_library_albums(json: &str) -> Page<AlbumSummary> {
    list_page(json, |i| match i {
        FeedItem::Album(a) => Some(a),
        _ => None,
    })
}

/// The account's saved artists.
pub(crate) fn parse_library_artists(json: &str) -> Page<ArtistSummary> {
    list_page(json, |i| match i {
        FeedItem::Artist(a) => Some(a),
        _ => None,
    })
}

/// Playable rows of a playlist-like page (liked songs, a playlist, an album-less list).
pub(crate) fn parse_song_rows(json: &str) -> Page<Track> {
    let Some(root) = parse_root(json) else {
        return Page {
            items: Vec::new(),
            next: None,
        };
    };
    Page {
        items: list_entries(&root)
            .into_iter()
            .filter_map(|e| e.get("musicResponsiveListItemRenderer"))
            .filter_map(song_row)
            .map(|(track, _)| track)
            .collect(),
        next: continuation_token(&root, &LIST_SCOPES),
    }
}

/// Header of a playlist page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PlaylistHeader {
    pub title: Option<String>,
    pub author: Option<String>,
    pub year: Option<u16>,
    pub thumbnails: Vec<Thumbnail>,
}

/// Title, owner and cover of a playlist page (also the editable header of one's own lists).
pub(crate) fn parse_playlist_header(json: &str) -> PlaylistHeader {
    let Some(root) = parse_root(json) else {
        return PlaylistHeader::default();
    };
    let Some(header) = find_key(&root, "musicResponsiveHeaderRenderer")
        .or_else(|| find_key(&root, "musicDetailHeaderRenderer"))
    else {
        return PlaylistHeader::default();
    };
    let author = header
        .get("straplineTextOne")
        .and_then(text_of)
        .or_else(|| {
            let sub = Runs::from_value(header.get("subtitle"));
            sub.artists.first().cloned()
        });
    let year = Runs::from_value(header.get("subtitle"))
        .year_text()
        .and_then(|t| parse_year(&t));
    PlaylistHeader {
        title: header.get("title").and_then(text_of),
        author,
        year,
        thumbnails: item_thumbnails(header),
    }
}

/// Tracks of a watch playlist (`next` answer): radios and mixes that have no browse page
/// (`RDAMVM...`, artist radios). Unplayable entries are dropped.
pub(crate) fn parse_watch_playlist(json: &str) -> Vec<Track> {
    let Some(root) = parse_root(json) else {
        return Vec::new();
    };
    let Some(contents) = find_key(&root, "playlistPanelRenderer")
        .and_then(|p| p.get("contents"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    contents
        .iter()
        .filter_map(|entry| {
            entry.get("playlistPanelVideoRenderer").or_else(|| {
                entry.pointer(
                    "/playlistPanelVideoWrapperRenderer/primaryRenderer/playlistPanelVideoRenderer",
                )
            })
        })
        .filter(|v| v.get("unplayableText").is_none())
        .filter_map(|v| {
            let byline = Runs::from_value(v.get("longBylineText"));
            map_song(&RawSong {
                video_id: v.get("videoId")?.as_str()?.to_owned(),
                title: v.get("title").and_then(text_of)?,
                artists: byline.artist_names(),
                album: byline.album.clone(),
                duration: v.get("lengthText").and_then(text_of),
                thumbnails: v.get("thumbnail").map(thumbnails_of).unwrap_or_default(),
            })
        })
        .take(MAX_WATCH_TRACKS)
        .collect()
}

/// The listening history, one section per period.
pub(crate) fn parse_history(json: &str) -> Vec<HistorySection> {
    let Some(root) = parse_root(json) else {
        return Vec::new();
    };
    section_contents(&root)
        .iter()
        .filter_map(|s| s.get("musicShelfRenderer"))
        .filter_map(|shelf| {
            let title = shelf.get("title").and_then(text_of)?;
            let tracks: Vec<Track> = shelf
                .get("contents")?
                .as_array()?
                .iter()
                .filter_map(|e| e.get("musicResponsiveListItemRenderer"))
                .filter_map(song_row)
                .map(|(t, _)| t)
                .collect();
            (!tracks.is_empty()).then_some(HistorySection { title, tracks })
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Podcasts
// ---------------------------------------------------------------------------------------------

fn collect_key<'a>(value: &'a Value, key: &str, out: &mut Vec<&'a Value>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if k == key {
                    out.push(v);
                } else {
                    collect_key(v, key, out);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_key(item, key, out);
            }
        }
        _ => {}
    }
}

/// A podcast page: header and episodes.
pub(crate) fn parse_podcast(json: &str, requested: &BrowseId) -> Option<PodcastDetail> {
    let root = parse_root(json)?;
    let header = find_key(&root, "musicResponsiveHeaderRenderer")
        .or_else(|| find_key(&root, "musicDetailHeaderRenderer"))?;
    let title = header.get("title").and_then(text_of)?;
    let author = header
        .get("straplineTextOne")
        .and_then(text_of)
        .or_else(|| {
            Runs::from_value(header.get("subtitle"))
                .artists
                .first()
                .cloned()
        });
    let description = find_key(&root, "musicDescriptionShelfRenderer")
        .and_then(|d| d.get("description"))
        .and_then(joined_text);
    let mut rows = Vec::new();
    collect_key(&root, "musicMultiRowListItemRenderer", &mut rows);
    let mut episodes: Vec<EpisodeSummary> = rows.into_iter().filter_map(episode_item).collect();
    if episodes.is_empty() {
        // Older layout: episodes as plain rows.
        let mut plain = Vec::new();
        collect_key(&root, "musicResponsiveListItemRenderer", &mut plain);
        episodes = plain
            .into_iter()
            .filter_map(song_row)
            .map(|(track, _)| EpisodeSummary {
                track,
                description: None,
                published: None,
            })
            .collect();
    }
    let show = author.clone();
    for episode in &mut episodes {
        if episode.track.artists.is_empty() {
            episode.track.artists.extend(show.clone());
        }
    }
    Some(PodcastDetail {
        summary: PodcastSummary {
            id: requested.clone(),
            title,
            author,
            thumbnails: item_thumbnails(header),
        },
        description,
        episodes,
    })
}

/// Pause between the parts of a long list, so a page of results is fetched at human pace.
pub(crate) const PAGE_PAUSE: Duration = Duration::from_millis(400);

#[cfg(test)]
#[allow(
    clippy::assert_is_empty,
    clippy::needless_pass_by_value,
    reason = "readable test assertions; helpers take JSON builders by value"
)]
mod tests {
    use super::*;
    use serde_json::json;

    const ID: &str = "abcdefghijk";
    const ID2: &str = "lmnopqrstuv";

    fn runs(texts: &[&str]) -> Value {
        json!({ "runs": texts.iter().map(|t| json!({ "text": t })).collect::<Vec<_>>() })
    }

    /// A run that links to a page.
    fn link(text: &str, browse_id: &str) -> Value {
        json!({ "text": text, "navigationEndpoint": { "browseEndpoint": { "browseId": browse_id } } })
    }

    fn thumbs(url: &str) -> Value {
        json!({ "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
            { "url": url, "width": 120, "height": 120 }
        ] } } })
    }

    fn browse(id: &str, page_type: &str) -> Value {
        json!({ "browseEndpoint": {
            "browseId": id,
            "browseEndpointContextSupportedConfigs": { "browseEndpointContextMusicConfig": {
                "pageType": page_type
            } }
        } })
    }

    fn card(title: &str, subtitle: Value, nav: Value) -> Value {
        json!({ "musicTwoRowItemRenderer": {
            "title": runs(&[title]),
            "subtitle": subtitle,
            "thumbnailRenderer": thumbs("https://img.invalid/cover"),
            "thumbnailCornerOverlay": thumbs("https://img.invalid/avatar"),
            "navigationEndpoint": nav
        } })
    }

    fn watch(video_id: &str, kind: &str) -> Value {
        json!({ "watchEndpoint": {
            "videoId": video_id,
            "watchEndpointMusicSupportedConfigs": { "watchEndpointMusicConfig": {
                "musicVideoType": kind
            } }
        } })
    }

    fn row(
        title: &str,
        video_id: &str,
        artist: &str,
        album: Option<&str>,
        duration: &str,
    ) -> Value {
        let mut columns = vec![
            json!({ "musicResponsiveListItemFlexColumnRenderer": { "text": runs(&[title]) } }),
            json!({ "musicResponsiveListItemFlexColumnRenderer": {
                "text": { "runs": [ link(artist, "UCsynthetic1") ] } } }),
        ];
        if let Some(album) = album {
            columns.push(json!({ "musicResponsiveListItemFlexColumnRenderer": {
                "text": { "runs": [ link(album, "MPREbsynthetic1") ] } } }));
        }
        json!({ "musicResponsiveListItemRenderer": {
            "thumbnail": thumbs("https://img.invalid/row"),
            "flexColumns": columns,
            "fixedColumns": [ { "musicResponsiveListItemFixedColumnRenderer": {
                "text": runs(&[duration]) } } ],
            "playlistItemData": { "videoId": video_id }
        } })
    }

    fn carousel(title: &str, strapline: Option<&str>, items: Vec<Value>) -> Value {
        let mut header = json!({ "title": runs(&[title]) });
        if let Some(line) = strapline {
            header["strapline"] = runs(&[line]);
        }
        json!({ "musicCarouselShelfRenderer": {
            "header": { "musicCarouselShelfBasicHeaderRenderer": header },
            "contents": items
        } })
    }

    fn section_list(sections: Vec<Value>) -> Value {
        json!({ "contents": { "singleColumnBrowseResultsRenderer": { "tabs": [ {
            "tabRenderer": { "content": { "sectionListRenderer": { "contents": sections } } }
        } ] } } })
    }

    fn chips() -> Value {
        json!({ "chipCloudRenderer": { "chips": [
            { "chipCloudChipRenderer": { "text": runs(&["Relax"]), "navigationEndpoint": {
                "browseEndpoint": { "browseId": "FEmusic_home", "params": "ggM8abc%3D" } } } },
            { "chipCloudChipRenderer": { "text": runs(&["No params"]), "navigationEndpoint": {
                "browseEndpoint": { "browseId": "FEmusic_home" } } } }
        ] } })
    }

    #[test]
    fn home_page_has_chips_subtitles_podcasts_and_a_next_token() {
        let mut page = section_list(vec![
            carousel(
                "Listen again",
                Some("SOMEONE"),
                vec![card(
                    "My Mix",
                    json!({ "runs": [ link("Someone", "UCsynthetic9"), { "text": " • " }, { "text": "20 songs" } ] }),
                    browse("VLPLsynthetic1", "MUSIC_PAGE_TYPE_PLAYLIST"),
                )],
            ),
            carousel(
                "Shows",
                None,
                vec![card(
                    "A Show",
                    json!({ "runs": [ link("The Host", "UCsynthetic2") ] }),
                    browse("MPSPsynthetic1", "MUSIC_PAGE_TYPE_PODCAST_SHOW_DETAIL_PAGE"),
                )],
            ),
            carousel(
                "New albums",
                None,
                vec![card(
                    "An Album",
                    json!({ "runs": [ { "text": "Album" }, { "text": " • " }, link("Artist One", "UCsynthetic3"), { "text": " • " }, { "text": "2024" } ] }),
                    browse("MPREbsynthetic2", "MUSIC_PAGE_TYPE_ALBUM"),
                )],
            ),
        ]);
        page["contents"]["singleColumnBrowseResultsRenderer"]["tabs"][0]["tabRenderer"]["content"]
            ["sectionListRenderer"]["header"] = chips();
        page["contents"]["singleColumnBrowseResultsRenderer"]["tabs"][0]["tabRenderer"]["content"]
            ["sectionListRenderer"]["continuations"] =
            json!([ { "nextContinuationData": { "continuation": "tok123" } } ]);
        let (feed, next) = parse_home_page(&page.to_string());
        assert_eq!(next.as_deref(), Some("tok123"));
        // The chip without params cannot filter anything and is dropped.
        assert_eq!(feed.chips.len(), 1);
        assert_eq!(feed.chips[0].title, "Relax");
        assert_eq!(feed.chips[0].id.0, "FEmusic_home|ggM8abc%3D");
        assert_eq!(feed.shelves.len(), 3);
        assert_eq!(feed.shelves[0].subtitle.as_deref(), Some("SOMEONE"));
        assert_eq!(feed.shelves[0].kind, ShelfKind::Cards);
        match &feed.shelves[0].items[0] {
            FeedItem::Playlist(p) => {
                assert_eq!(p.author.as_deref(), Some("Someone"));
                assert_eq!(p.thumbnails[0].url, "https://img.invalid/cover");
            }
            other => panic!("{other:?}"),
        }
        match &feed.shelves[1].items[0] {
            FeedItem::Podcast(p) => {
                assert_eq!(p.id.0, "MPSPsynthetic1");
                assert_eq!(p.author.as_deref(), Some("The Host"));
            }
            other => panic!("{other:?}"),
        }
        match &feed.shelves[2].items[0] {
            FeedItem::Album(a) => {
                assert_eq!(a.year, Some(2024));
                assert_eq!(a.artists, vec!["Artist One".to_owned()]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn translated_labels_do_not_leak_into_names() {
        let item = card(
            "Mix",
            json!({ "runs": [ { "text": "Danh sách phát" }, { "text": " • " }, link("Chủ", "UCsynthetic4"), { "text": " • " }, { "text": "139 bài hát" } ] }),
            browse("VLPLsynthetic2", "MUSIC_PAGE_TYPE_PLAYLIST"),
        );
        match two_row_item(&item["musicTwoRowItemRenderer"]) {
            Some(FeedItem::Playlist(p)) => assert_eq!(p.author.as_deref(), Some("Chủ")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn songs_and_videos_are_told_apart_by_the_music_video_type() {
        let song = card(
            "A Song",
            json!({ "runs": [ link("Artist One", "UCsynthetic1") ] }),
            watch(ID, "MUSIC_VIDEO_TYPE_ATV"),
        );
        let video = card(
            "A Video",
            json!({ "runs": [ link("Artist Two", "UCsynthetic2"), { "text": " • " }, { "text": "1.2M views" } ] }),
            watch(ID2, "MUSIC_VIDEO_TYPE_OMV"),
        );
        let upload = card(
            "Upload",
            json!({ "runs": [ { "text": "Some Channel" } ] }),
            watch(ID, "MUSIC_VIDEO_TYPE_UGC"),
        );
        let episode = card(
            "Episode",
            json!({ "runs": [] }),
            watch(ID, "MUSIC_VIDEO_TYPE_PODCAST_EPISODE"),
        );
        let shelves =
            parse_sections(&[carousel("Mixed", None, vec![song, video, upload, episode])]);
        let items = &shelves[0].items;
        assert!(matches!(&items[0], FeedItem::Song(t) if t.artists == ["Artist One"]));
        assert!(matches!(&items[1], FeedItem::Video(t) if t.artists == ["Artist Two"]));
        assert!(matches!(&items[2], FeedItem::Video(t) if t.artists == ["Some Channel"]));
        assert!(matches!(&items[3], FeedItem::Episode(_)));
        assert_eq!(shelves[0].kind, ShelfKind::Cards);
    }

    #[test]
    fn a_shelf_of_rows_is_a_quick_picks_list() {
        let shelves = parse_sections(&[carousel(
            "Quick picks",
            None,
            vec![row("Row Song", ID, "Artist One", Some("An Album"), "3:45")],
        )]);
        assert_eq!(shelves[0].kind, ShelfKind::QuickPicks);
        match &shelves[0].items[0] {
            FeedItem::Song(t) => {
                assert_eq!(t.title, "Row Song");
                assert_eq!(t.artists, vec!["Artist One".to_owned()]);
                assert_eq!(t.album.as_deref(), Some("An Album"));
                assert_eq!(t.duration, Some(Duration::from_secs(225)));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn unusable_entries_are_dropped_and_the_rest_stays() {
        let grey = json!({ "musicTwoRowItemRenderer": {
            "title": runs(&["Greyed"]),
            "musicItemRendererDisplayPolicy": GREY_OUT,
            "navigationEndpoint": watch(ID, "MUSIC_VIDEO_TYPE_ATV")
        } });
        let bad_id = card(
            "Bad id",
            json!({ "runs": [] }),
            watch("short", "MUSIC_VIDEO_TYPE_ATV"),
        );
        let no_nav = json!({ "musicTwoRowItemRenderer": { "title": runs(&["No nav"]) } });
        let create = card(
            "New playlist",
            json!({}),
            json!({ "createPlaylistEndpoint": { "params": "x" } }),
        );
        let other_page = card(
            "Other",
            json!({}),
            browse("FEsomething", "MUSIC_PAGE_TYPE_UNKNOWN"),
        );
        let good = card(
            "Good",
            json!({}),
            browse("VLPLsynthetic3", "MUSIC_PAGE_TYPE_PLAYLIST"),
        );
        let shelves = parse_sections(&[
            carousel(
                "Mixed",
                None,
                vec![grey, bad_id, no_nav, create, other_page, good],
            ),
            carousel("Empty", None, vec![]),
            json!({ "musicDescriptionShelfRenderer": {} }),
            carousel("", None, vec![]),
        ]);
        assert_eq!(shelves.len(), 1);
        assert_eq!(shelves[0].items.len(), 1);
        assert!(parse_sections(&[]).is_empty());
        assert_eq!(parse_home_page("nope").0, HomeFeed::default());
    }

    #[test]
    fn mood_tiles_become_chips_and_ids_round_trip() {
        let tile = json!({ "musicNavigationButtonRenderer": {
            "buttonText": runs(&["Chill"]),
            "clickCommand": { "browseEndpoint": {
                "browseId": "FEmusic_moods_and_genres_category", "params": "ggMPOg32abc" } }
        } });
        let bad = json!({ "musicNavigationButtonRenderer": {
            "buttonText": runs(&["Evil"]),
            "clickCommand": { "browseEndpoint": { "browseId": "FEx", "params": "a b;c" } }
        } });
        let shelves = parse_sections(&[carousel("Moods", None, vec![tile, bad])]);
        assert_eq!(shelves[0].kind, ShelfKind::Chips);
        assert_eq!(shelves[0].items.len(), 1);
        let FeedItem::Mood(mood) = &shelves[0].items[0] else {
            panic!("not a mood");
        };
        assert_eq!(mood.id.0, "FEmusic_moods_and_genres_category|ggMPOg32abc");
        assert_eq!(
            split_mood_id(&mood.id),
            Some(("FEmusic_moods_and_genres_category", Some("ggMPOg32abc")))
        );
        assert_eq!(
            split_mood_id(&BrowseId("FEx".to_owned())),
            Some(("FEx", None))
        );
        for bad in ["", "a b|c", "FEx|a;b", "|abc", "FEx|", "a/b"] {
            assert_eq!(split_mood_id(&BrowseId(bad.to_owned())), None, "{bad:?}");
        }
        assert_eq!(mood_id("FEx", Some("a b")), None);
    }

    #[test]
    fn library_playlists_skip_the_create_tile_and_follow_the_token() {
        let create = json!({ "musicTwoRowItemRenderer": {
            "title": runs(&["New playlist"]),
            "navigationEndpoint": { "createPlaylistEndpoint": { "params": "x" } }
        } });
        let liked = card(
            "Liked music",
            json!({ "runs": [ { "text": "Auto playlist" } ] }),
            browse("VLLM", "MUSIC_PAGE_TYPE_PLAYLIST"),
        );
        let own = card(
            "Mine",
            json!({ "runs": [ { "text": "Playlist" }, { "text": " • " }, { "text": "139 songs" } ] }),
            browse("VLPLsynthetic4", "MUSIC_PAGE_TYPE_PLAYLIST"),
        );
        let first = json!({ "contents": { "singleColumnBrowseResultsRenderer": { "tabs": [ {
            "tabRenderer": { "content": { "sectionListRenderer": { "contents": [
                { "gridRenderer": {
                    "items": [create, liked, own],
                    "continuations": [ { "nextContinuationData": { "continuation": "next1" } } ]
                } }
            ] } } }
        } ] } } });
        let page = parse_library_playlists(&first.to_string());
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].id.0, "VLLM");
        assert_eq!(page.items[1].title, "Mine");
        assert_eq!(page.items[1].author, None);
        assert_eq!(page.next.as_deref(), Some("next1"));

        let more = card(
            "More",
            json!({}),
            browse("VLPLsynthetic5", "MUSIC_PAGE_TYPE_PLAYLIST"),
        );
        let second = json!({ "continuationContents": { "gridContinuation": { "items": [more] } } });
        let page = parse_library_playlists(&second.to_string());
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.next, None);
        assert_eq!(parse_library_playlists("nope").items.len(), 0);
    }

    #[test]
    fn library_albums_and_artists() {
        let album = card(
            "Saved",
            json!({ "runs": [ { "text": "Album" }, { "text": " • " }, link("Artist One", "UCsynthetic3"), { "text": " • " }, { "text": "2020" } ] }),
            browse("MPREbsynthetic3", "MUSIC_PAGE_TYPE_ALBUM"),
        );
        let grid = json!({ "gridRenderer": { "items": [album] } });
        let albums = parse_library_albums(&json!({ "contents": [grid] }).to_string());
        assert_eq!(albums.items.len(), 1);
        assert_eq!(albums.items[0].year, Some(2020));

        let artist = json!({ "musicResponsiveListItemRenderer": {
            "thumbnail": thumbs("https://img.invalid/a"),
            "flexColumns": [ { "musicResponsiveListItemFlexColumnRenderer": { "text": runs(&["Band"]) } } ],
            "navigationEndpoint": browse("UCsynthetic7", "MUSIC_PAGE_TYPE_ARTIST")
        } });
        let shelf = json!({ "musicShelfRenderer": {
            "contents": [artist],
            "continuations": [ { "nextContinuationData": { "continuation": "more" } } ]
        } });
        let artists = parse_library_artists(&json!({ "contents": [shelf] }).to_string());
        assert_eq!(artists.items.len(), 1);
        assert_eq!(artists.items[0].name, "Band");
        assert_eq!(artists.items[0].thumbnails.len(), 1);
        assert_eq!(artists.next.as_deref(), Some("more"));
    }

    #[test]
    fn liked_songs_rows_skip_greyed_items_and_only_use_their_own_token() {
        let mut grey = row("Gone", ID2, "Artist", None, "1:00");
        grey["musicResponsiveListItemRenderer"]["musicItemRendererDisplayPolicy"] = json!(GREY_OUT);
        let page = json!({
            "contents": { "twoColumnBrowseResultsRenderer": { "secondaryContents": {
                "sectionListRenderer": {
                    "contents": [ { "musicPlaylistShelfRenderer": {
                        "contents": [
                            row("First", ID, "Artist One", Some("Album X"), "3:45"),
                            grey,
                            json!({ "musicResponsiveListItemRenderer": { "flexColumns": [] } })
                        ],
                        "continuations": [ { "nextContinuationData": { "continuation": "rows2" } } ]
                    } } ],
                    "continuations": [ { "nextContinuationData": { "continuation": "elsewhere" } } ]
                }
            } } }
        });
        let parsed = parse_song_rows(&page.to_string());
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.items[0].title, "First");
        assert_eq!(parsed.next.as_deref(), Some("rows2"));

        let later = json!({ "continuationContents": { "musicPlaylistShelfContinuation": {
            "contents": [ row("Second", ID2, "Artist Two", None, "2:00") ]
        } } });
        let parsed = parse_song_rows(&later.to_string());
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.next, None);
    }

    #[test]
    fn a_playlist_header_is_read_also_inside_the_editable_header() {
        let page = json!({ "header": { "musicEditablePlaylistDetailHeaderRenderer": {
            "editHeader": {},
            "header": { "musicResponsiveHeaderRenderer": {
                "title": runs(&["My Playlist"]),
                "straplineTextOne": runs(&["Owner"]),
                "thumbnail": thumbs("https://img.invalid/h")
            } }
        } } });
        let header = parse_playlist_header(&page.to_string());
        assert_eq!(header.title.as_deref(), Some("My Playlist"));
        assert_eq!(header.author.as_deref(), Some("Owner"));
        assert_eq!(header.thumbnails.len(), 1);
        assert_eq!(parse_playlist_header("{}"), PlaylistHeader::default());
    }

    /// A browse page as playlists of every kind come back on 2026-11-11 (`PL`, `OLAK5uy`,
    /// `RDCLAK`, `RDTMAK`): two-column layout, responsive header, playlist shelf.
    fn playlist_page(rows: Vec<Value>) -> Value {
        json!({
            "contents": { "twoColumnBrowseResultsRenderer": {
                "tabs": [ { "tabRenderer": { "content": { "sectionListRenderer": { "contents": [
                    { "musicResponsiveHeaderRenderer": {
                        "title": runs(&["Curated Radio"]),
                        "straplineTextOne": runs(&["YouTube Music"]),
                        "subtitle": runs(&["Playlist", " • ", "2026"]),
                        "thumbnail": thumbs("https://img.invalid/p")
                    } }
                ] } } } } ],
                "secondaryContents": { "sectionListRenderer": { "contents": [
                    { "musicPlaylistShelfRenderer": { "contents": rows } }
                ] } }
            } }
        })
    }

    #[test]
    fn every_playlist_kind_reads_header_and_rows() {
        let page = playlist_page(vec![
            row("One", ID, "Artist One", Some("Album X"), "3:45"),
            row("Two", ID2, "Artist Two", None, "2:00"),
        ])
        .to_string();
        let header = parse_playlist_header(&page);
        assert_eq!(header.title.as_deref(), Some("Curated Radio"));
        assert_eq!(header.author.as_deref(), Some("YouTube Music"));
        assert_eq!(header.year, Some(2026));
        let rows = parse_song_rows(&page);
        assert_eq!(rows.items.len(), 2);
        assert_eq!(rows.items[1].artists, vec!["Artist Two".to_owned()]);
    }

    #[test]
    fn an_album_page_keeps_rows_without_artists() {
        let bare = json!({ "musicResponsiveListItemRenderer": {
            "flexColumns": [ { "musicResponsiveListItemFlexColumnRenderer": {
                "text": runs(&["Track"]) } } ],
            "fixedColumns": [ { "musicResponsiveListItemFixedColumnRenderer": {
                "text": runs(&["4:01"]) } } ],
            "playlistItemData": { "videoId": ID }
        } });
        let page = json!({ "contents": { "twoColumnBrowseResultsRenderer": {
            "tabs": [ { "tabRenderer": { "content": { "sectionListRenderer": { "contents": [
                { "musicResponsiveHeaderRenderer": {
                    "title": runs(&["Album"]),
                    "straplineTextOne": { "runs": [ link("The Band", "UCsynthetic3") ] },
                    "subtitle": runs(&["Album", " • ", "2019"])
                } } ] } } } } ],
            "secondaryContents": { "sectionListRenderer": { "contents": [
                { "musicShelfRenderer": { "contents": [ bare ] } } ] } }
        } } })
        .to_string();
        let rows = parse_song_rows(&page);
        assert_eq!(rows.items.len(), 1);
        assert!(rows.items[0].artists.is_empty());
        let header = parse_playlist_header(&page);
        assert_eq!(header.author.as_deref(), Some("The Band"));
        assert_eq!(header.year, Some(2019));
    }

    #[test]
    fn a_watch_playlist_lists_radio_tracks() {
        let video = |id: &str, title: &str| {
            json!({ "playlistPanelVideoRenderer": {
                "videoId": id,
                "title": runs(&[title]),
                "longBylineText": { "runs": [
                    link("Artist One", "UCsynthetic1"), { "text": " • " },
                    link("Album X", "MPREbsynthetic1"), { "text": " • " }, { "text": "2020" }
                ] },
                "lengthText": runs(&["3:21"]),
                "thumbnail": thumbs("https://img.invalid/w")
            } })
        };
        let mut gone = video(ID2, "Gone");
        gone["playlistPanelVideoRenderer"]["unplayableText"] = runs(&["Not available"]);
        let wrapped = json!({ "playlistPanelVideoWrapperRenderer": { "primaryRenderer":
            video("ccccccccccc", "Wrapped") } });
        let answer = json!({ "contents": { "singleColumnMusicWatchNextResultsRenderer": {
            "tabbedRenderer": { "watchNextTabbedResultsRenderer": { "tabs": [ { "tabRenderer": {
                "content": { "musicQueueRenderer": { "content": { "playlistPanelRenderer": {
                    "contents": [ video(ID, "First"), gone, wrapped,
                        json!({ "automixPreviewVideoRenderer": {} }) ]
                } } } }
            } } ] } }
        } } })
        .to_string();
        let tracks = parse_watch_playlist(&answer);
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].title, "First");
        assert_eq!(tracks[0].artists, vec!["Artist One".to_owned()]);
        assert_eq!(tracks[0].album.as_deref(), Some("Album X"));
        assert_eq!(tracks[0].duration, Some(Duration::from_secs(201)));
        assert_eq!(tracks[1].title, "Wrapped");
        assert!(parse_watch_playlist("{}").is_empty());
        assert!(parse_watch_playlist("not json").is_empty());
    }

    #[test]
    fn history_is_grouped_by_period() {
        let shelf = |title: &str, rows: Vec<Value>| json!({ "musicShelfRenderer": { "title": runs(&[title]), "contents": rows } });
        let page = section_list(vec![
            shelf(
                "Today",
                vec![
                    row("A", ID, "Artist", None, "1:00"),
                    row("B", ID2, "Artist", None, "2:00"),
                ],
            ),
            shelf("Empty day", vec![]),
            shelf("Yesterday", vec![row("C", ID, "Artist", None, "3:00")]),
        ]);
        let history = parse_history(&page.to_string());
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].title, "Today");
        assert_eq!(history[0].tracks.len(), 2);
        assert_eq!(history[1].tracks[0].title, "C");
    }

    #[test]
    fn explore_splits_new_releases_moods_and_the_rest() {
        let album = |title: &str, id: &str| {
            card(
                title,
                json!({ "runs": [ link("Artist", "UCsynthetic3") ] }),
                browse(id, "MUSIC_PAGE_TYPE_ALBUM"),
            )
        };
        let tile = json!({ "musicNavigationButtonRenderer": {
            "buttonText": runs(&["Focus"]),
            "clickCommand": { "browseEndpoint": {
                "browseId": "FEmusic_moods_and_genres_category", "params": "ggMPabc" } }
        } });
        let other_tile = json!({ "musicNavigationButtonRenderer": {
            "buttonText": runs(&["Charts"]),
            "clickCommand": { "browseEndpoint": { "browseId": "FEmusic_charts" } }
        } });
        let video = card(
            "Clip",
            json!({ "runs": [ link("Artist", "UCsynthetic3") ] }),
            watch(ID, "MUSIC_VIDEO_TYPE_OMV"),
        );
        let page = section_list(vec![
            carousel(
                "New albums",
                None,
                vec![
                    album("One", "MPREbsynthetic4"),
                    album("Two", "MPREbsynthetic5"),
                ],
            ),
            carousel(
                "Top songs",
                None,
                vec![row("Hit", ID, "Artist", None, "3:00")],
            ),
            carousel("Moods", None, vec![tile, other_tile]),
            carousel("Videos", None, vec![video]),
            carousel("More albums", None, vec![album("Three", "MPREbsynthetic6")]),
        ]);
        let explore = parse_explore(&page.to_string());
        assert_eq!(explore.new_releases.len(), 2);
        assert_eq!(explore.moods.len(), 1);
        assert_eq!(explore.moods[0].title, "Focus");
        let titles: Vec<&str> = explore.shelves.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["Top songs", "Videos", "More albums"]);
        assert!(explore.charts.is_empty());
        assert!(matches!(&explore.shelves[1].items[0], FeedItem::Video(_)));
    }

    #[test]
    fn a_mood_page_keeps_carousels_and_grids() {
        let grid = json!({ "gridRenderer": {
            "header": { "gridHeaderRenderer": { "title": runs(&["Featured"]) } },
            "items": [ card("Radio", json!({}), browse("VLRDCLAKsynthetic", "MUSIC_PAGE_TYPE_PLAYLIST")) ]
        } });
        let page = section_list(vec![
            carousel(
                "Picks",
                None,
                vec![card(
                    "List",
                    json!({}),
                    browse("VLPLsynthetic6", "MUSIC_PAGE_TYPE_PLAYLIST"),
                )],
            ),
            grid,
        ]);
        let shelves = parse_mood_page(&page.to_string());
        assert_eq!(shelves.len(), 2);
        assert_eq!(shelves[1].title, "Featured");
        assert!(parse_mood_page("nope").is_empty());
    }

    #[test]
    fn a_podcast_page_has_a_header_and_episodes() {
        let episode = |title: &str, id: &str| {
            json!({ "musicMultiRowListItemRenderer": {
                "title": runs(&[title]),
                "description": runs(&["About it"]),
                "subtitle": runs(&["Jan 5, 2026"]),
                "thumbnail": thumbs("https://img.invalid/e"),
                "onTap": { "watchEndpoint": { "videoId": id } }
            } })
        };
        let page = json!({ "contents": { "twoColumnBrowseResultsRenderer": {
            "tabs": [ { "tabRenderer": { "content": { "sectionListRenderer": { "contents": [
                { "musicResponsiveHeaderRenderer": {
                    "title": runs(&["The Show"]),
                    "straplineTextOne": runs(&["The Host"]),
                    "thumbnail": thumbs("https://img.invalid/s"),
                    "description": { "musicDescriptionShelfRenderer": { "description": runs(&["Show text"]) } }
                } }
            ] } } } } ],
            "secondaryContents": { "sectionListRenderer": { "contents": [
                { "musicShelfRenderer": { "contents": [ episode("Ep 1", ID), episode("Ep 2", ID2),
                    json!({ "musicMultiRowListItemRenderer": { "title": runs(&["No id"]) } }) ] } }
            ] } }
        } } });
        let id = BrowseId("MPSPsynthetic1".to_owned());
        let detail = parse_podcast(&page.to_string(), &id).expect("podcast");
        assert_eq!(detail.summary.title, "The Show");
        assert_eq!(detail.summary.author.as_deref(), Some("The Host"));
        assert_eq!(detail.summary.id, id);
        assert_eq!(detail.episodes.len(), 2);
        assert_eq!(detail.episodes[0].track.title, "Ep 1");
        assert_eq!(
            detail.episodes[0].track.artists,
            vec!["The Host".to_owned()]
        );
        assert_eq!(detail.episodes[0].published.as_deref(), Some("Jan 5, 2026"));
        assert_eq!(detail.episodes[0].description.as_deref(), Some("About it"));
        assert_eq!(parse_podcast("{}", &id), None);
    }

    #[test]
    fn continuation_tokens_are_taken_from_the_right_container() {
        let page = json!({ "a": { "nextContinuationData": { "continuation": "stray" } },
            "gridRenderer": { "continuations": [ { "nextContinuationData": { "continuation": "mine" } } ] } });
        assert_eq!(
            continuation_token(&page, &LIST_SCOPES).as_deref(),
            Some("mine")
        );
        assert_eq!(continuation_token(&page, &HOME_SCOPES), None);
        let new_style = json!({ "appendContinuationItemsAction": { "continuationItems": [
            { "continuationItemRenderer": { "continuationEndpoint": {
                "continuationCommand": { "token": "newtok" } } } } ] } });
        assert_eq!(
            continuation_token(&new_style, &LIST_SCOPES).as_deref(),
            Some("newtok")
        );
        assert_eq!(continuation_token(&json!({}), &LIST_SCOPES), None);
    }
}
