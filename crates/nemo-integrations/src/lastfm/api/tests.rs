//! Tests for the signed Last.fm client, against a local test server only. The key, secret and
//! session key below are made-up test values.

use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};

use nemo_core::secret::Secret;

use super::{ApiError, BASE_URL, LastFmApi, NowPlaying, ScrobbleOutcome, read_answer, sign};
use crate::lastfm::queue::{BATCH, NewScrobble};
use crate::lastfm::test_server::{Request, TestServer};

const KEY: &str = "test-api-key-0123";
const SECRET: &str = "test-api-secret-4567";
const SESSION: &str = "test-session-key-89ab";

fn api(server: &TestServer) -> LastFmApi {
    LastFmApi::new(
        &server.url(),
        Secret::new(KEY.to_owned()),
        Secret::new(SECRET.to_owned()),
    )
    .expect("client")
}

fn session() -> Secret<String> {
    Secret::new(SESSION.to_owned())
}

fn scrobble(i: usize) -> NewScrobble {
    NewScrobble {
        ts: 1_700_000_000 + i64::try_from(i).expect("small"),
        artist: format!("Artist {i}"),
        track: format!("Track {i}"),
        album: i.is_multiple_of(2).then(|| format!("Album {i}")),
        duration_s: Some(200),
    }
}

/// Check the signature the way Last.fm does: re-sign every received field except `api_sig`.
fn assert_signed(req: &Request) {
    let fields: Vec<(&str, &str)> = req
        .form
        .iter()
        .filter(|(n, _)| n != "api_sig")
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();
    assert_eq!(req.field("api_sig"), Some(sign(&fields, SECRET).as_str()));
}

#[test]
fn sign_matches_an_independent_md5() {
    // Expected value from `md5sum` and Python's `hashlib` over the string
    // "api_keyKEYartist[10]Đen Vâuartist[2]Sigur Rósmethodtrack.scrobbleskSK\
    //  track[10]Bài này chill phếtSECRET" (UTF-8): sorted by name in byte order, so
    // `artist[10]` comes before `artist[2]`, and `format` is not signed.
    let params = [
        ("method", "track.scrobble"),
        ("artist[2]", "Sigur Rós"),
        ("track[10]", "Bài này chill phết"),
        ("format", "json"),
        ("artist[10]", "Đen Vâu"),
        ("sk", "SK"),
        ("api_key", "KEY"),
    ];
    assert_eq!(sign(&params, "SECRET"), "cf3664a44ba717b57e2fb3c19f2a73d6");
}

#[test]
fn sign_matches_the_desktop_auth_example() {
    // Shaped like the example of https://www.last.fm/api/desktopauth (placeholder key and
    // token, secret "ilovecher"); expected value from `md5sum`.
    let params = [
        ("api_key", "xxxxxxxxxx"),
        ("method", "auth.getSession"),
        ("token", "yyyyyy"),
    ];
    assert_eq!(
        sign(&params, "ilovecher"),
        "b87d61da3cda91a8b6746c4aef55d6f8"
    );
}

#[test]
fn only_https_is_accepted_outside_tests() {
    let key = || Secret::new(KEY.to_owned());
    assert!(BASE_URL.starts_with("https://"));
    let host = reqwest::Url::parse(BASE_URL).expect("url");
    assert_eq!(host.host_str(), crate::info::LASTFM.hosts.first().copied());
    assert!(LastFmApi::new(BASE_URL, key(), key()).is_ok());
    for bad in [
        "http://ws.audioscrobbler.com/2.0/",
        "ftp://127.0.0.1/",
        "http://localhost/",
    ] {
        assert_eq!(
            LastFmApi::new(bad, key(), key()).err(),
            Some(ApiError::Insecure),
            "{bad}"
        );
    }
    assert_eq!(
        LastFmApi::new("not a url", key(), key()).err(),
        Some(ApiError::BadUrl)
    );
}

#[test]
fn debug_output_hides_the_key_and_secret() {
    let api = LastFmApi::new(
        BASE_URL,
        Secret::new(KEY.to_owned()),
        Secret::new(SECRET.to_owned()),
    )
    .expect("client");
    let text = format!("{api:?}");
    assert!(!text.contains(KEY) && !text.contains(SECRET), "{text}");
}

#[tokio::test]
async fn now_playing_is_one_signed_post_with_the_track() {
    let server = TestServer::start(|_, _| (200, r#"{"nowplaying":{}}"#.to_owned()));
    let now = NowPlaying {
        artist: "Sigur Rós".to_owned(),
        track: "Hoppípolla & <Co>".to_owned(),
        album: Some("Takk...".to_owned()),
        duration_s: Some(268),
    };
    api(&server)
        .update_now_playing(&session(), &now)
        .await
        .expect("sent");

    let reqs = server.requests();
    assert_eq!(reqs.len(), 1);
    let req = &reqs[0];
    assert_eq!(req.verb, "POST");
    assert_eq!(req.path, "/2.0/", "nothing travels in the URL");
    assert!(
        req.content_type
            .starts_with("application/x-www-form-urlencoded")
    );
    for (name, value) in [
        ("method", "track.updateNowPlaying"),
        ("api_key", KEY),
        ("sk", SESSION),
        ("artist", "Sigur Rós"),
        ("track", "Hoppípolla & <Co>"),
        ("album", "Takk..."),
        ("duration", "268"),
        ("format", "json"),
    ] {
        assert_eq!(req.field(name), Some(value), "{name}");
    }
    assert_eq!(req.field("api_secret"), None);
    assert!(
        req.form.iter().all(|(_, v)| v != SECRET),
        "secret never sent"
    );
    assert_signed(req);
}

#[tokio::test]
async fn a_batch_of_fifty_keeps_its_order_and_reads_the_counts() {
    let server = TestServer::start(|_, _| {
        (
            200,
            r#"{"scrobbles":{"scrobble":[],"@attr":{"accepted":48,"ignored":"2"}}}"#.to_owned(),
        )
    });
    let batch: Vec<NewScrobble> = (0..BATCH).map(scrobble).collect();
    let outcome = api(&server)
        .scrobble(&session(), &batch)
        .await
        .expect("sent");
    assert_eq!(
        outcome,
        ScrobbleOutcome {
            accepted: 48,
            ignored: 2
        }
    );

    let reqs = server.requests();
    assert_eq!(reqs.len(), 1);
    let req = &reqs[0];
    assert_eq!(req.field("method"), Some("track.scrobble"));
    for (i, s) in batch.iter().enumerate() {
        assert_eq!(req.field(&format!("artist[{i}]")), Some(s.artist.as_str()));
        assert_eq!(req.field(&format!("track[{i}]")), Some(s.track.as_str()));
        assert_eq!(
            req.field(&format!("timestamp[{i}]")),
            Some(s.ts.to_string().as_str())
        );
        assert_eq!(req.field(&format!("album[{i}]")), s.album.as_deref());
    }
    assert_eq!(req.field("artist[50]"), None);
    assert_signed(req);
}

#[tokio::test]
async fn an_empty_or_oversized_batch_sends_nothing() {
    let server = TestServer::start(|_, _| (200, "{}".to_owned()));
    let client = api(&server);
    assert_eq!(
        client.scrobble(&session(), &[]).await,
        Ok(ScrobbleOutcome::default())
    );
    let too_many: Vec<NewScrobble> = (0..=BATCH).map(scrobble).collect();
    assert_eq!(
        client.scrobble(&session(), &too_many).await,
        Err(ApiError::BatchTooLarge)
    );
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn a_lastfm_error_in_the_body_wins_over_the_http_status() {
    // Last.fm answers an invalid session with HTTP 403 and `{"error": 9}`.
    let server = TestServer::start(|_, i| match i {
        0 => (
            403,
            r#"{"error":9,"message":"Invalid session key"}"#.to_owned(),
        ),
        1 => (200, r#"{"error":"16","message":"Try again"}"#.to_owned()),
        2 => (503, "<html>down</html>".to_owned()),
        _ => (200, "not json".to_owned()),
    });
    let client = api(&server);
    let batch = [scrobble(1)];
    assert_eq!(
        client.scrobble(&session(), &batch).await,
        Err(ApiError::Api(9))
    );
    assert_eq!(
        client.scrobble(&session(), &batch).await,
        Err(ApiError::Api(16))
    );
    assert!(matches!(
        client.scrobble(&session(), &batch).await,
        Err(ApiError::Network(_))
    ));
    assert_eq!(
        client.scrobble(&session(), &batch).await,
        Err(ApiError::Parse)
    );
}

#[test]
fn answers_are_read_whatever_the_status() {
    assert_eq!(read_answer(500, br#"{"error":11}"#), Err(ApiError::Api(11)));
    assert!(read_answer(200, br#"{"nowplaying":{}}"#).is_ok());
    assert!(matches!(
        read_answer(404, br#"{"page":"missing"}"#),
        Err(ApiError::Network(_))
    ));
    assert_eq!(read_answer(200, b""), Err(ApiError::Parse));
}

#[tokio::test]
async fn a_scrobble_answer_without_counts_is_not_understood() {
    let server = TestServer::start(|_, _| (200, r#"{"scrobbles":{}}"#.to_owned()));
    assert_eq!(
        api(&server).scrobble(&session(), &[scrobble(0)]).await,
        Err(ApiError::Parse)
    );
}

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Every `tracing` line of this test binary at every level. A global subscriber, not a scoped
/// one: `tracing` caches whether a log line is wanted per call site, and a call site first hit
/// on another test thread would be cached as unwanted while only a scoped subscriber exists.
fn capture() -> &'static Capture {
    static CAPTURE: OnceLock<Capture> = OnceLock::new();
    CAPTURE.get_or_init(|| {
        let capture = Capture::default();
        let writer = capture.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::set_global_default(subscriber).expect("first and only subscriber");
        capture
    })
}

#[tokio::test]
async fn logs_hold_no_key_secret_session_key_or_title() {
    let capture = capture();
    let server = TestServer::start(|_, i| match i {
        0 => (200, r#"{"nowplaying":{}}"#.to_owned()),
        _ => (
            403,
            r#"{"error":9,"message":"Invalid session key"}"#.to_owned(),
        ),
    });
    let client = api(&server);
    let now = NowPlaying {
        artist: "Secret Artist".to_owned(),
        track: "Secret Title".to_owned(),
        album: Some("Secret Album".to_owned()),
        duration_s: None,
    };
    client
        .update_now_playing(&session(), &now)
        .await
        .expect("sent");
    let failed = client
        .scrobble(
            &session(),
            &[NewScrobble {
                ts: 1,
                artist: now.artist.clone(),
                track: now.track.clone(),
                album: now.album.clone(),
                duration_s: None,
            }],
        )
        .await;
    assert_eq!(failed, Err(ApiError::Api(9)));
    let error_text = failed.err().map(|e| e.to_string()).unwrap_or_default();

    let logs = String::from_utf8(capture.0.lock().expect("lock").clone()).expect("utf-8");
    assert!(
        logs.contains("Last.fm answered"),
        "the capture works: {logs}"
    );
    for leak in [
        KEY,
        SECRET,
        SESSION,
        "Secret Artist",
        "Secret Title",
        "Secret Album",
        "api_sig",
    ] {
        assert!(!logs.contains(leak), "{leak} in logs: {logs}");
        assert!(!error_text.contains(leak), "{leak} in error: {error_text}");
    }
}
