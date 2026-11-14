//! Tests for all-YouTube search: the parser on the spike fixture, the request order, the
//! backoff and the reqwest transport against the loopback test server.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};

use nemo_core::catalog::{ArtistDetail, PlaylistDetail};
use nemo_core::track::Artwork;
use serde_json::Value;

use super::*;
use crate::test_server::{Response, TestServer};

const FIXTURE: &str = include_str!("../../tests/fixtures/web/search.json");

/// Poll until ready with a no-op waker; the fakes below make progress on every poll.
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(Waker::noop());
    for _ in 0..10_000 {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        std::thread::yield_now();
    }
    panic!("the future did not finish");
}

type Log = Arc<Mutex<Vec<&'static str>>>;

fn push(log: &Log, entry: &'static str) {
    log.lock().expect("log").push(entry);
}

/// A future that is pending once, so a search takes two polls.
struct YieldOnce(bool);

impl Future for YieldOnce {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            Poll::Pending
        }
    }
}

/// The music catalog: logs when its search starts and ends.
struct Music(Log);

impl Catalog for Music {
    fn search<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        Box::pin(async move {
            push(&self.0, "music-start");
            YieldOnce(false).await;
            push(&self.0, "music-end");
            Ok(SearchResults {
                playlists: vec![PlaylistSummary {
                    id: BrowseId("PLe0XBeaAl1KElgUjwGuNtut0AdjryI4DD".to_owned()),
                    title: "Same playlist".to_owned(),
                    author: None,
                    thumbnails: Vec::new(),
                }],
                ..SearchResults::default()
            })
        })
    }

    fn album<'a>(&'a self, _: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        Box::pin(async { Err(CatalogError::NotFound) })
    }

    fn artist<'a>(&'a self, _: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        Box::pin(async { Err(CatalogError::NotFound) })
    }

    fn playlist<'a>(
        &'a self,
        _: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        Box::pin(async { Err(CatalogError::NotFound) })
    }
}

/// The web host: logs each request (when it is sent) and answers `status`.
struct Web {
    log: Log,
    calls: AtomicUsize,
    status: u16,
    retry_after: Option<Duration>,
}

impl WebPost for Web {
    fn post<'a>(&'a self, url: &'a str, body: String) -> BoxFuture<'a, Result<WebAnswer, String>> {
        assert_eq!(url, SEARCH_URL);
        assert!(body.contains("\"query\":\"lofi girl\""), "{body}");
        push(&self.log, "web");
        self.calls.fetch_add(1, Ordering::SeqCst);
        let answer = WebAnswer {
            status: self.status,
            body: FIXTURE.to_owned(),
            retry_after: self.retry_after,
        };
        Box::pin(async move { Ok(answer) })
    }
}

fn ext(status: u16, retry_after: Option<Duration>) -> (WebCatalogExt<Web>, Log) {
    let log = Log::default();
    let web = Web {
        log: Arc::clone(&log),
        calls: AtomicUsize::new(0),
        status,
        retry_after,
    };
    (
        WebCatalogExt::new(Arc::new(Music(Arc::clone(&log))), web),
        log,
    )
}

fn all(catalog: &WebCatalogExt<Web>) -> Result<SearchResults, CatalogError> {
    block_on(catalog.search_scoped("lofi girl", SearchScope::AllOfYoutube))
}

#[test]
fn the_fixture_gives_a_live_video_a_video_and_a_playlist() {
    let found = parse_search(FIXTURE).expect("parses");
    assert_eq!(found.videos.len(), 2);
    let live = &found.videos[0];
    assert_eq!(
        live.track.video_id.as_ref().map(VideoId::as_str),
        Some("rFZHOHl-L8A")
    );
    assert!(live.live);
    assert_eq!(live.track.duration, None);
    assert_eq!(live.views.as_deref(), Some("8,384 watching"));
    let channel = live.channel.as_ref().expect("channel");
    assert_eq!(channel.id, BrowseId("UCSJ4gkVC6NrvII8umztf0Ow".to_owned()));
    assert_eq!(live.track.artists, vec!["Lofi Girl".to_owned()]);

    let plain = &found.videos[1];
    assert_eq!(plain.track.title, "1 A.M Study Session 📚 [lofi hip hop]");
    assert!(!plain.live);
    assert_eq!(plain.track.duration, Some(Duration::from_secs(3674)));
    assert_eq!(plain.views.as_deref(), Some("138,166,841 views"));
    assert!(
        matches!(&plain.track.artwork, Artwork::Remote(t) if t[0].url.starts_with("https://i.ytimg.com/"))
    );

    assert_eq!(found.playlists.len(), 1);
    assert_eq!(
        found.playlists[0].id.0,
        "PLe0XBeaAl1KElgUjwGuNtut0AdjryI4DD"
    );
    assert_eq!(found.playlists[0].author.as_deref(), Some("Julie K"));
    // The stripped fixture's channel card lost its channel id, so no channel is made up.
    assert!(found.channels.is_empty());
}

#[test]
fn a_channel_card_with_its_id_in_the_header_is_a_channel() {
    let mut root: Value = serde_json::from_str(FIXTURE).expect("json");
    let card = root
        .pointer_mut("/contents/twoColumnSearchResultsRenderer/primaryContents/sectionListRenderer/contents/0/itemSectionRenderer/contents/0/officialCardViewModel/header/pageHeaderViewModel")
        .expect("card header");
    card["rendererContext"] = serde_json::json!({ "commandContext": { "onTap": {
        "innertubeCommand": { "browseEndpoint": { "browseId": "UCSJ4gkVC6NrvII8umztf0Ow" } }
    } } });
    let found = parse_search(&root.to_string()).expect("parses");
    assert_eq!(found.channels.len(), 1);
    let channel = &found.channels[0];
    assert_eq!(channel.name, "Lofi Girl");
    assert_eq!(channel.id.0, "UCSJ4gkVC6NrvII8umztf0Ow");
    assert_eq!(channel.subscribers.as_deref(), Some("15.8M subscribers"));
    assert!(
        channel.thumbnails[0]
            .url
            .starts_with("https://yt3.ggpht.com/")
    );
    assert_eq!(found.videos.len(), 2);
}

#[test]
fn a_mixed_answer_keeps_only_the_items_it_understands() {
    let text = r#"{"contents":{"twoColumnSearchResultsRenderer":{"primaryContents":{"sectionListRenderer":{"contents":[
        {"itemSectionRenderer":{"contents":[
            {"shelfRenderer":{"title":{"simpleText":"Latest"}}},
            {"shortsLockupViewModel":{"entityId":"x"}},
            {"videoRenderer":{"videoId":"not an id","title":{"simpleText":"Bad"}}},
            {"videoRenderer":{"videoId":"abcdefghijk","title":{"runs":[{"text":"Good"}]},"lengthText":{"simpleText":"3:05"}}},
            {"lockupViewModel":{"contentType":"LOCKUP_CONTENT_TYPE_VIDEO","contentId":"abcdefghijk"}},
            {"lockupViewModel":{"contentType":"LOCKUP_CONTENT_TYPE_PLAYLIST","contentId":"PL bad/id",
                "metadata":{"lockupMetadataViewModel":{"title":{"content":"Bad"}}}}}
        ]}},
        {"continuationItemRenderer":{}}
    ]}}}}}"#;
    let found = parse_search(text).expect("parses");
    assert_eq!(found.videos.len(), 1);
    assert_eq!(found.videos[0].track.title, "Good");
    assert_eq!(
        found.videos[0].track.duration,
        Some(Duration::from_secs(185))
    );
    assert!(found.videos[0].channel.is_none());
    assert!(found.playlists.is_empty() && found.channels.is_empty());
}

#[test]
fn an_answer_that_is_not_a_search_is_a_parse_error() {
    assert!(matches!(
        parse_search("<html>"),
        Err(CatalogError::Parse(_))
    ));
    assert!(matches!(parse_search("{}"), Err(CatalogError::Parse(_))));
}

#[test]
fn all_of_youtube_runs_the_music_search_then_one_web_search() {
    let (catalog, log) = ext(200, None);
    let found = all(&catalog).expect("search");
    assert_eq!(
        *log.lock().expect("log"),
        ["music-start", "music-end", "web"]
    );
    assert_eq!(found.videos.len(), 2);
    // The playlist both searches found is listed once, with the music answer's title.
    assert_eq!(found.playlists.len(), 1);
    assert_eq!(found.playlists[0].title, "Same playlist");
}

#[test]
fn music_scope_and_plain_search_never_reach_the_web() {
    let (catalog, log) = ext(200, None);
    block_on(catalog.search_scoped("lofi girl", SearchScope::Music)).expect("music");
    block_on(catalog.search("lofi girl")).expect("search");
    block_on(catalog.search_scoped("   ", SearchScope::AllOfYoutube)).expect("empty");
    assert_eq!(catalog.web.calls.load(Ordering::SeqCst), 0);
    assert_eq!(log.lock().expect("log").len(), 4);
}

#[test]
fn a_429_stops_all_of_youtube_search_with_no_request() {
    let (catalog, _) = ext(429, Some(Duration::from_secs(600)));
    assert_eq!(all(&catalog), Err(CatalogError::RateLimited));
    assert_eq!(all(&catalog), Err(CatalogError::RateLimited));
    assert_eq!(catalog.web.calls.load(Ordering::SeqCst), 1);

    let (catalog, _) = ext(429, None);
    assert_eq!(all(&catalog), Err(CatalogError::RateLimited));
    assert!(catalog.blocked(), "no Retry-After waits the default");

    let (catalog, _) = ext(429, Some(Duration::ZERO));
    assert_eq!(all(&catalog), Err(CatalogError::RateLimited));
    assert_eq!(all(&catalog), Err(CatalogError::RateLimited));
    assert_eq!(
        catalog.web.calls.load(Ordering::SeqCst),
        2,
        "a zero wait is over"
    );
}

#[test]
fn other_statuses_are_network_errors() {
    let (catalog, _) = ext(500, None);
    assert_eq!(
        all(&catalog),
        Err(CatalogError::Network("web search: HTTP 500".to_owned()))
    );
    assert!(!catalog.blocked());
}

#[test]
fn the_request_body_is_the_web_client_with_the_query_only() {
    let body: Value = serde_json::from_str(&search_body("lofi \"girl\"")).expect("json");
    assert_eq!(body["query"], "lofi \"girl\"");
    assert_eq!(body["context"]["client"]["clientName"], "WEB");
    assert_eq!(
        body["context"]["client"]["clientVersion"],
        WEB_CLIENT_VERSION
    );
    assert!(body.get("params").is_none());
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("runtime")
}

#[test]
fn the_reqwest_transport_reads_status_body_and_retry_after() {
    let server = TestServer::start(|request, _| match request.path.as_str() {
        "/ok" => Response::new(200, b"{}".to_vec()),
        _ => Response::new(429, Vec::new()).header("Retry-After", "42"),
    });
    let rt = runtime();
    let web = ReqwestWeb::build(rt.handle().clone(), false).expect("client");
    let ok = block_on_thread(web.post(&server.url("/ok"), "{}".to_owned())).expect("ok");
    assert_eq!((ok.status, ok.body.as_str()), (200, "{}"));
    let limited = block_on_thread(web.post(&server.url("/x"), "{}".to_owned())).expect("429");
    assert_eq!(limited.retry_after, Some(Duration::from_secs(42)));
}

#[test]
fn plain_http_is_refused_and_the_error_names_no_url() {
    let server = TestServer::start(|_, _| Response::new(200, Vec::new()));
    let rt = runtime();
    let web = ReqwestWeb::new(rt.handle().clone()).expect("client");
    let error = block_on_thread(web.post(&server.url("/q?secret=1"), "{}".to_owned()))
        .expect_err("refused");
    assert!(
        !error.contains("127.0.0.1") && !error.contains("secret"),
        "{error}"
    );
    assert!(server.requests().is_empty());
}

/// Wait on a future that a tokio task completes, parking between polls.
fn block_on_thread<F: Future>(future: F) -> F::Output {
    use std::task::Wake;

    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        std::thread::park();
    }
}
