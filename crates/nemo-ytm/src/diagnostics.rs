//! Development aids: describe an answer's shape without showing any of its content.
//!
//! Output holds key names and counts only. Never titles, ids, names or URLs.

use std::collections::BTreeMap;
use std::time::Duration;

use nemo_core::catalog::{CatalogError, FeedItem, FeedShelf};
use nemo_core::track::VideoId;
use serde_json::Value;
use ytmapi_rs::YtMusic;
use ytmapi_rs::auth::AuthToken;

use crate::pages;

/// Count every JSON key that ends in `Renderer` or `ViewModel`, at any depth.
fn count_renderers(value: &Value, counts: &mut BTreeMap<String, usize>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                if key.ends_with("Renderer") || key.ends_with("ViewModel") {
                    *counts.entry(key.clone()).or_default() += 1;
                }
                count_renderers(inner, counts);
            }
        }
        Value::Array(items) => {
            for item in items {
                count_renderers(item, counts);
            }
        }
        _ => {}
    }
}

/// A one-line shape summary of a JSON answer: size, top-level keys, an error code if the answer
/// is an error, and how many of each renderer kind it holds.
#[must_use]
pub fn shape_summary(json: &str) -> String {
    let Ok(root) = serde_json::from_str::<Value>(json) else {
        return format!("bytes={} not-json", json.len());
    };
    let top: Vec<&str> = root
        .as_object()
        .map(|m| m.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let error = root
        .pointer("/error/code")
        .and_then(Value::as_u64)
        .map_or_else(String::new, |c| format!(" error-code={c}"));
    let mut counts = BTreeMap::new();
    count_renderers(&root, &mut counts);
    let kinds: Vec<String> = counts.iter().map(|(k, n)| format!("{k}:{n}")).collect();
    format!(
        "bytes={} top=[{}]{error} renderers=[{}]",
        json.len(),
        top.join(","),
        kinds.join(",")
    )
}

/// What a diagnostic run found: lines for the report, and one video to try the stream resolver on.
#[derive(Debug, Clone, Default)]
pub struct Diagnosis {
    /// Counts and error labels, one per line. No titles, ids or URLs.
    pub lines: Vec<String>,
    /// A music video (not a song) seen on the home or explore page, if any.
    pub sample_video: Option<VideoId>,
}

impl Diagnosis {
    /// A report that only says why nothing could be checked.
    #[must_use]
    pub fn failed(reason: &str) -> Self {
        Self {
            lines: vec![format!("diagnose: {reason}")],
            sample_video: None,
        }
    }
}

fn line<T>(name: &str, result: &Result<T, CatalogError>, count: impl Fn(&T) -> String) -> String {
    match result {
        Ok(value) => format!("{name}: {}", count(value)),
        Err(e) => format!("{name}: error ({e})"),
    }
}

fn shelf_counts(shelves: &[FeedShelf]) -> String {
    let items: usize = shelves.iter().map(|s| s.items.len()).sum();
    format!("{} shelves, {items} items", shelves.len())
}

fn first_video(shelves: &[FeedShelf]) -> Option<VideoId> {
    shelves
        .iter()
        .flat_map(|s| &s.items)
        .find_map(|item| match item {
            FeedItem::Video(track) => track.video_id.clone(),
            _ => None,
        })
}

/// Ask the account and discovery pages the way the app does and describe what came back.
/// Makes about twenty requests, with the pagination pauses of the app and one second between
/// the steps. Development only.
#[allow(clippy::too_many_lines, reason = "a linear development report")]
pub(crate) async fn account_report<A: AuthToken>(api: &YtMusic<A>) -> Diagnosis {
    let pause = || tokio::time::sleep(Duration::from_secs(1));
    let mut out: Vec<String> = Vec::new();
    let mut sample_video = None;

    let home = pages::home_feed(api).await;
    out.push(line("home_feed", &home, |h| {
        let kinds: Vec<String> = h
            .shelves
            .iter()
            .map(|s| format!("{:?}:{}", s.kind, s.items.len()))
            .collect();
        format!(
            "{} chips, {} [{}]",
            h.chips.len(),
            shelf_counts(&h.shelves),
            kinds.join(" ")
        )
    }));
    if let Ok(h) = &home {
        let mut counts = [0_usize; 8];
        for item in h.shelves.iter().flat_map(|s| &s.items) {
            let slot = match item {
                FeedItem::Song(_) => 0,
                FeedItem::Video(_) => 1,
                FeedItem::Album(_) => 2,
                FeedItem::Artist(_) => 3,
                FeedItem::Playlist(_) => 4,
                FeedItem::Podcast(_) => 5,
                FeedItem::Episode(_) => 6,
                FeedItem::Mood(_) => 7,
            };
            counts[slot] += 1;
        }
        out.push(format!(
            "  home items: songs {}, videos {}, albums {}, artists {}, playlists {}, podcasts {}, episodes {}",
            counts[0], counts[1], counts[2], counts[3], counts[4], counts[5], counts[6]
        ));
        sample_video = first_video(&h.shelves);
    }

    pause().await;
    let lists = pages::library_playlists(api, pages::LIST_PAGES).await;
    out.push(line("library_playlists", &lists, |l| l.len().to_string()));
    pause().await;
    let liked = pages::liked_songs(api, pages::PLAYLIST_PAGES).await;
    out.push(line("liked_songs", &liked, |l| l.len().to_string()));
    pause().await;
    let albums = pages::library_albums(api).await;
    out.push(line("library_albums", &albums, |l| l.len().to_string()));
    pause().await;
    let artists = pages::library_artists(api).await;
    out.push(line("library_artists", &artists, |l| l.len().to_string()));
    pause().await;
    let history = pages::history(api).await;
    out.push(line("history", &history, |h| {
        let tracks: usize = h.iter().map(|s| s.tracks.len()).sum();
        format!("{} sections, {tracks} tracks", h.len())
    }));
    pause().await;
    let explore = pages::explore(api).await;
    out.push(line("explore", &explore, |e| {
        format!(
            "{} new releases, {} chart shelves, {} other shelves, {} moods",
            e.new_releases.len(),
            e.charts.len(),
            e.shelves.len(),
            e.moods.len()
        )
    }));
    if let Ok(e) = &explore {
        if sample_video.is_none() {
            sample_video = first_video(&e.shelves);
        }
        if let Some(mood) = e.moods.first() {
            pause().await;
            let shelves = pages::mood_playlists(api, &mood.id).await;
            out.push(line("mood_playlists (explore tile)", &shelves, |s| {
                shelf_counts(s)
            }));
        }
    }
    if let Ok(h) = &home {
        if let Some(chip) = h.chips.first() {
            pause().await;
            let shelves = pages::mood_playlists(api, &chip.id).await;
            out.push(line("mood_playlists (home chip)", &shelves, |s| {
                shelf_counts(s)
            }));
        }
        let show = h
            .shelves
            .iter()
            .flat_map(|s| &s.items)
            .find_map(|i| match i {
                FeedItem::Podcast(p) => Some(p.id.clone()),
                _ => None,
            });
        if let Some(show) = show {
            pause().await;
            let detail = pages::podcast(api, &show).await;
            out.push(line("podcast", &detail, |d| {
                format!("{} episodes", d.episodes.len())
            }));
        }
    }
    if let Ok(l) = &lists
        && let Some(own) = l.iter().find(|p| p.id.0 != "VLLM")
    {
        pause().await;
        let detail = pages::playlist(api, &own.id).await;
        out.push(line("playlist (own, first)", &detail, |d| {
            format!("{} tracks", d.tracks.len())
        }));
    }
    Diagnosis {
        lines: out,
        sample_video,
    }
}

/// Keys whose subtree is noise for layout work (menus, tracking, accessibility text).
const SKIPPED: [&str; 12] = [
    "menu",
    "accessibility",
    "accessibilityData",
    "trackingParams",
    "clickTrackingParams",
    "loggingDirectives",
    "thumbnailOverlay",
    "frameworkUpdates",
    "responseContext",
    "background",
    "overlay",
    "buttonRenderer",
];
/// Keys whose string value is a structural id: only the first four characters are kept.
const ID_KEYS: [&str; 5] = ["browseId", "playlistId", "videoId", "params", "token"];
/// Keys whose string value is a fixed layout name and is shown whole.
const NAME_KEYS: [&str; 3] = ["pageType", "style", "musicVideoType"];

/// A compact outline of `value` down to `depth` levels: key names, array sizes, value kinds,
/// four-character id prefixes and layout names. Every other string is shown as its length only,
/// so titles, artists and URLs never appear.
#[must_use]
pub fn skeleton(value: &Value, depth: usize) -> String {
    outline("", value, depth)
}

fn outline(key: &str, value: &Value, depth: usize) -> String {
    match value {
        Value::Object(map) => {
            if depth == 0 {
                return "{..}".to_owned();
            }
            let parts: Vec<String> = map
                .iter()
                .filter(|(k, _)| !SKIPPED.contains(&k.as_str()))
                .map(|(k, v)| format!("{k}:{}", outline(k, v, depth - 1)))
                .collect();
            format!("{{{}}}", parts.join(" "))
        }
        Value::Array(items) => {
            if depth == 0 {
                return format!("[{}]", items.len());
            }
            let shown: Vec<String> = items
                .iter()
                .take(2)
                .map(|v| outline(key, v, depth - 1))
                .collect();
            format!("[{}: {}]", items.len(), shown.join(", "))
        }
        Value::String(text) if NAME_KEYS.contains(&key) => format!("'{text}'"),
        Value::String(text) if ID_KEYS.contains(&key) => {
            let head: String = text.chars().take(4).collect();
            format!("'{head}…'({})", text.chars().count())
        }
        Value::String(text) => format!("s{}", text.chars().count()),
        Value::Number(_) => "n".to_owned(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_owned(),
    }
}

/// Outlines of the first `limit` values stored under any key named `key`, at any depth.
#[must_use]
pub fn outlines_of(root: &Value, key: &str, depth: usize, limit: usize) -> Vec<String> {
    fn walk<'a>(value: &'a Value, key: &str, found: &mut Vec<&'a Value>, limit: usize) {
        if found.len() >= limit {
            return;
        }
        match value {
            Value::Object(map) => {
                for (k, v) in map {
                    if k == key {
                        found.push(v);
                    } else {
                        walk(v, key, found, limit);
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, key, found, limit);
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    walk(root, key, &mut found, limit);
    found.into_iter().map(|v| skeleton(v, depth)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn skeleton_hides_text_but_keeps_layout() {
        let v = json!({
            "title": { "runs": [ { "text": "SECRET TITLE" } ] },
            "navigationEndpoint": { "browseEndpoint": { "browseId": "MPREb_secretid",
                "browseEndpointContextSupportedConfigs": { "browseEndpointContextMusicConfig":
                    { "pageType": "MUSIC_PAGE_TYPE_ALBUM" } } } },
            "menu": { "x": "hidden" }
        });
        let line = skeleton(&v, 9);
        assert!(!line.contains("SECRET"), "{line}");
        assert!(!line.contains("secretid"), "{line}");
        assert!(!line.contains("hidden"), "{line}");
        assert!(line.contains("MPRE…"), "{line}");
        assert!(line.contains("MUSIC_PAGE_TYPE_ALBUM"), "{line}");
        assert_eq!(outlines_of(&v, "browseId", 3, 5).len(), 1);
    }

    #[test]
    fn shape_has_counts_and_no_text() {
        let answer = json!({
            "contents": { "a": { "musicTwoRowItemRenderer": { "title": "SECRET" } },
                          "b": [ { "musicTwoRowItemRenderer": {} } ] },
            "responseContext": {}
        })
        .to_string();
        let line = shape_summary(&answer);
        assert!(line.contains("musicTwoRowItemRenderer:2"), "{line}");
        assert!(line.contains("top=[contents,responseContext]"), "{line}");
        assert!(!line.contains("SECRET"));
    }

    #[test]
    fn errors_and_garbage_are_described() {
        let line = shape_summary(&json!({ "error": { "code": 401 } }).to_string());
        assert!(line.contains("error-code=401"), "{line}");
        assert!(shape_summary("nope").contains("not-json"));
    }
}
