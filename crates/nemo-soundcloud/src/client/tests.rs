//! Tests for the client: id discovery and refresh, pacing, backoff, retries.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;
use crate::fake::{FakeHttp, ok, status};
use crate::http::{ReqwestHttp, ScAnswer};
use crate::test_server::{Response, TestServer};

// Made-up ids, not real ones.
const ID: &str = "AbCdEfGh0123456789AbCdEfGh012345";
const ID2: &str = "ZyXwVuTs9876543210ZyXwVuTs987654";

const HOME: &str = r#"<script src="https://a-v2.sndcdn.com/assets/1-a.js"></script>
<script src="https://a-v2.sndcdn.com/assets/2-b.js"></script>"#;

fn fast() -> Pacing {
    Pacing {
        min_gap: Duration::ZERO,
        retry_delays: [Duration::ZERO, Duration::ZERO],
        default_backoff: Duration::from_millis(40),
        max_backoff: Duration::from_millis(40),
    }
}

fn client(fake: &Arc<FakeHttp>, pacing: Pacing) -> ScClient {
    ScClient::with(
        Arc::clone(fake) as Arc<dyn ScHttp>,
        Endpoints::production(),
        pacing,
    )
}

/// Home page, then the last script holds `id`; API calls answer `{}`.
fn site(id: &'static str) -> impl Fn(&str, usize) -> Result<ScAnswer, ScHttpError> {
    move |url, _| {
        if url == "https://soundcloud.com/" {
            ok(HOME)
        } else if url.ends_with("2-b.js") {
            ok(format!("x={{client_id:\"{id}\"}}"))
        } else if url.ends_with("1-a.js") {
            ok("no id here")
        } else {
            ok("{}")
        }
    }
}

#[tokio::test]
async fn the_id_is_found_in_the_last_script_and_reused() {
    let fake = Arc::new(FakeHttp::new(site(ID)));
    let client = client(&fake, fast());
    client
        .get("/search", &[("q", "a b")])
        .await
        .expect("first call");
    client.get("/tracks/1", &[]).await.expect("second call");
    let urls = fake.urls();
    assert_eq!(
        urls.len(),
        4,
        "home, one script, two api calls: {}",
        urls.len()
    );
    assert!(urls[1].ends_with("2-b.js"), "the last script is read first");
    assert!(urls[2].starts_with("https://api-v2.soundcloud.com/search?q=a+b&client_id="));
    assert!(urls[2].ends_with(ID) && urls[3].ends_with(ID));
}

#[tokio::test]
async fn a_403_looks_the_id_up_again_once() {
    let fake = Arc::new(FakeHttp::new(|url, index| match url {
        "https://soundcloud.com/" => ok(HOME),
        u if u.ends_with("2-b.js") => ok(format!(
            "client_id:\"{}\"",
            if index < 3 { ID } else { ID2 }
        )),
        u if u.contains(ID) => status(403, "{}"),
        _ => ok("{\"ok\":true}"),
    }));
    let client = client(&fake, fast());
    let body = client
        .get("/search", &[])
        .await
        .expect("works with the new id");
    assert_eq!(body, b"{\"ok\":true}");
    let urls = fake.urls();
    assert_eq!(
        urls.iter()
            .filter(|u| *u == "https://soundcloud.com/")
            .count(),
        2
    );
    assert!(urls.last().is_some_and(|u| u.ends_with(ID2)));
}

#[tokio::test]
async fn a_second_refusal_is_final() {
    let fake = Arc::new(FakeHttp::new(|url, _| match url {
        "https://soundcloud.com/" => ok(HOME),
        u if u.ends_with("2-b.js") => ok(format!("client_id:\"{ID}\"")),
        _ => status(401, "{}"),
    }));
    let client = client(&fake, fast());
    assert_eq!(client.get("/me", &[]).await, Err(ClientError::Refused));
    let homes = fake
        .urls()
        .iter()
        .filter(|u| *u == "https://soundcloud.com/")
        .count();
    assert_eq!(homes, 2, "one refresh, never a loop");
}

#[tokio::test]
async fn a_page_with_no_id_gives_no_client_id() {
    let fake = Arc::new(FakeHttp::new(|url, _| {
        if url == "https://soundcloud.com/" {
            ok(HOME)
        } else {
            ok("nothing")
        }
    }));
    let err = client(&fake, fast()).get("/search", &[]).await;
    assert_eq!(err, Err(ClientError::NoClientId));
    assert_eq!(fake.urls().len(), 3, "home and both scripts, no api call");
}

#[tokio::test]
async fn a_429_stops_all_calls_until_the_backoff_ends() {
    let fake = Arc::new(FakeHttp::new(|url, index| {
        let mut answer = site(ID)(url, index);
        if url.contains("/search") && index < 4 {
            answer = Ok(ScAnswer {
                status: 429,
                body: Vec::new(),
                retry_after: Some(Duration::from_hours(1)),
            });
        }
        answer
    }));
    let client = client(&fake, fast());
    assert_eq!(
        client.get("/search", &[]).await,
        Err(ClientError::RateLimited)
    );
    let seen = fake.urls().len();
    assert_eq!(
        client.get("/tracks/1", &[]).await,
        Err(ClientError::RateLimited)
    );
    assert_eq!(
        fake.urls().len(),
        seen,
        "no request while the backoff lasts"
    );
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(
        client.get("/tracks/1", &[]).await.is_ok(),
        "the backoff is capped at max_backoff"
    );
}

#[tokio::test]
async fn a_429_without_retry_after_uses_the_default_backoff() {
    let fake = Arc::new(FakeHttp::new(|url, index| {
        if url.contains("/search") {
            status(429, "")
        } else {
            site(ID)(url, index)
        }
    }));
    let client = client(&fake, fast());
    assert_eq!(
        client.get("/search", &[]).await,
        Err(ClientError::RateLimited)
    );
    assert_eq!(client.get("/x", &[]).await, Err(ClientError::RateLimited));
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(client.get("/x", &[]).await.is_ok());
}

#[tokio::test]
async fn a_server_error_is_retried_twice() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = Arc::clone(&calls);
    let fake = Arc::new(FakeHttp::new(move |url, index| {
        if url.contains("/tracks") {
            let n = seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return if n < 2 {
                status(500, "")
            } else {
                ok("{\"id\":1}")
            };
        }
        site(ID)(url, index)
    }));
    let client = client(&fake, fast());
    assert!(client.get("/tracks/1", &[]).await.is_ok());
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
}

#[tokio::test]
async fn three_server_errors_fail_with_network() {
    let fake = Arc::new(FakeHttp::new(|url, index| {
        if url.contains("/tracks") {
            status(503, "")
        } else {
            site(ID)(url, index)
        }
    }));
    let client = client(&fake, fast());
    assert_eq!(
        client.get("/tracks/1", &[]).await,
        Err(ClientError::Network)
    );
    let api_calls = fake.urls().iter().filter(|u| u.contains("/tracks")).count();
    assert_eq!(api_calls, 3, "the call and two retries, no more");
}

#[tokio::test]
async fn a_missing_page_is_not_found_and_not_retried() {
    let fake = Arc::new(FakeHttp::new(|url, index| {
        if url.contains("/tracks") {
            status(404, "{}")
        } else {
            site(ID)(url, index)
        }
    }));
    let client = client(&fake, fast());
    assert_eq!(
        client.get("/tracks/1", &[]).await,
        Err(ClientError::NotFound)
    );
    assert_eq!(
        fake.urls().iter().filter(|u| u.contains("/tracks")).count(),
        1
    );
}

#[tokio::test]
async fn requests_are_one_at_a_time_and_a_gap_apart() {
    let times = Arc::new(std::sync::Mutex::new(Vec::<Instant>::new()));
    let log = Arc::clone(&times);
    let fake = Arc::new(FakeHttp::new(move |url, index| {
        if url.contains("/x") {
            log.lock().expect("lock").push(Instant::now());
        }
        site(ID)(url, index)
    }));
    let pacing = Pacing {
        min_gap: Duration::from_millis(40),
        ..fast()
    };
    let client = client(&fake, pacing);
    let (a, b, c) = tokio::join!(
        client.get("/x", &[]),
        client.get("/x", &[]),
        client.get("/x", &[])
    );
    assert!(a.is_ok() && b.is_ok() && c.is_ok());
    assert_eq!(fake.max_in_flight(), 1, "never two requests open at once");
    let times = times.lock().expect("lock").clone();
    assert_eq!(times.len(), 3);
    for pair in times.windows(2) {
        // The fake logs when it answers, a little after the start; the gap is still kept.
        assert!(
            pair[1].duration_since(pair[0]) >= Duration::from_millis(35),
            "gap kept"
        );
    }
}

#[tokio::test]
async fn warm_up_finds_the_id_without_an_api_call() {
    let fake = Arc::new(FakeHttp::new(site(ID)));
    let client = client(&fake, fast());
    client.warm_up().await.expect("warm");
    assert_eq!(fake.urls().len(), 2);
    client.warm_up().await.expect("warm again");
    assert_eq!(fake.urls().len(), 2, "known id, no new request");
}

#[tokio::test]
async fn errors_and_debug_output_hold_no_id_or_address() {
    let fake = Arc::new(FakeHttp::new(site(ID)));
    let client = client(&fake, fast());
    client.warm_up().await.expect("warm");
    assert!(!format!("{client:?}").contains(ID));
    for e in [
        ClientError::RateLimited,
        ClientError::NotFound,
        ClientError::Refused,
        ClientError::Network,
        ClientError::Timeout,
        ClientError::Parse,
        ClientError::NoClientId,
    ] {
        let text = e.to_string();
        assert!(
            !text.contains("http") && !text.contains("client_id="),
            "{text}"
        );
    }
}

#[tokio::test]
async fn it_works_over_real_http_on_loopback() {
    // The page must name its script with the server's own address, known only after it starts.
    let base = Arc::new(std::sync::OnceLock::<String>::new());
    let known = Arc::clone(&base);
    let server = TestServer::start(move |path, _| {
        let base = known.get().cloned().unwrap_or_default();
        match path {
            "/" => Response::new(
                200,
                format!(r#"<script src="{base}/assets/a.js"></script>"#),
            ),
            "/assets/a.js" => Response::new(200, format!("client_id:\"{ID}\"")),
            other => Response::new(200, other.as_bytes()),
        }
    });
    let root = server.url("");
    base.set(root.clone()).expect("set once");
    let endpoints = Endpoints {
        site: root.clone(),
        api: root.clone(),
        script_prefix: format!("{root}/assets/"),
    };
    let http = ReqwestHttp::new_for_tests(tokio::runtime::Handle::current()).expect("client");
    let client = ScClient::with(Arc::new(http), endpoints, fast());
    let body = client.get("/search", &[("q", "a")]).await.expect("search");
    assert_eq!(
        String::from_utf8_lossy(&body),
        format!("/search?q=a&client_id={ID}")
    );
    assert_eq!(
        server.paths(),
        ["/", "/assets/a.js", &format!("/search?q=a&client_id={ID}")]
    );
}
