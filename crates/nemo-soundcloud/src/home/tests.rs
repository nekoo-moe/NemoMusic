//! Tests for home, charts, mixes and radio over the fake HTTP layer and recorded, rewritten
//! fixtures.

use std::sync::Arc;
use std::time::Duration;

use nemo_core::catalog::{Catalog, FeedItem, ShelfItem, ShelfKind};

use super::*;
use crate::client::{Endpoints, Pacing, ScClient};
use crate::fake::{FakeHttp, ok, status};
use crate::fixtures::{
    CHARTS_SELECTIONS, CHARTS_TRENDING, HOME, STATION, SYSTEM_PLAYLIST, SYSTEM_TRACKS_BATCH,
};
use crate::http::{ScAnswer, ScHttp, ScHttpError};

const ID: &str = "AbCdEfGh0123456789AbCdEfGh012345";

fn pacing() -> Pacing {
    Pacing {
        min_gap: Duration::ZERO,
        retry_delays: [Duration::ZERO, Duration::ZERO],
        default_backoff: Duration::from_millis(20),
        max_backoff: Duration::from_millis(20),
    }
}

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
        Arc::clone(&fake) as Arc<dyn ScHttp>,
        Endpoints::production(),
        pacing(),
    );
    (SoundcloudCatalog::new(Arc::new(client)), fake)
}

fn api_urls(fake: &FakeHttp) -> Vec<String> {
    fake.urls()
        .into_iter()
        .filter(|u| u.starts_with("https://api-v2.soundcloud.com"))
        .map(|u| {
            u.trim_start_matches("https://api-v2.soundcloud.com")
                .to_owned()
        })
        .collect()
}

#[test]
fn page_ids_round_trip_and_reject_anything_that_could_leave_the_path() {
    for page in [
        Page::System("trending-by-genre:trap".to_owned()),
        Page::Chart,
        Page::Station(7_200_001),
    ] {
        assert_eq!(Page::parse(&page.id()), Some(page));
    }
    for bad in [
        "",
        "sc:system:",
        "sc:system:../me",
        "sc:system:a b",
        "sc:system:A",
        "sc:system:a?x=1",
        "sc:chart:top:all-music",
        "sc:chart:trending:trap",
        "sc:station:",
        "sc:station:1a",
        "sc:station:../1",
        "sc:station:-1",
        "sc:playlist:1",
        "yt:system:a",
    ] {
        assert_eq!(Page::parse(&BrowseId(bad.to_owned())), None, "{bad:?}");
    }
    let long = format!("sc:system:{}", "a".repeat(MAX_TAIL + 1));
    assert_eq!(Page::parse(&BrowseId(long)), None);
}

#[tokio::test]
async fn the_home_page_is_one_request_and_keeps_each_row_as_the_source_titles_it() {
    let (catalog, fake) = catalog_with(|_| ok(HOME));
    let feed = catalog.home_feed().await.expect("home");
    assert_eq!(
        api_urls(&fake),
        ["/mixed-selections?client_id=".to_owned() + ID]
    );
    let titles: Vec<&str> = feed.shelves.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Fixture Selection One",
            "Fixture Trending By Genre",
            "Fixture Mixed Kinds"
        ],
        "an empty row is dropped"
    );
    assert!(feed.chips.is_empty());
    assert!(feed.shelves.iter().all(|s| s.kind == ShelfKind::Cards));
    // Playlists, system playlists as playlists with their own page ids, and mixed kinds.
    let first: Vec<_> = feed.shelves[0]
        .items
        .iter()
        .map(|i| matches!(i, FeedItem::Playlist(_)))
        .collect();
    assert_eq!(first, [true, true, true]);
    let FeedItem::Playlist(mix) = &feed.shelves[1].items[0] else {
        panic!("a system playlist is a playlist");
    };
    assert_eq!(mix.id.0, "sc:system:trending-by-genre:trap");
    assert!(mix.title.starts_with("Fixture"));
    assert!(!mix.thumbnails.is_empty(), "the computed cover is used");
    let kinds: Vec<&str> = feed.shelves[2]
        .items
        .iter()
        .map(|i| match i {
            FeedItem::Song(_) => "song",
            FeedItem::Artist(_) => "artist",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["song", "artist"], "an unknown kind is skipped");
}

#[tokio::test]
async fn the_plain_home_has_the_same_rows_as_shelves() {
    let (catalog, _) = catalog_with(|_| ok(HOME));
    let shelves = catalog.home().await.expect("home");
    assert_eq!(shelves.len(), 3);
    assert!(matches!(shelves[0].items[0], ShelfItem::Playlist(_)));
    assert!(matches!(shelves[2].items[0], ShelfItem::Song(_)));
}

#[tokio::test]
async fn the_charts_page_has_the_trending_songs_and_the_chart_playlists() {
    let (catalog, fake) = catalog_with(|path| {
        if path.starts_with("/charts/selections") {
            ok(CHARTS_SELECTIONS)
        } else {
            ok(CHARTS_TRENDING)
        }
    });
    let explore = catalog.explore().await.expect("explore");
    let urls = api_urls(&fake);
    assert_eq!(urls.len(), 2);
    assert!(
        urls[0].starts_with("/charts?kind=trending&genre=soundcloud%3Agenres%3Aall-music&limit=30"),
        "{}",
        urls[0]
    );
    assert!(urls[1].starts_with("/charts/selections"));
    assert_eq!(explore.charts.len(), 1);
    assert_eq!(explore.charts[0].kind, ShelfKind::QuickPicks);
    assert_eq!(
        explore.charts[0].items.len(),
        3,
        "the tracks of the entries, in chart order"
    );
    assert_eq!(explore.shelves.len(), 2);
    assert!(explore.new_releases.is_empty() && explore.moods.is_empty());
}

#[tokio::test]
async fn one_failing_half_of_the_charts_page_leaves_the_other() {
    let (catalog, _) = catalog_with(|path| {
        if path.starts_with("/charts/selections") {
            status(404, "{}")
        } else {
            ok(CHARTS_TRENDING)
        }
    });
    let explore = catalog.explore().await.expect("the trending half");
    assert_eq!(explore.charts.len(), 1);
    assert!(explore.shelves.is_empty());
    let (catalog, _) = catalog_with(|_| status(404, "{}"));
    assert_eq!(
        catalog.explore().await.map(|_| ()),
        Err(CatalogError::NotFound)
    );
}

#[tokio::test]
async fn a_mix_fills_its_track_ids_in_order_and_names_the_page_it_is() {
    let (catalog, fake) = catalog_with(|path| {
        if path.starts_with("/system-playlists/") {
            ok(SYSTEM_PLAYLIST)
        } else {
            ok(SYSTEM_TRACKS_BATCH)
        }
    });
    let id = Page::System("trending-by-genre:trap".to_owned()).id();
    let page = catalog.playlist(&id).await.expect("mix");
    let urls = api_urls(&fake);
    assert!(
        urls[0].starts_with(
            "/system-playlists/soundcloud:system-playlists:trending-by-genre:trap?client_id="
        ),
        "{}",
        urls[0]
    );
    assert!(urls[1].starts_with("/tracks?ids="), "one batch for the ids");
    assert_eq!(page.tracks.len(), 3);
    assert_eq!(page.summary.id, id);
    assert!(page.summary.title.starts_with("Fixture"));
    assert_eq!(page.meta.track_count, Some(3));
    assert!(page.edit.is_none());
}

#[tokio::test]
async fn the_chart_is_a_playlist_page_of_the_trending_tracks() {
    let (catalog, fake) = catalog_with(|_| ok(CHARTS_TRENDING));
    let page = catalog
        .playlist(&Page::Chart.id())
        .await
        .expect("chart page");
    assert_eq!(page.tracks.len(), 3);
    assert_eq!(page.summary.title, "Trending on SoundCloud");
    assert!(
        !page.summary.thumbnails.is_empty(),
        "the first track's cover stands in"
    );
    assert_eq!(api_urls(&fake).len(), 1);
}

#[tokio::test]
async fn a_radio_is_a_list_of_tracks_and_a_playlist_page() {
    let (catalog, fake) = catalog_with(|_| ok(STATION));
    let tracks = catalog
        .radio(&TrackId("sc:7200001".to_owned()))
        .await
        .expect("radio");
    assert_eq!(tracks.len(), 4);
    assert!(tracks.iter().all(|t| t.source_url.is_some()));
    assert!(
        api_urls(&fake)[0]
            .starts_with("/stations/soundcloud:track-stations:7200001/tracks?client_id=")
    );
    let page = catalog
        .playlist(&Page::Station(7_200_001).id())
        .await
        .expect("page");
    assert_eq!(page.tracks.len(), 4);
    assert_eq!(page.summary.id.0, "sc:station:7200001");
    // The tracks left records for the stream lookup.
    let first = tracks[0].source_url.as_ref().expect("page");
    assert!(catalog.client.records().get(first.as_str()).is_some());
}

#[tokio::test]
async fn radio_takes_only_a_soundcloud_track_id() {
    let (catalog, fake) = catalog_with(|_| ok(STATION));
    for bad in ["7200001", "sc:../me", "sc:1?x=2", "yt:abc", "sc:"] {
        assert_eq!(
            catalog.radio(&TrackId(bad.to_owned())).await,
            Err(CatalogError::NotFound),
            "{bad}"
        );
    }
    assert!(
        fake.urls().is_empty(),
        "a bad id makes no request, not even the client id"
    );
}

#[tokio::test]
async fn errors_are_plain_and_a_limit_stops_the_next_call() {
    let (catalog, fake) = catalog_with(|_| status(429, ""));
    assert_eq!(
        catalog.home().await.map(|_| ()),
        Err(CatalogError::RateLimited)
    );
    let seen = fake.urls().len();
    assert_eq!(
        catalog.radio(&TrackId("sc:1".to_owned())).await.map(|_| ()),
        Err(CatalogError::RateLimited)
    );
    assert_eq!(fake.urls().len(), seen);
    let (catalog, _) = catalog_with(|_| ok("not json"));
    assert!(matches!(catalog.home().await, Err(CatalogError::Parse(_))));
}
