//! Tests for the catalog over a fake HTTP layer and recorded, rewritten fixtures.

use std::sync::Arc;
use std::time::Duration;

use nemo_core::audio::Origin;
use nemo_core::catalog::SearchKind;
use nemo_core::track::Artwork;

use super::*;
use crate::client::{Endpoints, Pacing};
use crate::fake::{FakeHttp, ok, status};
use crate::fixtures::{
    PLAYLIST_WITH_STUBS, RELATED, SEARCH_MIXED, SEARCH_TRACKS, SEARCH_USERS, TRACK_HLS,
    TRACKS_BATCH, USER, USER_ALBUMS, USER_TRACKS,
};
use crate::http::{ScAnswer, ScHttpError};

const ID: &str = "AbCdEfGh0123456789AbCdEfGh012345";

fn fast() -> Pacing {
    Pacing {
        min_gap: Duration::ZERO,
        retry_delays: [Duration::ZERO, Duration::ZERO],
        default_backoff: Duration::from_millis(20),
        max_backoff: Duration::from_millis(20),
    }
}

/// A catalog whose API answers come from `api(path_and_query)`; the home page and script are
/// served too, so the client id is found as in real life.
fn catalog_with<F>(api: F) -> (SoundcloudCatalog, Arc<FakeHttp>)
where
    F: Fn(&str) -> Result<ScAnswer, ScHttpError> + Send + Sync + 'static,
{
    let fake = Arc::new(FakeHttp::new(move |url, _| {
        if url == "https://soundcloud.com/" {
            ok(r#"<script src="https://a-v2.sndcdn.com/assets/1-a.js"></script>"#)
        } else if url.starts_with("https://a-v2.sndcdn.com/") {
            ok(format!("client_id:\"{ID}\""))
        } else {
            api(url.trim_start_matches("https://api-v2.soundcloud.com"))
        }
    }));
    let client = ScClient::with(
        Arc::clone(&fake) as Arc<dyn crate::http::ScHttp>,
        Endpoints::production(),
        fast(),
    );
    (SoundcloudCatalog::new(Arc::new(client)), fake)
}

fn api_urls(fake: &FakeHttp) -> Vec<String> {
    fake.urls()
        .into_iter()
        .filter(|u| u.starts_with("https://api-v2.soundcloud.com"))
        .collect()
}

#[tokio::test]
async fn one_search_is_one_request_and_groups_the_results() {
    let (catalog, fake) = catalog_with(|_| ok(SEARCH_MIXED));
    let results = catalog.search("  ambient piano ").await.expect("results");
    assert_eq!((results.songs.len(), results.artists.len()), (2, 1));
    assert_eq!(
        (results.albums.len(), results.playlists.len()),
        (1, 1),
        "albums apart from playlists"
    );
    let urls = api_urls(&fake);
    assert_eq!(urls.len(), 1);
    assert!(
        urls[0].starts_with(
            "https://api-v2.soundcloud.com/search?q=ambient+piano&limit=20&client_id="
        )
    );
}

#[tokio::test]
async fn an_empty_query_makes_no_request() {
    let (catalog, fake) = catalog_with(|_| ok(SEARCH_MIXED));
    assert_eq!(catalog.search("   ").await, Ok(SearchResults::default()));
    assert!(
        fake.urls().is_empty(),
        "not even the client id is looked up"
    );
}

#[tokio::test]
async fn a_long_query_is_cut_and_encoded() {
    let (catalog, fake) = catalog_with(|_| ok(SEARCH_MIXED));
    let query = format!("&client_id=evil {}", "x".repeat(300));
    catalog.search(&query).await.expect("results");
    let url = api_urls(&fake).remove(0);
    assert!(url.contains("q=%26client_id%3Devil+xxxx"), "{}", &url[..80]);
    assert!(
        url.len() < 400,
        "at most 200 characters of the query are sent"
    );
    assert_eq!(
        url.matches("client_id=").count(),
        1,
        "the typed text cannot add a second id"
    );
}

#[tokio::test]
async fn each_filter_uses_its_own_path_and_videos_are_not_offered() {
    let (catalog, fake) = catalog_with(|path| {
        if path.starts_with("/search/users") {
            ok(SEARCH_USERS)
        } else {
            ok(SEARCH_TRACKS)
        }
    });
    let songs = catalog
        .search_kind("a", SearchKind::Songs)
        .await
        .expect("songs");
    assert_eq!(songs.songs.len(), 5);
    let artists = catalog
        .search_kind("a", SearchKind::Artists)
        .await
        .expect("artists");
    assert_eq!(artists.artists.len(), 1);
    catalog
        .search_kind("a", SearchKind::Albums)
        .await
        .expect("albums");
    catalog
        .search_kind("a", SearchKind::Playlists)
        .await
        .expect("playlists");
    let paths: Vec<String> = api_urls(&fake)
        .iter()
        .map(|u| {
            u.trim_start_matches("https://api-v2.soundcloud.com")
                .split('?')
                .next()
                .unwrap_or("")
                .to_owned()
        })
        .collect();
    assert_eq!(
        paths,
        [
            "/search/tracks",
            "/search/users",
            "/search/albums",
            "/search/playlists_without_albums"
        ]
    );
    // SoundCloud has no videos or channels: the chip answers empty, with no request.
    let before = fake.urls().len();
    for kind in [SearchKind::Videos, SearchKind::Channels] {
        let empty = catalog.search_kind("a", kind).await.expect("empty answer");
        assert!(empty.songs.is_empty() && empty.artists.is_empty());
    }
    assert_eq!(fake.urls().len(), before);
}

#[tokio::test]
async fn previews_and_protected_tracks_are_listed_and_still_attributed() {
    let (catalog, _) = catalog_with(|_| ok(SEARCH_TRACKS));
    let songs = catalog
        .search_kind("a", SearchKind::Songs)
        .await
        .expect("songs")
        .songs;
    // The preview and the encrypted-only track stay in the list; playback explains them.
    assert!(
        songs
            .iter()
            .all(|t| !t.title.is_empty() && !t.artists.is_empty())
    );
    assert!(
        songs
            .iter()
            .all(|t| t.source_url.is_some() && t.origin == Origin::SoundCloud)
    );
}

#[tokio::test]
async fn a_playlist_fills_its_stubs_in_order() {
    let (catalog, fake) = catalog_with(|path| {
        if path.starts_with("/playlists/") {
            ok(PLAYLIST_WITH_STUBS)
        } else {
            ok(TRACKS_BATCH)
        }
    });
    let id = map::browse_id(Kind::Playlist, 7_000_023);
    let page = catalog.playlist(&id).await.expect("playlist");
    assert_eq!(page.summary.title, "Fixture Playlist With Stubs");
    assert_eq!(
        page.tracks.len(),
        5,
        "two full tracks and three filled stubs"
    );
    let titles: Vec<&str> = page.tracks.iter().map(|t| t.title.as_str()).collect();
    assert!(titles[0].starts_with("Fixture Track") && titles[4].starts_with("Fixture Track"));
    let urls = api_urls(&fake);
    assert_eq!(urls.len(), 2, "the page and one batch");
    assert!(
        urls[1].starts_with("https://api-v2.soundcloud.com/tracks?ids=7000003%2C7000004%2C7000005")
            || urls[1].contains("ids=")
    );
    assert_eq!(page.meta.track_count, Some(5));
    assert!(page.edit.is_none());
}

#[tokio::test]
async fn stubs_are_fetched_in_batches_of_fifty_and_removed_tracks_drop_out() {
    let stubs: Vec<serde_json::Value> = (0..120_u64)
        .map(|n| serde_json::json!({"kind": "track", "id": 7_100_000 + n, "policy": "ALLOW"}))
        .collect();
    let playlist = serde_json::json!({
        "kind": "playlist", "id": 7_100_000_u64 - 1, "title": "Fixture Big", "is_album": false,
        "user": {"username": "Fixture Artist 1"}, "track_count": 120, "tracks": stubs
    });
    let body = playlist.to_string();
    let (catalog, fake) = catalog_with(move |path| {
        if path.starts_with("/playlists/") {
            return ok(body.clone());
        }
        // Answer every id except the odd ones (removed tracks).
        let ids = path
            .split("ids=")
            .nth(1)
            .unwrap_or("")
            .split('&')
            .next()
            .unwrap_or("")
            .replace("%2C", ",");
        let tracks: Vec<serde_json::Value> = ids
            .split(',')
            .filter_map(|s| s.parse::<u64>().ok())
            .filter(|id| id % 2 == 0)
            .map(|id| {
                serde_json::json!({
                    "kind": "track", "id": id, "title": format!("Fixture Track {id}"),
                    "permalink_url": format!("https://soundcloud.com/fixture-artist-1/t-{id}"),
                    "user": {"username": "Fixture Artist 1"}
                })
            })
            .collect();
        ok(serde_json::Value::Array(tracks).to_string())
    });
    let page = catalog
        .playlist(&map::browse_id(Kind::Playlist, 6_999_999))
        .await
        .expect("playlist");
    assert_eq!(
        api_urls(&fake).len(),
        1 + 3,
        "the page and three batches (50, 50, 20)"
    );
    assert_eq!(page.tracks.len(), 60, "odd ids were removed");
    assert!(
        page.tracks.windows(2).all(|w| w[0].id.0 < w[1].id.0),
        "playlist order is kept"
    );
}

#[tokio::test]
async fn an_album_page_is_a_playlist_page_with_album_facts() {
    let (catalog, _) = catalog_with(|path| {
        if path.starts_with("/playlists/") {
            ok(PLAYLIST_WITH_STUBS)
        } else {
            ok(TRACKS_BATCH)
        }
    });
    let page = catalog
        .album(&map::browse_id(Kind::Playlist, 1))
        .await
        .expect("album");
    assert_eq!(page.tracks.len(), 5);
    assert_eq!(page.summary.year, Some(2026));
}

#[tokio::test]
async fn an_artist_page_is_three_requests() {
    let (catalog, fake) = catalog_with(|path| {
        if path.starts_with("/users/7000100/tracks") {
            ok(USER_TRACKS)
        } else if path.starts_with("/users/7000100/albums") {
            ok(USER_ALBUMS)
        } else {
            ok(USER)
        }
    });
    let page = catalog
        .artist(&map::browse_id(Kind::User, 7_000_100))
        .await
        .expect("artist");
    assert!(page.summary.name.starts_with("Fixture Artist"));
    assert_eq!(page.top_songs.len(), 2);
    assert_eq!(page.albums.len(), 1);
    assert!(
        page.description
            .as_deref()
            .is_some_and(|d| d.starts_with("A short fixture biography"))
    );
    assert_eq!(api_urls(&fake).len(), 3);
}

#[tokio::test]
async fn a_track_page_is_the_track_then_what_soundcloud_suggests() {
    let (catalog, fake) = catalog_with(|path| {
        if path.starts_with("/tracks/7000001/related") {
            ok(RELATED)
        } else {
            ok(TRACK_HLS)
        }
    });
    let page = catalog
        .playlist(&BrowseId("sc:track:7000001".to_owned()))
        .await
        .expect("page");
    assert_eq!(page.summary.id, BrowseId("sc:track:7000001".to_owned()));
    assert_eq!(page.tracks[0].id, TrackId("sc:7000001".to_owned()));
    assert_eq!(page.tracks.len(), 3, "the track and two suggestions");
    assert_eq!(page.meta.track_count, Some(3));
    assert_eq!(page.meta.owner.as_deref(), Some("Fixture Artist 1"));
    assert_eq!(page.summary.title, page.tracks[0].title);
    assert!(page.edit.is_none(), "a track page is never editable");
    let urls = api_urls(&fake);
    assert_eq!(urls.len(), 2, "the track, then its related tracks");
    assert!(urls[0].starts_with("https://api-v2.soundcloud.com/tracks/7000001?"));
    assert!(urls[1].contains("/tracks/7000001/related?"));
}

#[tokio::test]
async fn a_track_page_whose_suggestions_fail_still_shows_the_track() {
    let (catalog, _) = catalog_with(|path| {
        if path.contains("/related") {
            status(500, "{}")
        } else {
            ok(TRACK_HLS)
        }
    });
    let page = catalog
        .playlist(&BrowseId("sc:track:7000001".to_owned()))
        .await
        .expect("page");
    assert_eq!(page.tracks.len(), 1);
}

#[tokio::test]
async fn a_track_page_that_is_gone_is_not_found_and_a_bad_id_asks_nothing() {
    let (catalog, fake) = catalog_with(|_| status(404, "{}"));
    assert_eq!(
        catalog
            .playlist(&BrowseId("sc:track:7000001".to_owned()))
            .await,
        Err(CatalogError::NotFound)
    );
    let before = api_urls(&fake).len();
    for bad in ["sc:track:", "sc:track:12/../me", "sc:track:1:2"] {
        assert!(catalog.playlist(&BrowseId(bad.to_owned())).await.is_err());
    }
    assert_eq!(api_urls(&fake).len(), before, "no request for a bad id");
}

#[tokio::test]
async fn related_tracks_take_a_track_id_and_nothing_else() {
    let (catalog, fake) = catalog_with(|_| ok(RELATED));
    let tracks = catalog
        .related(&TrackId("sc:7000001".to_owned()))
        .await
        .expect("related");
    assert_eq!(tracks.len(), 2);
    assert!(
        api_urls(&fake)[0]
            .starts_with("https://api-v2.soundcloud.com/tracks/7000001/related?limit=20")
    );
    for bad in ["7000001", "sc:../me", "sc:1?x=2", "yt:abc"] {
        assert_eq!(
            catalog.related(&TrackId(bad.to_owned())).await,
            Err(CatalogError::NotFound),
            "{bad}"
        );
    }
    assert_eq!(api_urls(&fake).len(), 1, "a bad id makes no request");
}

#[tokio::test]
async fn a_link_resolves_to_a_track_an_album_a_playlist_or_an_artist() {
    for (body, expected) in [(TRACK_HLS.to_owned(), "track"), (USER.to_owned(), "artist")] {
        let (catalog, fake) = catalog_with(move |_| ok(body.clone()));
        let url = MediaUrl::parse(
            "https://soundcloud.com/fixture-artist-1/fixture-track-1",
            crate::hosts::HOSTS,
        )
        .expect("url");
        let link = catalog.open_link(&url).await.expect("link");
        match (&link, expected) {
            (Link::Track(t), "track") => assert_eq!(t.id.0, "sc:7000001"),
            (Link::Artist(id), "artist") => assert!(id.0.starts_with("sc:user:")),
            other => panic!("unexpected link {other:?}"),
        }
        let url = &api_urls(&fake)[0];
        assert!(url.contains(
            "/resolve?url=https%3A%2F%2Fsoundcloud.com%2Ffixture-artist-1%2Ffixture-track-1"
        ));
    }
    let playlist = serde_json::json!({"kind": "playlist", "id": 5, "title": "x", "is_album": true})
        .to_string();
    let (catalog, _) = catalog_with(move |_| ok(playlist.clone()));
    let url = MediaUrl::parse("https://soundcloud.com/a/sets/b", crate::hosts::HOSTS).expect("url");
    assert_eq!(
        catalog.open_link(&url).await,
        Ok(Link::Album(map::browse_id(Kind::Playlist, 5)))
    );
}

#[tokio::test]
async fn errors_become_plain_catalog_errors_without_addresses() {
    let (catalog, _) = catalog_with(|_| status(404, "{}"));
    assert_eq!(catalog.search("a").await, Err(CatalogError::NotFound));
    let (catalog, _) = catalog_with(|_| ok("not json"));
    let Err(CatalogError::Parse(text)) = catalog.search("a").await else {
        panic!("expected a parse error");
    };
    assert!(!text.contains("json") && !text.contains("http"));
    let (catalog, _) = catalog_with(|_| status(500, ""));
    let Err(CatalogError::Network(text)) = catalog.search("a").await else {
        panic!("expected a network error");
    };
    assert!(!text.contains("http") && !text.contains("client_id"));
    let (catalog, _) = catalog_with(|_| Err(ScHttpError::Timeout));
    assert!(matches!(
        catalog.search("a").await,
        Err(CatalogError::Network(_))
    ));
}

#[tokio::test]
async fn a_429_becomes_rate_limited_and_the_next_call_makes_no_request() {
    let (catalog, fake) = catalog_with(|_| status(429, ""));
    assert_eq!(catalog.search("a").await, Err(CatalogError::RateLimited));
    let seen = fake.urls().len();
    assert_eq!(catalog.search("b").await, Err(CatalogError::RateLimited));
    assert_eq!(fake.urls().len(), seen);
}

#[tokio::test]
async fn pages_that_other_sources_own_are_not_found() {
    let (catalog, fake) = catalog_with(|_| ok("{}"));
    for id in ["PL12345", "sc:track:1", "sc:user:1", "sc:playlist:x"] {
        assert_eq!(
            catalog.playlist(&BrowseId(id.to_owned())).await.map(|_| ()),
            Err(CatalogError::NotFound),
            "{id}"
        );
    }
    assert_eq!(
        catalog
            .artist(&BrowseId("sc:playlist:1".to_owned()))
            .await
            .map(|_| ()),
        Err(CatalogError::NotFound)
    );
    assert!(fake.urls().is_empty());
}

#[tokio::test]
async fn covers_come_in_several_sizes() {
    let (catalog, _) = catalog_with(|_| ok(SEARCH_TRACKS));
    let songs = catalog
        .search_kind("a", SearchKind::Songs)
        .await
        .expect("songs")
        .songs;
    let Artwork::Remote(thumbs) = &songs[0].artwork else {
        panic!("expected remote artwork");
    };
    assert_eq!(
        thumbs.iter().map(|t| t.width).collect::<Vec<_>>(),
        [100, 300, 500]
    );
}

#[tokio::test]
async fn mapped_tracks_leave_a_record_of_their_streams_under_their_page() {
    let (catalog, _) = catalog_with(|_| ok(SEARCH_TRACKS));
    let songs = catalog
        .search_kind("a", SearchKind::Songs)
        .await
        .expect("songs")
        .songs;
    assert_eq!(songs.len(), 5);
    assert_eq!(catalog.client.records().len(), 5);
    for song in &songs {
        let page = song.source_url.as_ref().expect("page");
        let record = catalog
            .client
            .records()
            .get(page.as_str())
            .expect("a record");
        assert!(!record.transcodings.is_empty());
        assert_eq!(format!("sc:{}", record.id), song.id.0);
    }
    // The fourth fixture track is a preview.
    let preview = songs[3].source_url.as_ref().expect("page");
    assert!(
        catalog
            .client
            .records()
            .get(preview.as_str())
            .expect("record")
            .preview
    );
}
