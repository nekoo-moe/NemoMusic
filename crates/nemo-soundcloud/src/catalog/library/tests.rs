//! The account's own lists against recorded-shape fixtures and a fake HTTP layer. No real
//! account or token: the user id and the token below are invented.

use std::sync::Arc;
use std::time::Duration;

use nemo_core::catalog::Catalog;
use nemo_core::secret::Secret;

use super::*;
use crate::client::{Endpoints, Pacing, ScClient};
use crate::fake::{FakeHttp, ok};
use crate::fixtures::{FEED, FOLLOWINGS, HISTORY, LIBRARY_ALL, LIKES, LIKES_PAGE2, TRACKS_BATCH};
use crate::http::{ScAnswer, ScHttp, ScHttpError};

const ID: &str = "AbCdEfGh0123456789AbCdEfGh012345";
const HOME: &str = r#"<script src="https://a-v2.sndcdn.com/assets/1-a.js"></script>"#;
const TOKEN: &str = "2-293232-7000001-SyntheticToken";
const ME: u64 = 7_000_001;
/// The `next_href` the likes fixture carries.
const LIKES_NEXT: &str = "https://api-v2.soundcloud.com/users/7000001/track_likes?offset=7000100&limit=2&linked_partitioning=1";

fn fast() -> Pacing {
    Pacing {
        min_gap: Duration::ZERO,
        retry_delays: [Duration::ZERO, Duration::ZERO],
        default_backoff: Duration::from_millis(40),
        max_backoff: Duration::from_millis(40),
    }
}

/// The site, then `lists` answers the account calls.
fn rig(
    lists: impl Fn(&str, usize) -> Result<ScAnswer, ScHttpError> + Send + Sync + 'static,
    signed_in: bool,
) -> (SoundcloudCatalog, Arc<FakeHttp>) {
    let http = Arc::new(FakeHttp::new(move |url, index| {
        if url == "https://soundcloud.com/" {
            ok(HOME)
        } else if url.ends_with("1-a.js") {
            ok(format!("x={{client_id:\"{ID}\"}}"))
        } else {
            lists(url, index)
        }
    }));
    let client = Arc::new(ScClient::with(
        Arc::clone(&http) as Arc<dyn ScHttp>,
        Endpoints::production(),
        fast(),
    ));
    if signed_in {
        client.auth().set(Secret::new(TOKEN.to_owned()));
        client.auth().set_user(Some(ME));
    }
    (SoundcloudCatalog::new(client), http)
}

/// The account calls only (without the client id look-up).
fn list_urls(http: &FakeHttp) -> Vec<String> {
    http.urls()
        .into_iter()
        .filter(|u| u.starts_with("https://api-v2.soundcloud.com/"))
        .collect()
}

fn routes(url: &str, _: usize) -> Result<ScAnswer, ScHttpError> {
    let path = url.split("client_id").next().unwrap_or(url);
    if path.contains("/track_likes?offset=") {
        ok(LIKES_PAGE2)
    } else if path.contains("/track_likes") {
        ok(LIKES)
    } else if path.contains("/me/library/all") {
        ok(LIBRARY_ALL)
    } else if path.contains("/followings") {
        ok(FOLLOWINGS)
    } else if path.contains("/me/play-history/tracks") {
        ok(HISTORY)
    } else if path.contains("/stream") {
        ok(FEED)
    } else if path.contains("/tracks?ids=") {
        ok(TRACKS_BATCH)
    } else {
        ok("{}")
    }
}

#[tokio::test]
async fn every_read_while_signed_out_says_sign_in_and_sends_nothing() {
    let (catalog, http) = rig(routes, false);
    let sign_in = CatalogError::Unavailable("Sign in to SoundCloud to see this".to_owned());
    assert_eq!(catalog.likes_page(None).await, Err(sign_in.clone()));
    assert_eq!(catalog.library_page(None).await, Err(sign_in.clone()));
    assert_eq!(catalog.followings_page(None).await, Err(sign_in.clone()));
    assert_eq!(catalog.feed_page(None).await, Err(sign_in.clone()));
    assert_eq!(catalog.history_page(None).await, Err(sign_in.clone()));
    assert_eq!(catalog.liked_songs().await, Err(sign_in));
    assert!(http.urls().is_empty(), "not even the client id look-up");
}

#[tokio::test]
async fn likes_come_page_by_page_with_the_token_to_the_api_host_only() {
    let (catalog, http) = rig(routes, true);
    let first = catalog.likes_page(None).await.expect("first page");
    assert_eq!(first.items.len(), 2);
    assert_eq!(first.items[0].title, "Fixture Track 1");
    let next = first.next.expect("there is a next page");
    assert_eq!(next.as_str(), LIKES_NEXT);
    let second = catalog.likes_page(Some(&next)).await.expect("second page");
    assert_eq!(second.items.len(), 1);
    assert!(second.next.is_none());
    let urls = list_urls(&http);
    assert!(
        urls[0].starts_with("https://api-v2.soundcloud.com/users/7000001/track_likes?limit=50")
    );
    assert!(urls[1].contains("offset=7000100") && urls[1].contains("client_id="));
    // The token went to the API calls only, and never into an address.
    for (token, url) in http.tokens().into_iter().zip(http.urls()) {
        let api = url.starts_with("https://api-v2.soundcloud.com/");
        assert_eq!(token.as_deref(), api.then_some(TOKEN), "{url}");
    }
    assert!(http.urls().iter().all(|u| !u.contains(TOKEN)));
}

#[tokio::test]
async fn the_catalog_call_reads_every_page_up_to_the_cap() {
    let (catalog, http) = rig(routes, true);
    let all = catalog.liked_songs().await.expect("likes");
    assert_eq!(all.len(), 3, "both pages");
    assert_eq!(list_urls(&http).len(), 2);

    // A list that never ends stops at the cap.
    let (endless, http) = rig(|_, _| ok(LIKES), true);
    let tracks = endless.liked_songs().await.expect("likes");
    assert_eq!(tracks.len(), 2 * MAX_PAGES);
    assert_eq!(list_urls(&http).len(), MAX_PAGES);
}

#[tokio::test]
async fn a_next_address_off_the_api_host_is_dropped_and_never_requested() {
    for foreign in [
        "https://evil.example/track_likes?offset=1",
        "https://api-v2.soundcloud.com.evil.example/x",
        "https://api-v2.soundcloud.com@evil.example/x",
        "http://api-v2.soundcloud.com/users/7000001/track_likes",
        "api-v2.soundcloud.com/users/1/track_likes",
    ] {
        let body = LIKES.replace(LIKES_NEXT, foreign);
        let (catalog, http) = rig(move |_, _| ok(body.clone()), true);
        let page = catalog.likes_page(None).await.expect("page");
        assert!(page.next.is_none(), "{foreign}");
        assert_eq!(catalog.liked_songs().await.expect("all").len(), 2);
        assert!(
            http.urls().iter().all(|u| !u.contains("evil.example")),
            "{foreign}"
        );
    }
}

#[tokio::test]
async fn the_library_sorts_own_and_liked_playlists_and_albums_and_skips_the_rest() {
    let (catalog, _) = rig(routes, true);
    let page = catalog.library_page(None).await.expect("page");
    let lib = &page.items[0];
    let titles = |l: &[PlaylistSummary]| l.iter().map(|p| p.title.clone()).collect::<Vec<_>>();
    assert_eq!(titles(&lib.playlists), ["Fixture Own Playlist"]);
    assert_eq!(titles(&lib.liked_playlists), ["Fixture Liked Playlist"]);
    assert_eq!(lib.albums[0].title, "Fixture Own Album");
    assert_eq!(lib.liked_albums[0].title, "Fixture Liked Album");
    let playlists = catalog.library_playlists().await.expect("playlists");
    assert_eq!(
        titles(&playlists),
        ["Fixture Own Playlist", "Fixture Liked Playlist"],
        "own first"
    );
    let albums = catalog.library_albums().await.expect("albums");
    assert_eq!(albums.len(), 2);
}

#[tokio::test]
async fn followed_artists_map_to_artist_pages() {
    let (catalog, http) = rig(routes, true);
    let page = catalog.followings_page(None).await.expect("artists");
    assert_eq!(page.items.len(), 3);
    assert_eq!(page.items[0].name, "Fixture Followed 1");
    assert!(page.items[0].id.0.contains("7000300"));
    let next = page.next.expect("a next page");
    assert!(
        next.as_str()
            .starts_with("https://api-v2.soundcloud.com/users/7000001/followings")
    );
    assert!(list_urls(&http)[0].contains("/users/7000001/followings"));
    // The catalog call follows the pages, up to the cap.
    let all = catalog.library_artists().await.expect("all");
    assert_eq!(all.len(), 3 * MAX_PAGES);
}

#[tokio::test]
async fn the_feed_keeps_tracks_and_reposts_and_drops_playlists() {
    let (catalog, _) = rig(routes, true);
    let page = catalog.feed_page(None).await.expect("feed");
    assert_eq!(page.items.len(), 2);
    let next = page.next.expect("next");
    assert!(
        next.as_str()
            .starts_with("https://api-v2.soundcloud.com/stream")
    );
}

#[tokio::test]
async fn history_fills_a_stub_in_place_and_groups_by_day() {
    let (catalog, http) = rig(routes, true);
    let page = catalog.history_page(None).await.expect("history");
    let plays = page.items;
    assert_eq!(plays.len(), 4, "the stub was filled from /tracks?ids=");
    assert!(
        http.urls()
            .iter()
            .any(|u| u.contains("/tracks?ids=7000023")),
        "one batch call for the one stub"
    );
    assert!(plays.windows(2).all(|w| w[0].at_ms >= w[1].at_ms));
    let now = 7_000_864_000_000;
    let grouped = sections(plays, now);
    let shape: Vec<(&str, usize)> = grouped
        .iter()
        .map(|s| (s.title.as_str(), s.tracks.len()))
        .collect();
    assert_eq!(shape, [("Today", 2), ("Yesterday", 1), ("Earlier", 1)]);
}

#[test]
fn day_labels_and_grouping_follow_whole_days() {
    let value: Value = serde_json::from_str(LIKES).expect("fixture");
    let raw: ScTrack =
        serde_json::from_value(value["collection"][0]["track"].clone()).expect("track");
    let base = crate::map::track(&raw).expect("mapped");
    let track = |n: u64| Track {
        id: nemo_core::track::TrackId(format!("sc:{n}")),
        ..base.clone()
    };
    let day = 86_400_000_i64;
    let now = 100 * day + 5;
    let plays = vec![
        Played {
            at_ms: now,
            track: track(1),
        },
        Played {
            at_ms: 100 * day,
            track: track(2),
        },
        Played {
            at_ms: 99 * day + 1,
            track: track(3),
        },
        Played {
            at_ms: 97 * day,
            track: track(4),
        },
        Played {
            at_ms: 90 * day,
            track: track(5),
        },
        Played {
            at_ms: now + 2 * day,
            track: track(6),
        },
    ];
    let titles: Vec<String> = sections(plays, now).into_iter().map(|s| s.title).collect();
    assert_eq!(
        titles,
        ["Today", "Yesterday", "3 days ago", "Earlier", "Today"]
    );
    assert!(sections(Vec::new(), now).is_empty());
}

#[tokio::test]
async fn an_answer_that_is_not_a_list_is_a_parse_error_without_its_text() {
    let (catalog, _) = rig(|_, _| ok("not json at all <secret>"), true);
    let error = catalog.likes_page(None).await.expect_err("parse");
    assert_eq!(
        error,
        CatalogError::Parse("unexpected answer from SoundCloud".to_owned())
    );
}

#[tokio::test]
async fn another_users_sign_in_leaves_nothing_of_the_first_to_read() {
    let (catalog, http) = rig(routes, true);
    catalog.likes_page(None).await.expect("first user's likes");
    // Signing out (or in as someone else) clears the user id at once.
    catalog.client.auth().clear();
    assert!(catalog.likes_page(None).await.is_err());
    catalog.client.auth().set(Secret::new(TOKEN.to_owned()));
    catalog.client.auth().set_user(Some(7_000_002));
    catalog.likes_page(None).await.expect("second user's likes");
    let urls = list_urls(&http);
    assert!(
        urls.last()
            .is_some_and(|u| u.contains("/users/7000002/track_likes"))
    );
}
