//! The account's writes against a fake HTTP layer: methods, paths, bodies, pacing and failures.
//! No real account or token; every id and the token are invented. The answers are NOT recorded
//! (see `tests/fixtures/README.md`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use nemo_core::secret::Secret;

use super::*;
use crate::client::{Endpoints, Pacing, ScClient};
use crate::fake::{FakeHttp, ok, status};
use crate::fixtures::{PLAYLIST_CREATED, PLAYLIST_WITH_STUBS};
use crate::http::{ScAnswer, ScHttp, ScHttpError, ScMethod};

const ID: &str = "AbCdEfGh0123456789AbCdEfGh012345";
const HOME: &str = r#"<script src="https://a-v2.sndcdn.com/assets/1-a.js"></script>"#;
const TOKEN: &str = "2-293232-7000001-SyntheticToken";
const ME: u64 = 7_000_001;
const LIST: &str = "sc:playlist:7000022";

fn fast() -> Pacing {
    Pacing {
        min_gap: Duration::ZERO,
        retry_delays: [Duration::ZERO, Duration::ZERO],
        default_backoff: Duration::from_millis(40),
        max_backoff: Duration::from_millis(40),
    }
}

/// What the fake answers to writes: a status per call index, a body, or a lost connection.
type Writer = dyn Fn(&str, usize) -> Result<ScAnswer, ScHttpError> + Send + Sync;

struct Rig {
    writes: SoundcloudWrites,
    client: Arc<ScClient>,
    http: Arc<FakeHttp>,
}

fn rig_with(gap: Duration, answer: Arc<Writer>) -> Rig {
    let http = Arc::new(FakeHttp::new(move |url, index| {
        if url == "https://soundcloud.com/" {
            return ok(HOME);
        }
        if url.ends_with("1-a.js") {
            return ok(format!("x={{client_id:\"{ID}\"}}"));
        }
        if url.starts_with("https://api-v2.soundcloud.com/playlists/7000022?") {
            return ok(PLAYLIST_WITH_STUBS);
        }
        answer(url, index)
    }));
    let client = Arc::new(
        ScClient::with(
            Arc::clone(&http) as Arc<dyn ScHttp>,
            Endpoints::production(),
            fast(),
        )
        .with_write_gap(gap),
    );
    client.auth().set(Secret::new(TOKEN.to_owned()));
    client.auth().set_user(Some(ME));
    Rig {
        writes: SoundcloudWrites::new(Arc::clone(&client)),
        client,
        http,
    }
}

fn rig(code: u16) -> Rig {
    rig_with(
        Duration::ZERO,
        Arc::new(move |url, _| {
            if url.starts_with("https://api-v2.soundcloud.com/playlists?") {
                return status(code, PLAYLIST_CREATED);
            }
            status(code, "{}")
        }),
    )
}

/// The requests that were not reads of the home page or its scripts.
fn api_calls(rig: &Rig) -> Vec<(ScMethod, String, Option<String>)> {
    let urls = rig.http.urls();
    rig.http
        .sent()
        .into_iter()
        .zip(urls)
        .filter(|(_, u)| u.starts_with("https://api-v2.soundcloud.com/"))
        .map(|((m, b), u)| (m, u, b))
        .collect()
}

fn track(n: u64) -> TrackId {
    TrackId(format!("sc:{n}"))
}

fn page(id: &str) -> BrowseId {
    BrowseId(id.to_owned())
}

#[tokio::test]
async fn a_like_and_an_unlike_are_one_put_and_one_delete_with_the_token_only_in_the_header() {
    let rig = rig(200);
    rig.writes
        .set_track_liked(&track(5_000_002), true)
        .await
        .expect("liked");
    rig.writes
        .set_track_liked(&track(5_000_002), false)
        .await
        .expect("unliked");
    let calls = api_calls(&rig);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, ScMethod::Put);
    assert_eq!(calls[1].0, ScMethod::Delete);
    for (_, url, body) in &calls {
        assert!(
            url.starts_with("https://api-v2.soundcloud.com/users/7000001/track_likes/5000002?"),
            "{url}"
        );
        assert!(url.contains(&format!("client_id={ID}")));
        assert!(!url.contains(TOKEN) && !url.contains("oauth"));
        assert_eq!(body, &None);
    }
    let tokens: Vec<_> = rig
        .http
        .tokens()
        .into_iter()
        .zip(rig.http.urls())
        .filter(|(_, u)| u.contains("track_likes"))
        .map(|(t, _)| t)
        .collect();
    assert_eq!(tokens, vec![Some(TOKEN.to_owned()); 2]);
}

#[tokio::test]
async fn any_2xx_is_a_confirmed_write_and_nothing_else_is() {
    for code in [200, 201, 202, 204] {
        let rig = rig(code);
        assert_eq!(
            rig.writes.set_track_liked(&track(1), true).await,
            Ok(()),
            "{code}"
        );
    }
    for (code, error) in [
        (401, WriteError::Refused),
        (403, WriteError::Refused),
        (400, WriteError::Refused),
        (404, WriteError::NotFound),
        (410, WriteError::NotFound),
        (429, WriteError::RateLimited),
        (500, WriteError::Network),
        (503, WriteError::Network),
    ] {
        let rig = rig(code);
        assert_eq!(
            rig.writes.set_track_liked(&track(1), true).await,
            Err(error),
            "{code}"
        );
        assert_eq!(
            api_calls(&rig).len(),
            1,
            "{code}: sent once, never repeated"
        );
    }
}

#[tokio::test]
async fn a_lost_connection_and_a_timeout_are_reported_and_not_repeated() {
    for (failure, error) in [
        (ScHttpError::Failed, WriteError::Network),
        (ScHttpError::Timeout, WriteError::Network),
    ] {
        let rig = rig_with(Duration::ZERO, Arc::new(move |_, _| Err(failure)));
        assert_eq!(
            rig.writes.set_track_liked(&track(1), true).await,
            Err(error)
        );
        assert_eq!(
            api_calls(&rig).len(),
            1,
            "the change may have happened: no repeat"
        );
    }
}

#[tokio::test]
async fn a_429_stops_the_next_write_before_anything_is_sent() {
    let rig = rig(429);
    assert_eq!(
        rig.writes.set_track_liked(&track(1), true).await,
        Err(WriteError::RateLimited)
    );
    let before = rig.http.urls().len();
    assert_eq!(
        rig.writes.set_track_liked(&track(2), true).await,
        Err(WriteError::RateLimited)
    );
    assert_eq!(
        rig.http.urls().len(),
        before,
        "nothing went out during the backoff"
    );
}

#[tokio::test]
async fn a_refusal_looks_the_client_id_up_again_next_time_but_does_not_end_the_session() {
    let rig = rig(403);
    let _ = rig.writes.set_track_liked(&track(1), true).await;
    assert!(rig.client.auth().is_signed_in());
    let homes = |rig: &Rig| {
        rig.http
            .urls()
            .iter()
            .filter(|u| *u == "https://soundcloud.com/")
            .count()
    };
    assert_eq!(homes(&rig), 1);
    let _ = rig.writes.set_track_liked(&track(1), true).await;
    assert_eq!(homes(&rig), 2, "a fresh id for the next call");
}

#[tokio::test]
async fn nobody_signed_in_means_no_request_at_all() {
    let rig = rig(200);
    rig.client.auth().clear();
    for result in [
        rig.writes.set_track_liked(&track(1), true).await,
        rig.writes.unfollow(&page("sc:user:8000004")).await,
        rig.writes.rename_playlist(&page(LIST), "x").await,
    ] {
        assert_eq!(result, Err(WriteError::NotSignedIn));
    }
    assert!(rig.http.urls().is_empty());
}

#[tokio::test]
async fn off_means_nothing_is_sent() {
    let rig = rig(200);
    rig.client.set_enabled(false);
    assert_eq!(
        rig.writes.set_track_liked(&track(1), true).await,
        Err(WriteError::Off)
    );
    assert!(api_calls(&rig).is_empty());
}

#[tokio::test]
async fn ids_and_titles_that_are_not_soundcloud_ones_send_nothing() {
    let rig = rig(200);
    assert_eq!(
        rig.writes
            .set_track_liked(&TrackId("yt:abc".to_owned()), true)
            .await,
        Err(WriteError::Invalid)
    );
    assert_eq!(
        rig.writes
            .set_track_liked(&TrackId("sc:12/../me".to_owned()), true)
            .await,
        Err(WriteError::Invalid)
    );
    assert_eq!(
        rig.writes.unfollow(&page(LIST)).await,
        Err(WriteError::Invalid),
        "a playlist is not a user"
    );
    assert_eq!(
        rig.writes.rename_playlist(&page("sc:user:7"), "x").await,
        Err(WriteError::Invalid)
    );
    assert_eq!(
        rig.writes.rename_playlist(&page(LIST), "   ").await,
        Err(WriteError::Invalid)
    );
    assert_eq!(
        rig.writes
            .create_playlist(&"a".repeat(101), None)
            .await
            .map(|_| ()),
        Err(WriteError::Invalid)
    );
    assert!(rig.http.urls().is_empty());
}

#[tokio::test]
async fn unfollow_deletes_the_following_and_playlists_can_be_liked() {
    let rig = rig(200);
    rig.writes
        .unfollow(&page("sc:user:8000004"))
        .await
        .expect("unfollowed");
    rig.writes
        .set_playlist_liked(&page(LIST), true)
        .await
        .expect("liked");
    let calls = api_calls(&rig);
    assert_eq!(calls[0].0, ScMethod::Delete);
    assert!(calls[0].1.contains("/me/followings/8000004?"));
    assert_eq!(calls[1].0, ScMethod::Put);
    assert!(
        calls[1]
            .1
            .contains("/users/7000001/playlist_likes/7000022?")
    );
}

#[tokio::test]
async fn creating_a_playlist_posts_it_private_and_opens_the_new_page() {
    let rig = rig(201);
    let made = rig
        .writes
        .create_playlist("  Night drive ", Some(&track(7_000_001)))
        .await
        .expect("created");
    assert_eq!(made.id, page("sc:playlist:7000030"));
    let calls = api_calls(&rig);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, ScMethod::Post);
    assert!(
        calls[0]
            .1
            .starts_with("https://api-v2.soundcloud.com/playlists?")
    );
    let body: serde_json::Value =
        serde_json::from_str(calls[0].2.as_deref().expect("body")).expect("json");
    assert_eq!(body["playlist"]["title"], "Night drive");
    assert_eq!(body["playlist"]["sharing"], "private");
    assert_eq!(
        body["playlist"]["tracks"],
        serde_json::json!([7_000_001_u64])
    );
}

#[tokio::test]
async fn a_created_answer_without_an_id_is_an_error_the_screen_can_word() {
    let rig = rig_with(Duration::ZERO, Arc::new(|_, _| status(201, "{}")));
    assert_eq!(
        rig.writes.create_playlist("x", None).await.map(|_| ()),
        Err(WriteError::Network)
    );
}

#[tokio::test]
async fn adding_a_track_sends_the_whole_list_with_the_new_one_last_and_keeps_every_stub() {
    let rig = rig(200);
    rig.writes
        .add_to_playlist(&page(LIST), &track(5_000_009))
        .await
        .expect("added");
    let calls = api_calls(&rig);
    assert_eq!(calls[0].0, ScMethod::Get);
    assert_eq!(calls[1].0, ScMethod::Put);
    assert!(calls[1].1.contains("/playlists/7000022?"));
    let body: serde_json::Value =
        serde_json::from_str(calls[1].2.as_deref().expect("body")).expect("json");
    assert_eq!(
        body["playlist"]["tracks"],
        serde_json::json!([
            7_000_001_u64,
            7_000_003,
            7_000_023,
            7_000_024,
            7_000_025,
            5_000_009
        ])
    );
    assert_eq!(
        body["playlist"].as_object().map(serde_json::Map::len),
        Some(1),
        "tracks only"
    );
}

#[tokio::test]
async fn a_track_already_in_the_playlist_or_not_in_it_sends_no_write() {
    let rig = rig(200);
    assert_eq!(
        rig.writes
            .add_to_playlist(&page(LIST), &track(7_000_003))
            .await,
        Err(WriteError::Unchanged)
    );
    assert_eq!(
        rig.writes
            .remove_from_playlist(&page(LIST), &track(5_000_009))
            .await,
        Err(WriteError::Unchanged)
    );
    assert!(api_calls(&rig).iter().all(|(m, _, _)| *m == ScMethod::Get));
}

#[tokio::test]
async fn removing_a_track_sends_the_list_without_it() {
    let rig = rig(200);
    rig.writes
        .remove_from_playlist(&page(LIST), &track(7_000_023))
        .await
        .expect("removed");
    let calls = api_calls(&rig);
    let body: serde_json::Value =
        serde_json::from_str(calls[1].2.as_deref().expect("body")).expect("json");
    assert_eq!(
        body["playlist"]["tracks"],
        serde_json::json!([7_000_001_u64, 7_000_003, 7_000_024, 7_000_025])
    );
}

#[tokio::test]
async fn renaming_sends_only_the_title() {
    let rig = rig(200);
    rig.writes
        .rename_playlist(&page(LIST), " New name ")
        .await
        .expect("renamed");
    let calls = api_calls(&rig);
    assert_eq!(calls[0].0, ScMethod::Put);
    let body: serde_json::Value =
        serde_json::from_str(calls[0].2.as_deref().expect("body")).expect("json");
    assert_eq!(body, serde_json::json!({"playlist": {"title": "New name"}}));
}

#[tokio::test]
async fn writes_are_paced_and_never_overlap() {
    let gap = Duration::from_millis(120);
    let rig = Arc::new(rig_with(gap, Arc::new(|_, _| status(200, "{}"))));
    let started = Instant::now();
    let (first, second) = (track(1), track(2));
    let (a, b) = tokio::join!(
        rig.writes.set_track_liked(&first, true),
        rig.writes.set_track_liked(&second, true)
    );
    assert!(a.is_ok() && b.is_ok());
    assert!(
        started.elapsed() >= gap,
        "the second write waited for the gap"
    );
    assert_eq!(rig.http.max_in_flight(), 1, "one request at a time");
}

#[tokio::test]
async fn error_texts_never_hold_the_token_or_an_id() {
    for e in [
        WriteError::NotSignedIn,
        WriteError::Refused,
        WriteError::NotFound,
        WriteError::Network,
    ] {
        let text = e.to_string();
        assert!(!text.contains(TOKEN) && !text.contains("7000001"), "{text}");
    }
    assert!(!format!("{:?}", SoundcloudWrites::new(rig(200).client)).contains(TOKEN));
}
