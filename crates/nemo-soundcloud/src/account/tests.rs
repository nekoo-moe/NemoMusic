//! The account against a fake credential store, a fake sign-in window and a fake HTTP layer. No
//! real token, name or id appears anywhere; the token below is invented.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use nemo_core::account::WipeError;
use nemo_core::secret_store::MemoryStore;

use super::*;
use crate::client::{Endpoints, Pacing};
use crate::fake::{FakeHttp, ok, status};
use crate::fixtures::ME;
use crate::http::{ScAnswer, ScHttp, ScHttpError};

const TOKEN: &str = "2-293232-7000001-SyntheticToken";
const OTHER_TOKEN: &str = "2-293232-7000002-OtherSynthetic";
const ID: &str = "AbCdEfGh0123456789AbCdEfGh012345";
const HOME: &str = r#"<script src="https://a-v2.sndcdn.com/assets/1-a.js"></script>"#;

fn fast() -> Pacing {
    Pacing {
        min_gap: Duration::ZERO,
        retry_delays: [Duration::ZERO, Duration::ZERO],
        default_backoff: Duration::from_millis(40),
        max_backoff: Duration::from_millis(40),
    }
}

/// What `GET /me` answers: the fixture, or a status.
#[derive(Clone, Copy)]
enum MeAnswer {
    User,
    OtherUser,
    Status(u16),
}

struct Site {
    me: Mutex<MeAnswer>,
}

impl Site {
    fn answer(&self, url: &str) -> Result<ScAnswer, ScHttpError> {
        if url == "https://soundcloud.com/" {
            return ok(HOME);
        }
        if url.ends_with("1-a.js") {
            return ok(format!("x={{client_id:\"{ID}\"}}"));
        }
        if url.starts_with("https://api-v2.soundcloud.com/me?") {
            return match *self.me.lock().expect("lock") {
                MeAnswer::User => ok(ME),
                MeAnswer::OtherUser => ok(ME.replace("7000001", "7000002")),
                MeAnswer::Status(code) => status(code, "{}"),
            };
        }
        ok("{}")
    }
}

/// A window that answers once with `result`.
struct FakeWindow(Mutex<Option<Result<Secret<String>, SourceAccountError>>>);

impl FakeWindow {
    fn gives(token: &str) -> Arc<Self> {
        Arc::new(Self(Mutex::new(Some(Ok(Secret::new(token.to_owned()))))))
    }

    fn fails(error: SourceAccountError) -> Arc<Self> {
        Arc::new(Self(Mutex::new(Some(Err(error)))))
    }
}

impl TokenSource for FakeWindow {
    fn token(&self) -> BoxFuture<'_, Result<Secret<String>, SourceAccountError>> {
        let next = self.0.lock().expect("lock").take();
        Box::pin(async move { next.unwrap_or(Err(SourceAccountError::Cancelled)) })
    }
}

struct Counter {
    name: &'static str,
    runs: Arc<AtomicUsize>,
    fails: bool,
}

impl SignOutWipe for Counter {
    fn name(&self) -> &'static str {
        self.name
    }

    fn wipe(&self, _scope: WipeScope) -> Result<usize, WipeError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        if self.fails {
            Err(WipeError("cannot delete"))
        } else {
            Ok(2)
        }
    }
}

struct Rig {
    account: ScAccount,
    client: Arc<ScClient>,
    http: Arc<FakeHttp>,
    store: Arc<MemoryStore>,
    site: Arc<Site>,
    files: AccountFiles,
    dir: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nemo-sc-account-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

fn rig(name: &str, window: Arc<FakeWindow>) -> Rig {
    rig_with(name, window, Arc::new(MemoryStore::default()), None)
}

/// A rig over an existing store and folder (a restart), or a new folder.
fn rig_with(
    name: &str,
    window: Arc<FakeWindow>,
    store: Arc<MemoryStore>,
    existing: Option<PathBuf>,
) -> Rig {
    let dir = existing.unwrap_or_else(|| dir(name));
    let site = Arc::new(Site {
        me: Mutex::new(MeAnswer::User),
    });
    let routes = Arc::clone(&site);
    let http = Arc::new(FakeHttp::new(move |url, _| routes.answer(url)));
    let client = Arc::new(ScClient::with(
        Arc::clone(&http) as Arc<dyn ScHttp>,
        Endpoints::production(),
        fast(),
    ));
    let files = AccountFiles {
        webview_dir: dir.join("webview"),
        cache_file: dir.join("account").join("soundcloud.json"),
    };
    let account = ScAccount::new(
        Arc::clone(&client),
        Arc::clone(&store) as Arc<dyn SecretStore>,
        window,
        files.clone(),
        Handle::current(),
    );
    Rig {
        account,
        client,
        http,
        store,
        site,
        files,
        dir,
    }
}

fn me_calls(rig: &Rig) -> usize {
    rig.http
        .urls()
        .iter()
        .filter(|u| u.starts_with("https://api-v2.soundcloud.com/me?"))
        .count()
}

fn stored(rig: &Rig) -> Option<String> {
    rig.store
        .get(TOKEN_KEY)
        .expect("get")
        .map(|b| String::from_utf8(b).expect("utf8"))
}

fn counter(rig: &Rig, name: &'static str, fails: bool) -> Arc<AtomicUsize> {
    let runs = Arc::new(AtomicUsize::new(0));
    rig.account.register_wipe(Arc::new(Counter {
        name,
        runs: Arc::clone(&runs),
        fails,
    }));
    runs
}

fn make_webview_dir(rig: &Rig) {
    std::fs::create_dir_all(rig.files.webview_dir.join("Default")).expect("dir");
    std::fs::write(rig.files.webview_dir.join("Default").join("Cookies"), "x").expect("file");
}

#[tokio::test]
async fn the_window_signs_in_and_the_token_goes_only_to_the_store_and_the_api() {
    let rig = rig("window", FakeWindow::gives(TOKEN));
    assert_eq!(rig.account.state(), SourceAccountState::SignedOut);
    let info = rig.account.sign_in().await.expect("signed in");
    assert_eq!(info.name, "Test Person");
    assert_eq!(info.avatar.len(), 3, "the sizes of the avatar");
    assert_eq!(rig.account.state(), SourceAccountState::SignedIn(info));
    assert_eq!(stored(&rig).as_deref(), Some(TOKEN));
    assert_eq!(rig.account.user_id(), Some(7_000_001));
    // One check, on the API host, with the token in the header and nowhere in an address.
    let calls = rig.http.urls();
    assert!(
        calls
            .iter()
            .all(|u| !u.contains(TOKEN) && !u.contains("oauth"))
    );
    let me: Vec<_> = rig
        .http
        .tokens()
        .into_iter()
        .zip(&calls)
        .filter(|(_, u)| u.starts_with("https://api-v2.soundcloud.com/me?"))
        .collect();
    assert_eq!(me.len(), 1);
    assert_eq!(me[0].0.as_deref(), Some(TOKEN));
    // The site and script requests never carried it.
    for (token, url) in rig.http.tokens().into_iter().zip(&calls) {
        if !url.starts_with("https://api-v2.soundcloud.com/") {
            assert_eq!(token, None, "{url}");
        }
    }
}

#[tokio::test]
async fn only_the_name_and_avatar_are_cached_on_disk() {
    let rig = rig("cache", FakeWindow::gives(TOKEN));
    rig.account.sign_in().await.expect("signed in");
    let text = std::fs::read_to_string(&rig.files.cache_file).expect("cache");
    let value: serde_json::Value = serde_json::from_str(&text).expect("json");
    let keys: Vec<&str> = value
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys.len(), 3, "{keys:?}");
    assert!(
        keys.iter()
            .all(|k| ["id", "username", "avatar_url"].contains(k))
    );
    assert!(!text.contains(TOKEN) && !text.contains("followers"));
}

#[tokio::test]
async fn a_pasted_cookie_pair_signs_in_the_same_way() {
    let rig = rig("paste", FakeWindow::fails(SourceAccountError::Cancelled));
    let pasted = Secret::new(format!(" oauth_token={TOKEN} "));
    rig.account
        .sign_in_with_token(pasted)
        .await
        .expect("signed in");
    assert_eq!(stored(&rig).as_deref(), Some(TOKEN));
}

#[tokio::test]
async fn a_value_that_is_not_shaped_like_a_token_is_refused_without_a_request() {
    let rig = rig("shape", FakeWindow::fails(SourceAccountError::Cancelled));
    for bad in ["", "short", "two words in it", "a;b=c-1234567"] {
        let result = rig
            .account
            .sign_in_with_token(Secret::new(bad.to_owned()))
            .await;
        assert_eq!(result, Err(SourceAccountError::Rejected), "{bad:?}");
    }
    assert!(rig.http.urls().is_empty());
    assert_eq!(rig.account.state(), SourceAccountState::SignedOut);
    assert!(rig.store.is_empty());
}

#[tokio::test]
async fn a_rejected_token_leaves_nothing_behind_and_costs_two_checks() {
    let rig = rig("rejected", FakeWindow::gives(TOKEN));
    make_webview_dir(&rig);
    *rig.site.me.lock().expect("lock") = MeAnswer::Status(401);
    let error = rig.account.sign_in().await.expect_err("rejected");
    assert_eq!(error, SourceAccountError::Rejected);
    assert!(!error.to_string().contains(TOKEN));
    assert_eq!(
        me_calls(&rig),
        2,
        "one check and one with a fresh client id"
    );
    assert_eq!(rig.account.state(), SourceAccountState::SignedOut);
    assert!(rig.store.is_empty());
    assert!(!rig.client.auth().is_signed_in());
    assert!(!rig.files.cache_file.exists());
    assert!(!rig.files.webview_dir.exists(), "the next try starts clean");
}

#[tokio::test]
async fn a_failed_check_keeps_the_earlier_state() {
    let rig = rig("offline", FakeWindow::gives(TOKEN));
    *rig.site.me.lock().expect("lock") = MeAnswer::Status(500);
    let error = rig.account.sign_in().await.expect_err("fails");
    assert!(matches!(error, SourceAccountError::Network(_)));
    assert_eq!(rig.account.state(), SourceAccountState::SignedOut);
    assert!(rig.store.is_empty());
}

#[tokio::test]
async fn a_closed_window_is_cancelled_and_changes_nothing() {
    let rig = rig("closed", FakeWindow::fails(SourceAccountError::Cancelled));
    assert_eq!(
        rig.account.sign_in().await,
        Err(SourceAccountError::Cancelled)
    );
    assert_eq!(rig.account.state(), SourceAccountState::SignedOut);
    assert!(rig.http.urls().is_empty());
}

#[tokio::test]
async fn a_store_that_refuses_the_write_keeps_no_session() {
    let rig = rig("storage", FakeWindow::gives(TOKEN));
    rig.store.fail_writes(true);
    let error = rig.account.sign_in().await.expect_err("storage");
    assert!(matches!(error, SourceAccountError::Storage(_)));
    assert!(!error.to_string().contains(TOKEN));
    assert!(!rig.client.auth().is_signed_in(), "no token is sent");
    assert_eq!(rig.account.state(), SourceAccountState::SignedOut);
    assert!(!rig.files.cache_file.exists());
}

#[tokio::test]
async fn a_restart_signs_in_from_the_store_and_the_cache_without_a_request() {
    let first = rig("restart", FakeWindow::gives(TOKEN));
    first.account.sign_in().await.expect("signed in");
    let again = rig_with(
        "restart",
        FakeWindow::fails(SourceAccountError::Cancelled),
        Arc::clone(&first.store),
        Some(first.dir.clone()),
    );
    assert!(matches!(
        again.account.state(),
        SourceAccountState::SignedIn(_)
    ));
    assert!(again.client.auth().is_signed_in());
    assert_eq!(again.account.user_id(), Some(7_000_001));
    assert!(again.http.urls().is_empty(), "no request at start");
}

#[tokio::test]
async fn a_saved_token_without_a_cache_looks_the_name_up_in_the_background() {
    let first = rig("nocache", FakeWindow::gives(TOKEN));
    first.account.sign_in().await.expect("signed in");
    std::fs::remove_file(&first.files.cache_file).expect("remove cache");
    let again = rig_with(
        "nocache",
        FakeWindow::fails(SourceAccountError::Cancelled),
        Arc::clone(&first.store),
        Some(first.dir.clone()),
    );
    let started = std::time::Instant::now();
    while !matches!(again.account.state(), SourceAccountState::SignedIn(_))
        && started.elapsed() < Duration::from_secs(5)
    {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    match again.account.state() {
        SourceAccountState::SignedIn(info) => assert_eq!(info.name, "Test Person"),
        other => panic!("not signed in: {other:?}"),
    }
}

#[tokio::test]
async fn sign_out_removes_the_token_the_cache_the_profile_and_runs_every_step() {
    let rig = rig("signout", FakeWindow::gives(TOKEN));
    let good = counter(&rig, "first", false);
    let bad = counter(&rig, "second", true);
    let last = counter(&rig, "third", false);
    rig.account.sign_in().await.expect("signed in");
    make_webview_dir(&rig);
    let report = rig.account.sign_out().await.expect("signed out");
    assert_eq!(rig.account.state(), SourceAccountState::SignedOut);
    assert!(rig.store.is_empty());
    assert!(!rig.files.cache_file.exists() && !rig.files.webview_dir.exists());
    assert_eq!(report.credentials_removed, 1);
    assert_eq!(report.files_removed, 2);
    assert_eq!(report.items_removed, 4);
    assert_eq!(report.failed_steps, vec!["second"]);
    for runs in [good, bad, last] {
        assert_eq!(runs.load(Ordering::SeqCst), 1, "every step ran, once");
    }
    assert_eq!(rig.account.user_id(), None);
    // Later calls carry no token.
    rig.client
        .get("/tracks/7000009", &[])
        .await
        .expect("public call");
    assert_eq!(rig.http.tokens().last(), Some(&None));
}

#[tokio::test]
async fn sign_out_reports_a_store_that_cannot_delete_but_still_signs_out_here() {
    struct Stuck(MemoryStore);
    impl SecretStore for Stuck {
        fn set(&self, k: &str, v: &[u8]) -> Result<(), nemo_core::secret_store::StoreError> {
            self.0.set(k, v)
        }
        fn get(&self, k: &str) -> Result<Option<Vec<u8>>, nemo_core::secret_store::StoreError> {
            self.0.get(k)
        }
        fn delete(&self, _: &str) -> Result<bool, nemo_core::secret_store::StoreError> {
            Err(nemo_core::secret_store::StoreError(
                "the store refused the delete",
            ))
        }
    }
    let rig = rig("stuck", FakeWindow::gives(TOKEN));
    let stuck: Arc<dyn SecretStore> = Arc::new(Stuck(MemoryStore::default()));
    let account = ScAccount::new(
        Arc::clone(&rig.client),
        stuck,
        FakeWindow::gives(TOKEN),
        rig.files.clone(),
        Handle::current(),
    );
    account.sign_in().await.expect("signed in");
    let error = account.sign_out().await.expect_err("store");
    assert_eq!(
        error,
        SourceAccountError::Storage("the store refused the delete".to_owned())
    );
    assert_eq!(account.state(), SourceAccountState::SignedOut);
    assert!(
        !rig.client.auth().is_signed_in(),
        "the token is not sent again"
    );
}

#[tokio::test]
async fn a_session_soundcloud_ends_expires_once_without_a_retry_storm() {
    let rig = rig("expired", FakeWindow::gives(TOKEN));
    let runs = counter(&rig, "caches", false);
    rig.account.sign_in().await.expect("signed in");
    make_webview_dir(&rig);
    let before = rig.http.urls().len();
    // Every API call is refused with 401 from now on.
    *rig.site.me.lock().expect("lock") = MeAnswer::Status(401);
    let error = rig.client.get("/me", &[]).await.expect_err("ended");
    assert_eq!(error, ClientError::SessionEnded);
    assert_eq!(rig.account.state(), SourceAccountState::Expired);
    assert!(rig.store.is_empty(), "the token is gone from this computer");
    assert!(!rig.files.cache_file.exists() && !rig.files.webview_dir.exists());
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    let storm = rig.http.urls().len() - before;
    assert!(
        storm <= 4,
        "the call, a fresh client id and one repeat: {storm}"
    );
    // The next call is anonymous and starts no new round.
    let _ = rig.client.get("/me", &[]).await;
    assert_eq!(rig.http.tokens().last(), Some(&None));
    assert_eq!(
        runs.load(Ordering::SeqCst),
        1,
        "the session ended only once"
    );
}

#[tokio::test]
async fn a_403_is_a_refusal_not_the_end_of_the_session() {
    let rig = rig("forbidden", FakeWindow::gives(TOKEN));
    rig.account.sign_in().await.expect("signed in");
    *rig.site.me.lock().expect("lock") = MeAnswer::Status(403);
    let error = rig.client.get("/me", &[]).await.expect_err("refused");
    assert_eq!(error, ClientError::Refused);
    assert!(matches!(
        rig.account.state(),
        SourceAccountState::SignedIn(_)
    ));
    assert_eq!(stored(&rig).as_deref(), Some(TOKEN));
}

#[tokio::test]
async fn another_account_clears_the_data_of_the_first() {
    let rig = rig("switch", FakeWindow::gives(TOKEN));
    let runs = counter(&rig, "caches", false);
    rig.account.sign_in().await.expect("first account");
    assert_eq!(runs.load(Ordering::SeqCst), 0);
    // The same account again keeps its data.
    rig.account
        .sign_in_with_token(Secret::new(TOKEN.to_owned()))
        .await
        .expect("same account");
    assert_eq!(runs.load(Ordering::SeqCst), 0);
    // A different user id drops what the first one had.
    *rig.site.me.lock().expect("lock") = MeAnswer::OtherUser;
    rig.account
        .sign_in_with_token(Secret::new(OTHER_TOKEN.to_owned()))
        .await
        .expect("second account");
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    assert_eq!(stored(&rig).as_deref(), Some(OTHER_TOKEN));
    assert_eq!(rig.account.user_id(), Some(7_000_002));
}

#[tokio::test]
async fn nothing_prints_the_token() {
    let rig = rig("debug", FakeWindow::gives(TOKEN));
    rig.account.sign_in().await.expect("signed in");
    let text = format!(
        "{:?} {:?} {:?} {:?}",
        rig.account,
        rig.client,
        rig.client.auth(),
        rig.account.state()
    );
    assert!(!text.contains(TOKEN), "{text}");
}
