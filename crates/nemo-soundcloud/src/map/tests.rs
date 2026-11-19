//! Tests for the mapping to `nemo-core` types.

use nemo_core::audio::Origin;
use nemo_core::track::Artwork;

use super::*;
use crate::fixtures::{SEARCH_MIXED, SEARCH_TRACKS, TRACK_HLS};
use crate::map;
use crate::model::{Item, parse_item, parse_list, parse_one};

fn first_track(body: &str) -> ScTrack {
    parse_one(body.as_bytes()).expect("track fixture")
}

fn track_at(body: &str, index: usize) -> ScTrack {
    let list = parse_list(body.as_bytes()).expect("list fixture");
    match parse_item(list[index].clone()) {
        Some(Item::Track(t)) => t,
        _ => panic!("item {index} is not a track"),
    }
}

#[test]
fn a_track_keeps_its_attribution_and_its_page() {
    let t = map::track(&first_track(TRACK_HLS)).expect("track");
    assert_eq!(t.id, TrackId("sc:7000001".to_owned()));
    assert_eq!(t.title, "Fixture Track 1");
    // The publisher's artist first, then the uploader.
    assert_eq!(t.artists, ["Fixture Publisher 1", "Fixture Artist 1"]);
    assert_eq!(t.origin, Origin::SoundCloud);
    assert_eq!(
        t.duration,
        Some(Duration::from_millis(124_108)),
        "the full length, not the played one"
    );
    assert_eq!(
        t.source_url.as_ref().map(MediaUrl::as_str),
        Some("https://soundcloud.com/fixture-artist-1/fixture-track-1")
    );
    assert!(t.video_id.is_none() && t.local_path.is_none());
}

#[test]
fn non_ascii_titles_are_kept_as_they_are() {
    let t = map::track(&track_at(SEARCH_TRACKS, 1)).expect("track");
    assert_eq!(t.title, "Café Ünïcode ♪ 夜");
}

#[test]
fn an_uploader_equal_to_the_publisher_is_listed_once() {
    let mut raw = first_track(TRACK_HLS);
    raw.publisher_metadata.as_mut().expect("publisher").artist =
        Some("Fixture Artist 1".to_owned());
    let t = map::track(&raw).expect("track");
    assert_eq!(t.artists, ["Fixture Artist 1"]);
}

#[test]
fn a_stub_a_track_without_a_page_and_a_foreign_page_are_not_tracks() {
    let mut raw = first_track(TRACK_HLS);
    raw.title = None;
    assert!(map::track(&raw).is_none(), "a stub has no title");
    let mut raw = first_track(TRACK_HLS);
    raw.permalink_url = None;
    assert!(map::track(&raw).is_none());
    let mut raw = first_track(TRACK_HLS);
    raw.permalink_url = Some("https://evil.example/a".to_owned());
    assert!(
        map::track(&raw).is_none(),
        "the page must be on a SoundCloud host"
    );
    let mut raw = first_track(TRACK_HLS);
    raw.permalink_url = Some("http://soundcloud.com/a/b".to_owned());
    assert!(map::track(&raw).is_none(), "https only");
}

#[test]
fn a_track_without_artwork_uses_the_uploaders_picture() {
    let mut raw = first_track(TRACK_HLS);
    raw.artwork_url = None;
    let Artwork::Remote(thumbs) = map::track(&raw).expect("track").artwork else {
        panic!("expected the avatar");
    };
    assert!(thumbs[0].url.contains("avatars-"));
    raw.user = None;
    assert_eq!(map::track(&raw).expect("track").artwork, Artwork::None);
}

#[test]
fn artwork_comes_in_three_sizes_from_the_large_name() {
    let thumbs = map::thumbnails(Some("https://i1.sndcdn.com/artworks-abc-large.png"));
    let sizes: Vec<(u32, &str)> = thumbs.iter().map(|t| (t.width, t.url.as_str())).collect();
    assert_eq!(
        sizes,
        [
            (100, "https://i1.sndcdn.com/artworks-abc-large.png"),
            (300, "https://i1.sndcdn.com/artworks-abc-t300x300.png"),
            (500, "https://i1.sndcdn.com/artworks-abc-t500x500.png"),
        ]
    );
}

#[test]
fn artwork_of_another_shape_or_host_is_handled_safely() {
    let one = map::thumbnails(Some("https://i1.sndcdn.com/artworks-abc-crop.jpg"));
    assert_eq!(
        (one.len(), one[0].width),
        (1, 0),
        "an unknown name stays as it is, size unknown"
    );
    assert!(map::thumbnails(Some("https://evil.example/a-large.png")).is_empty());
    assert!(map::thumbnails(Some("http://i1.sndcdn.com/a-large.png")).is_empty());
    assert!(map::thumbnails(Some("not a url")).is_empty());
    assert!(map::thumbnails(None).is_empty());
}

#[test]
fn page_ids_round_trip_and_reject_anything_but_digits() {
    let id = map::browse_id(Kind::Playlist, 42);
    assert_eq!(id.0, "sc:playlist:42");
    assert_eq!(map::parse_browse_id(&id), Some((Kind::Playlist, 42)));
    for bad in [
        "",
        "sc",
        "sc:user",
        "sc:user:",
        "sc:user:1a",
        "sc:user:1:2",
        "yt:user:1",
        "sc:album:1",
        "sc:user:../x",
        "sc:user:-1",
    ] {
        assert_eq!(
            map::parse_browse_id(&BrowseId(bad.to_owned())),
            None,
            "{bad}"
        );
    }
    assert_eq!(parse_track_id(&TrackId("sc:7".to_owned())), Some(7));
    assert_eq!(parse_track_id(&TrackId("7".to_owned())), None);
    assert_eq!(parse_track_id(&TrackId("sc:7/../8".to_owned())), None);
}

#[test]
fn albums_and_playlists_read_from_the_mixed_list() {
    let list = parse_list(SEARCH_MIXED.as_bytes()).expect("list");
    let mut playlists = Vec::new();
    for raw in list {
        if let Some(Item::Playlist(p)) = parse_item(raw) {
            playlists.push(p);
        }
    }
    assert_eq!(playlists.len(), 2);
    let album = playlists.iter().find(|p| p.is_album).expect("an album");
    let summary = map::album(album, None).expect("album");
    assert!(summary.title.starts_with("Fixture Playlist"));
    assert_eq!(summary.artists.len(), 1);
    assert!(!summary.thumbnails.is_empty());
    let meta = map::collection_meta(album);
    assert_eq!(meta.track_count, album.track_count);
    assert!(meta.owner.is_some());
}

#[test]
fn a_user_without_a_name_is_not_listed() {
    let user = ScUser {
        id: 1,
        username: None,
        avatar_url: None,
        description: None,
    };
    assert!(map::artist(&user).is_none());
    let user = ScUser {
        id: 2,
        username: Some("Fixture Artist 9".to_owned()),
        avatar_url: None,
        description: None,
    };
    let summary = map::artist(&user).expect("artist");
    assert_eq!(summary.id, map::browse_id(Kind::User, 2));
    assert!(summary.thumbnails.is_empty());
}
