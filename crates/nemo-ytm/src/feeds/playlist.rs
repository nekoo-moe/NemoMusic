//! Playlist headers, watch playlists, history and podcasts.

use nemo_core::catalog::{BrowseId, EpisodeSummary, HistorySection, PodcastDetail, PodcastSummary};
use nemo_core::track::{Thumbnail, Track};
use serde_json::Value;

use super::MAX_WATCH_TRACKS;
use super::item::{episode_item, song_row};
use super::json::{find_key, item_thumbnails, joined_text, text_of, thumbnails_of};
use super::runs::Runs;
use super::shelf::{parse_root, section_contents};
use crate::catalog::{RawSong, map_song, parse_year};

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
