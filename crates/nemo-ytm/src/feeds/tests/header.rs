//! The header facts of real album and playlist pages, anonymous answers of 2026-11-15
//! (`tests/fixtures/music/README.md`), and the edit header of one's own playlist.

use nemo_core::catalog::Privacy;
use serde_json::json;

use super::super::{parse_playlist_header, parse_song_rows};
use super::fixtures::runs;

const PLAYLIST: &str = include_str!("../../../tests/fixtures/music/browse-playlist.json");
const ALBUM: &str = include_str!("../../../tests/fixtures/music/browse-album.json");

#[test]
fn a_community_playlist_header_gives_owner_year_views_count_and_length() {
    let h = parse_playlist_header(PLAYLIST);
    assert_eq!(h.title.as_deref(), Some("MCK - HVL: Full Album Tracklist"));
    // The facepile owner (its name and picture were replaced by synthetic values).
    assert_eq!(h.author.as_deref(), Some("Playlist Owner"));
    let m = &h.meta;
    assert_eq!(m.owner.as_deref(), Some("Playlist Owner"));
    assert_eq!(m.owner_avatar.len(), 1);
    assert!(m.owner_avatar[0].url.starts_with("https://yt3.ggpht.com/"));
    assert_eq!(m.year, Some(2026));
    assert_eq!(h.year, Some(2026));
    assert_eq!(m.view_count, Some(437_000));
    assert_eq!(m.track_count, Some(30));
    assert_eq!(m.duration_text.as_deref(), Some("36 minutes"));
    // An anonymous visitor is not told the privacy.
    assert_eq!(m.privacy, None);
    assert!(!parse_song_rows(PLAYLIST).items.is_empty());
}

#[test]
fn an_album_header_gives_the_artist_picture_count_and_length_but_no_views() {
    let h = parse_playlist_header(ALBUM);
    let m = &h.meta;
    assert!(h.title.is_some());
    assert!(m.owner.is_some(), "the strapline artist");
    assert_eq!(h.author, m.owner);
    assert!(m.owner_avatar.len() >= 2, "{:?}", m.owner_avatar);
    assert_eq!(m.year, Some(2026));
    assert_eq!(m.track_count, Some(8));
    assert_eq!(m.duration_text.as_deref(), Some("28 minutes"));
    assert_eq!(m.view_count, None);
    assert_eq!(m.privacy, None);
}

#[test]
fn ones_own_playlist_shows_its_privacy_from_the_subtitle_or_the_edit_header() {
    let in_subtitle = json!({ "header": { "musicResponsiveHeaderRenderer": {
        "title": runs(&["Mine"]),
        "subtitle": runs(&["Playlist", " • ", "Public", " • ", "2026"]),
        "secondSubtitle": runs(&["411 views", " • ", "249 tracks", " • ", "13+ hours"])
    } } });
    let m = parse_playlist_header(&in_subtitle.to_string()).meta;
    assert_eq!(m.privacy, Some(Privacy::Public));
    assert_eq!(m.year, Some(2026));
    assert_eq!(m.view_count, Some(411));
    assert_eq!(m.track_count, Some(249));
    assert_eq!(m.duration_text.as_deref(), Some("13+ hours"));

    let edit = json!({ "header": { "musicEditablePlaylistDetailHeaderRenderer": {
        "editHeader": { "musicPlaylistEditHeaderRenderer": { "privacy": "UNLISTED" } },
        "header": { "musicResponsiveHeaderRenderer": {
            "title": runs(&["Mine"]),
            "subtitle": runs(&["Playlist", " • ", "2025"])
        } }
    } } });
    let m = parse_playlist_header(&edit.to_string()).meta;
    assert_eq!(m.privacy, Some(Privacy::Unlisted));
    assert_eq!(m.view_count, None, "nothing is guessed");
    assert_eq!(m.track_count, None);
    assert_eq!(m.duration_text, None);
}
