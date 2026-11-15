//! Read the unfiltered search answer.
//!
//! Layout (checked on 2026-11-15 with an anonymous request for "HVL"): under
//! `tabbedSearchResultsRenderer` the first tab holds a `sectionListRenderer`. Its first section
//! is the top-result card (`musicCardShelfRenderer`); then each result comes in its own
//! `itemSectionRenderer` as one `musicResponsiveListItemRenderer`, songs, artists, videos,
//! playlists and albums mixed in YouTube's order. The older layout put the results in titled
//! `musicShelfRenderer` shelves, and the card could hold more rows; both are read too.
//!
//! Every entry is classified by its navigation endpoint (page type, id prefix, music video
//! type), never by its text, because the texts follow the account language. Podcasts,
//! episodes and user profiles have no place in `SearchResults` yet and are skipped.

use nemo_core::catalog::{
    AlbumSummary, ArtistSummary, PlaylistSummary, SearchKind, SearchResults, TopResult,
    VideoSummary,
};
use nemo_core::track::{Thumbnail, Track};
use serde_json::Value;

use crate::catalog::{
    RawAlbum, RawArtist, RawPlaylist, RawSong, map_album, map_artist, map_playlist, map_song,
    parse_duration, parse_year,
};
use crate::feeds::{find_key, text_of, thumbnails_of};

const SECTIONS: &str =
    "/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents";
const GREY_OUT: &str = "MUSIC_ITEM_RENDERER_DISPLAY_POLICY_GREY_OUT";
/// Most results kept per kind.
const MAX_PER_KIND: usize = 50;

/// One result of any kind.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Item {
    Song(Track),
    Video(VideoSummary),
    Album(AlbumSummary),
    Artist(ArtistSummary),
    Playlist(PlaylistSummary),
}

/// The texts after the title: linked artists, the linked album, and unlinked texts.
#[derive(Debug, Default)]
struct Sub {
    artists: Vec<String>,
    album: Option<String>,
    plain: Vec<String>,
}

fn is_separator(text: &str) -> bool {
    matches!(text, "•" | "·" | "&" | "," | "-" | "–" | "|")
}

impl Sub {
    /// Add the runs of one column. `labelled`: the first run is the result type ("Song",
    /// "Album", or the same word in the account language), which is dropped.
    fn add(&mut self, text: Option<&Value>, labelled: bool) {
        let runs = text
            .and_then(|t| t.get("runs"))
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        let mut skip_label = labelled;
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
            if std::mem::take(&mut skip_label) && id.is_none() {
                continue;
            }
            match id {
                Some(id) if id.starts_with("UC") => self.artists.push(text.to_owned()),
                Some(id) if id.starts_with("MPRE") => self.album = Some(text.to_owned()),
                Some(_) => {}
                None => self.plain.push(text.to_owned()),
            }
        }
    }

    /// Linked artists, else the unlinked texts that look like names (no digits).
    fn artist_names(&self) -> Vec<String> {
        if !self.artists.is_empty() {
            return self.artists.clone();
        }
        self.plain
            .iter()
            .filter(|t| !t.chars().any(|c| c.is_ascii_digit()))
            .cloned()
            .collect()
    }

    fn year(&self) -> Option<String> {
        self.plain.iter().find(|t| parse_year(t).is_some()).cloned()
    }

    fn duration(&self) -> Option<String> {
        self.plain
            .iter()
            .find(|t| parse_duration(t).is_some())
            .cloned()
    }

    /// A count such as "861K views": the last text with a digit that is no duration or year.
    fn count_text(&self) -> Option<String> {
        self.plain
            .iter()
            .rev()
            .find(|t| {
                t.chars().any(|c| c.is_ascii_digit())
                    && parse_duration(t).is_none()
                    && parse_year(t).is_none()
            })
            .cloned()
    }
}

fn page_type(browse: &Value) -> &str {
    browse
        .pointer("/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType")
        .and_then(Value::as_str)
        .unwrap_or("")
}

/// An album, artist or playlist entry, from its browse endpoint.
fn browse_item(browse: &Value, title: String, sub: &Sub, thumbs: Vec<Thumbnail>) -> Option<Item> {
    let id = browse.get("browseId")?.as_str()?;
    let kind = page_type(browse);
    if kind.contains("PODCAST") || kind.contains("USER_CHANNEL") || id.starts_with("MPSP") {
        return None;
    }
    if kind.contains("ALBUM") || id.starts_with("MPRE") {
        let raw = RawAlbum {
            id: id.to_owned(),
            title,
            artists: sub.artist_names(),
            year: sub.year(),
            thumbnails: thumbs,
        };
        return map_album(&raw).map(Item::Album);
    }
    if kind.contains("PLAYLIST") || id.starts_with("VL") {
        let raw = RawPlaylist {
            id: id.to_owned(),
            title,
            author: sub.artist_names().into_iter().next(),
            thumbnails: thumbs,
        };
        return map_playlist(&raw).map(Item::Playlist);
    }
    if kind.contains("ARTIST") || id.starts_with("UC") {
        let raw = RawArtist {
            id: id.to_owned(),
            name: title,
            thumbnails: thumbs,
        };
        return map_artist(&raw).map(Item::Artist);
    }
    None
}

/// A song or a music video. Podcast episodes are skipped.
fn media_item(
    video_id: &str,
    video_type: Option<&str>,
    title: String,
    sub: &Sub,
    thumbs: Vec<Thumbnail>,
) -> Option<Item> {
    let video_type = video_type.unwrap_or("");
    if video_type.contains("PODCAST_EPISODE") {
        return None;
    }
    let track = map_song(&RawSong {
        video_id: video_id.to_owned(),
        title,
        artists: sub.artist_names(),
        album: sub.album.clone(),
        duration: sub.duration(),
        thumbnails: thumbs,
    })?;
    let video = ["OMV", "UGC", "OFFICIAL_SOURCE"]
        .iter()
        .any(|t| video_type.contains(t));
    Some(if video {
        Item::Video(VideoSummary {
            track,
            channel: None,
            live: false,
            views: sub.count_text(),
        })
    } else {
        Item::Song(track)
    })
}

fn flex_text(row: &Value, index: usize) -> Option<&Value> {
    row.get("flexColumns")?
        .as_array()?
        .get(index)?
        .get("musicResponsiveListItemFlexColumnRenderer")?
        .get("text")
}

/// One result row (`musicResponsiveListItemRenderer`).
fn row_item(row: &Value, labelled: bool) -> Option<Item> {
    if row
        .get("musicItemRendererDisplayPolicy")
        .and_then(Value::as_str)
        == Some(GREY_OUT)
    {
        return None;
    }
    let title = flex_text(row, 0).and_then(text_of)?;
    let columns = row
        .get("flexColumns")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let mut sub = Sub::default();
    for index in 1..columns {
        sub.add(flex_text(row, index), labelled && index == 1);
    }
    if let Some(fixed) = row
        .pointer("/fixedColumns/0/musicResponsiveListItemFixedColumnRenderer/text")
        .and_then(text_of)
    {
        sub.plain.push(fixed);
    }
    let thumbs = row.get("thumbnail").map(thumbnails_of).unwrap_or_default();
    if let Some(browse) = row.pointer("/navigationEndpoint/browseEndpoint") {
        return browse_item(browse, title, &sub, thumbs);
    }
    let video_id = row
        .pointer("/playlistItemData/videoId")
        .or_else(|| find_key(row, "videoId"))?
        .as_str()?;
    let video_type = find_key(row, "musicVideoType").and_then(Value::as_str);
    media_item(video_id, video_type, title, &sub, thumbs)
}

/// The top-result card itself (`musicCardShelfRenderer`).
fn card_item(card: &Value) -> Option<Item> {
    let title_value = card.get("title")?;
    let title = text_of(title_value)?;
    let mut sub = Sub::default();
    sub.add(card.get("subtitle"), true);
    let thumbs = card.get("thumbnail").map(thumbnails_of).unwrap_or_default();
    let nav = title_value
        .pointer("/runs/0/navigationEndpoint")
        .or_else(|| card.get("onTap"))?;
    if let Some(browse) = nav.get("browseEndpoint") {
        return browse_item(browse, title, &sub, thumbs);
    }
    let watch = nav.get("watchEndpoint")?;
    let video_id = watch.get("videoId")?.as_str()?;
    let video_type = find_key(watch, "musicVideoType").and_then(Value::as_str);
    media_item(video_id, video_type, title, &sub, thumbs)
}

/// The answer being built: lists, the order of first appearance, the top result.
#[derive(Default)]
struct Acc {
    out: SearchResults,
}

impl Acc {
    fn has(&self, item: &Item) -> bool {
        let r = &self.out;
        if r.top.as_ref().is_some_and(|t| is_top(t, item)) {
            return true;
        }
        match item {
            Item::Song(t) => r.songs.iter().any(|s| s.id == t.id),
            Item::Video(v) => r.videos.iter().any(|s| s.track.id == v.track.id),
            Item::Album(a) => r.albums.iter().any(|s| s.id == a.id),
            Item::Artist(a) => r.artists.iter().any(|s| s.id == a.id),
            Item::Playlist(p) => r.playlists.iter().any(|s| s.id == p.id),
        }
    }

    fn push(&mut self, item: Item) {
        if self.has(&item) {
            return;
        }
        let kind = kind_of(&item);
        if self.out.count(kind) >= MAX_PER_KIND {
            return;
        }
        if !self.out.order.contains(&kind) {
            self.out.order.push(kind);
        }
        let r = &mut self.out;
        match item {
            Item::Song(t) => r.songs.push(t),
            Item::Video(v) => r.videos.push(v),
            Item::Album(a) => r.albums.push(a),
            Item::Artist(a) => r.artists.push(a),
            Item::Playlist(p) => r.playlists.push(p),
        }
    }

    fn rows(&mut self, entries: Option<&Value>, labelled: bool) {
        let entries = entries
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        for entry in entries {
            if let Some(item) = entry
                .get("musicResponsiveListItemRenderer")
                .and_then(|row| row_item(row, labelled))
            {
                self.push(item);
            }
        }
    }
}

impl From<Item> for TopResult {
    fn from(item: Item) -> Self {
        match item {
            Item::Song(t) => Self::Song(t),
            Item::Video(v) => Self::Video(v),
            Item::Album(a) => Self::Album(a),
            Item::Artist(a) => Self::Artist(a),
            Item::Playlist(p) => Self::Playlist(p),
        }
    }
}

fn kind_of(item: &Item) -> SearchKind {
    match item {
        Item::Song(_) => SearchKind::Songs,
        Item::Video(_) => SearchKind::Videos,
        Item::Album(_) => SearchKind::Albums,
        Item::Artist(_) => SearchKind::Artists,
        Item::Playlist(_) => SearchKind::Playlists,
    }
}

/// `true` when `item` is the top result again.
fn is_top(top: &TopResult, item: &Item) -> bool {
    match (top, item) {
        (TopResult::Song(x), Item::Song(y)) => x.id == y.id,
        (TopResult::Video(x), Item::Video(y)) => x.track.id == y.track.id,
        (TopResult::Album(x), Item::Album(y)) => x.id == y.id,
        (TopResult::Artist(x), Item::Artist(y)) => x.id == y.id,
        (TopResult::Playlist(x), Item::Playlist(y)) => x.id == y.id,
        _ => false,
    }
}

/// Read an unfiltered search answer. `None` when it is not a search answer this reader knows
/// (no section list); an answer with no results gives empty [`SearchResults`].
pub(crate) fn parse_search(json: &str) -> Option<SearchResults> {
    let root: Value = serde_json::from_str(json).ok()?;
    let sections = root.pointer(SECTIONS)?.as_array()?;
    let mut acc = Acc::default();
    for section in sections {
        if let Some(card) = section.get("musicCardShelfRenderer") {
            if acc.out.top.is_none()
                && let Some(item) = card_item(card)
            {
                acc.out.top = Some(item.into());
            }
            acc.rows(card.get("contents"), true);
        } else if let Some(shelf) = section.get("musicShelfRenderer") {
            // Titled shelves name the kind; their rows carry no type label.
            acc.rows(shelf.get("contents"), false);
        } else if let Some(items) = section.get("itemSectionRenderer") {
            acc.rows(items.get("contents"), true);
        }
    }
    Some(acc.out)
}
