//! Unit tests for the Google account and the account-aware catalog.

use nemo_core::catalog::{
    AlbumDetail, ArtistDetail, BrowseId, CatalogError, Explore, PlaylistDetail, SearchResults,
    Shelf,
};
use nemo_core::track::Track;

use super::*;
use nemo_core::secret_store::MemoryStore;

const COOKIE: &str = "SAPISID=synthetic; other=1";

struct FakeCatalog(&'static str);

impl Catalog for FakeCatalog {
    fn search<'a>(&'a self, _query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        let label = self.0;
        Box::pin(async move { Err(CatalogError::Unavailable(label.to_owned())) })
    }
    fn album<'a>(&'a self, _id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        Box::pin(async { Err(CatalogError::NotFound) })
    }
    fn artist<'a>(
        &'a self,
        _id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        Box::pin(async { Err(CatalogError::NotFound) })
    }
    fn playlist<'a>(
        &'a self,
        _id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        Box::pin(async { Err(CatalogError::NotFound) })
    }
    fn home(&self) -> BoxFuture<'_, Result<Vec<Shelf>, CatalogError>> {
        let label = self.0;
        Box::pin(async move { Err(CatalogError::Unavailable(label.to_owned())) })
    }
    fn explore(&self) -> BoxFuture<'_, Result<Explore, CatalogError>> {
        let label = self.0;
        Box::pin(async move { Err(CatalogError::Unavailable(label.to_owned())) })
    }
    fn liked_songs(&self) -> BoxFuture<'_, Result<Vec<Track>, CatalogError>> {
        let label = self.0;
        Box::pin(async move { Err(CatalogError::Unavailable(label.to_owned())) })
    }
}

struct FakeBackend {
    result: Result<AccountInfo, AccountError>,
}

impl SessionBackend for FakeBackend {
    fn probe(&self, _: Secret<String>) -> BoxFuture<'_, Result<AccountInfo, AccountError>> {
        let result = self.result.clone();
        Box::pin(async move { result })
    }
    fn catalog(&self, _: Secret<String>) -> Arc<dyn Catalog> {
        Arc::new(FakeCatalog("signed-in"))
    }
}

fn info() -> AccountInfo {
    AccountInfo {
        name: "Test Person".to_owned(),
        handle: Some("@test".to_owned()),
        avatar: vec![Thumbnail {
            url: "https://img.invalid/p".to_owned(),
            width: 48,
            height: 48,
        }],
    }
}

struct Fixture {
    account: GoogleAccount,
    store: Arc<MemoryStore>,
    slot: SessionSlot,
    dir: PathBuf,
    runtime: tokio::runtime::Runtime,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A fake helper that prints `stdout` (one line) and exits with `code`.
fn helper_script(stdout: Option<&str>, code: i32) -> String {
    if cfg!(windows) {
        let echo = stdout
            .map(|line| format!("@echo {line}\r\n"))
            .unwrap_or_default();
        format!("{echo}@exit /b {code}\r\n")
    } else {
        let echo = stdout
            .map(|line| format!("printf '%s\\n' '{line}'\n"))
            .unwrap_or_default();
        format!("#!/bin/sh\n{echo}exit {code}\n")
    }
}

/// Write the helper script; on Unix also mark it executable.
fn write_script(path: &Path, script: &str) {
    std::fs::write(path, script).expect("script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
}

fn fixture(
    name: &str,
    result: Result<AccountInfo, AccountError>,
    helper: Option<&str>,
    store: Option<Arc<MemoryStore>>,
) -> Fixture {
    let dir = std::env::temp_dir().join(format!("nemo-ytm-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let helper_path = dir.join(if cfg!(windows) {
        "helper.cmd"
    } else {
        "helper.sh"
    });
    if let Some(script) = helper {
        write_script(&helper_path, script);
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    let store = store.unwrap_or_default();
    let slot: SessionSlot = Arc::default();
    let config = AccountConfig {
        helper: helper_path,
        webview_dir: dir.join("webview"),
        cache_file: dir.join("account").join("account.json"),
    };
    let account = GoogleAccount::new(
        config,
        store.clone(),
        Arc::new(FakeBackend { result }),
        slot.clone(),
        runtime.handle().clone(),
    )
    .with_timeout(Duration::from_secs(20));
    Fixture {
        account,
        store,
        slot,
        dir,
        runtime,
    }
}

#[test]
fn account_header_survives_the_cache_round_trip() {
    let back = info_from_json(&info_to_json(&info())).expect("round trip");
    assert_eq!(back, info());
    assert_eq!(info_from_json("{}"), None);
    assert_eq!(info_from_json("nope"), None);
}

#[test]
fn helper_output_is_one_json_line() {
    let ok = parse_helper_output("{\"cookie\":\"SAPISID=x; a=b\"}\n").expect("cookie");
    assert_eq!(ok.expose(), "SAPISID=x; a=b");
    assert!(parse_helper_output("").is_none());
    assert!(parse_helper_output("{\"cookie\":\"\"}").is_none());
    assert!(parse_helper_output("SAPISID=x").is_none());
    assert_eq!(format!("{ok:?}"), "[redacted]");
}

#[test]
fn sign_in_with_a_helper_stores_the_session() {
    let script = helper_script(Some("{\"cookie\":\"SAPISID=synthetic; a=b\"}"), 0);
    let f = fixture("flow", Ok(info()), Some(&script), None);
    assert_eq!(f.account.state(), AccountState::SignedOut);
    let got = f.runtime.block_on(f.account.sign_in()).expect("sign in");
    assert_eq!(got, info());
    assert_eq!(f.account.state(), AccountState::SignedIn(info()));
    assert!(f.slot.read().expect("lock").is_some());
    assert!(f.store.get("session-0").expect("get").is_some());
}

#[test]
fn closing_the_window_cancels_and_keeps_the_old_state() {
    let f = fixture("cancel", Ok(info()), Some(&helper_script(None, 2)), None);
    let err = f.runtime.block_on(f.account.sign_in()).expect_err("cancel");
    assert_eq!(err, AccountError::Cancelled);
    assert_eq!(f.account.state(), AccountState::SignedOut);
    assert!(f.store.is_empty());
}

#[test]
fn a_missing_webview_runtime_is_reported() {
    let f = fixture("runtime", Ok(info()), Some(&helper_script(None, 3)), None);
    let err = f
        .runtime
        .block_on(f.account.sign_in())
        .expect_err("missing");
    assert_eq!(err, AccountError::Unavailable(WEBVIEW_MISSING.to_owned()));
}

#[test]
fn a_missing_helper_is_unavailable() {
    let f = fixture("nohelper", Ok(info()), None, None);
    let err = f
        .runtime
        .block_on(f.account.sign_in())
        .expect_err("no helper");
    assert!(matches!(err, AccountError::Unavailable(_)));
    assert_eq!(f.account.state(), AccountState::SignedOut);
}

#[test]
fn a_pasted_cookie_is_checked_and_stored() {
    let f = fixture("paste", Ok(info()), None, None);
    let got = f
        .runtime
        .block_on(
            f.account
                .sign_in_with_cookie(Secret::new(COOKIE.to_owned())),
        )
        .expect("paste");
    assert_eq!(got.name, "Test Person");
    let saved = Vault::new(f.store.clone()).load().expect("load");
    assert_eq!(saved.as_deref(), Some("SAPISID=synthetic; other=1;"));
}

#[test]
fn a_rejected_cookie_stores_nothing() {
    let f = fixture("reject", Err(AccountError::Rejected), None, None);
    let err = f
        .runtime
        .block_on(
            f.account
                .sign_in_with_cookie(Secret::new(COOKIE.to_owned())),
        )
        .expect_err("rejected");
    assert_eq!(err, AccountError::Rejected);
    assert!(f.store.is_empty());
    assert!(f.slot.read().expect("lock").is_none());
    assert_eq!(f.account.state(), AccountState::SignedOut);
}

#[test]
fn a_cookie_without_a_session_never_reaches_the_network() {
    let f = fixture("nosession", Ok(info()), None, None);
    let err = f
        .runtime
        .block_on(f.account.sign_in_with_cookie(Secret::new("a=1".to_owned())))
        .expect_err("rejected");
    assert_eq!(err, AccountError::Rejected);
}

#[test]
fn a_saved_session_is_loaded_at_start() {
    let first = fixture("persist", Ok(info()), None, None);
    first
        .runtime
        .block_on(
            first
                .account
                .sign_in_with_cookie(Secret::new(COOKIE.to_owned())),
        )
        .expect("sign in");
    let second = fixture("persist2", Ok(info()), None, Some(first.store.clone()));
    // Same credentials, different folder: no cached header, so the account checks it.
    for _ in 0..100 {
        if matches!(second.account.state(), AccountState::SignedIn(_)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(second.account.state(), AccountState::SignedIn(info()));
    assert!(second.slot.read().expect("lock").is_some());
}

#[test]
fn a_cached_header_makes_start_instant() {
    let first = fixture("cache", Ok(info()), None, None);
    first
        .runtime
        .block_on(
            first
                .account
                .sign_in_with_cookie(Secret::new(COOKIE.to_owned())),
        )
        .expect("sign in");
    let again = GoogleAccount::new(
        AccountConfig {
            helper: first.dir.join("none.exe"),
            webview_dir: first.dir.join("webview"),
            cache_file: first.dir.join("account").join("account.json"),
        },
        first.store.clone(),
        Arc::new(FakeBackend {
            result: Err(AccountError::Network("offline".to_owned())),
        }),
        Arc::default(),
        first.runtime.handle().clone(),
    );
    assert_eq!(again.state(), AccountState::SignedIn(info()));
}

#[test]
fn a_rejected_saved_session_is_wiped() {
    let store = Arc::new(MemoryStore::default());
    Vault::new(store.clone())
        .save("SAPISID=old;")
        .expect("seed");
    let f = fixture("stale", Err(AccountError::Rejected), None, Some(store));
    for _ in 0..100 {
        if f.account.state() == AccountState::SignedOut {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(f.account.state(), AccountState::SignedOut);
    assert!(f.store.is_empty());
    assert!(f.slot.read().expect("lock").is_none());
}

#[test]
fn sign_out_wipes_credentials_folders_and_session() {
    let f = fixture("out", Ok(info()), None, None);
    f.runtime
        .block_on(
            f.account
                .sign_in_with_cookie(Secret::new(COOKIE.to_owned())),
        )
        .expect("sign in");
    std::fs::create_dir_all(f.dir.join("webview").join("EBWebView")).expect("profile");
    let report = f.runtime.block_on(f.account.sign_out()).expect("sign out");
    assert_eq!(report.credentials_removed, 2);
    assert_eq!(report.files_removed, 2);
    assert!(!report.ended_remotely);
    assert!(f.store.is_empty());
    assert!(!f.dir.join("webview").exists());
    assert!(!f.dir.join("account").join("account.json").exists());
    assert!(f.slot.read().expect("lock").is_none());
    assert_eq!(f.account.state(), AccountState::SignedOut);
    // Signing out twice is harmless.
    let again = f.runtime.block_on(f.account.sign_out()).expect("again");
    assert_eq!(again, SignOutReport::default());
}

/// A sign-out step that records when it ran and whether the session was still stored then.
struct RecordingStep {
    name: &'static str,
    fails: bool,
    store: Arc<MemoryStore>,
    log: Arc<Mutex<Vec<(&'static str, bool)>>>,
}

impl SignOutWipe for RecordingStep {
    fn name(&self) -> &'static str {
        self.name
    }

    fn wipe(&self, scope: WipeScope) -> Result<usize, nemo_core::account::WipeError> {
        assert_eq!(scope, WipeScope::All);
        let session_stored = self.store.get("session-manifest").expect("get").is_some();
        self.log
            .lock()
            .expect("lock")
            .push((self.name, session_stored));
        if self.fails {
            Err(nemo_core::account::WipeError("cannot delete"))
        } else {
            Ok(1)
        }
    }
}

#[test]
fn sign_out_runs_every_step_before_the_vault_is_wiped() {
    let f = fixture("steps", Ok(info()), None, None);
    f.runtime
        .block_on(
            f.account
                .sign_in_with_cookie(Secret::new(COOKIE.to_owned())),
        )
        .expect("sign in");
    let log = Arc::new(Mutex::new(Vec::new()));
    for (name, fails) in [("offline", false), ("ai", true), ("cast", false)] {
        f.account.register_wipe(Arc::new(RecordingStep {
            name,
            fails,
            store: f.store.clone(),
            log: log.clone(),
        }));
    }
    let report = f.runtime.block_on(f.account.sign_out()).expect("sign out");
    assert_eq!(
        *log.lock().expect("lock"),
        vec![("offline", true), ("ai", true), ("cast", true)]
    );
    assert_eq!(report.failed_steps, vec!["ai"]);
    assert_eq!(report.items_removed, 2);
    assert_eq!(report.credentials_removed, 2);
    assert!(f.store.is_empty());
    assert_eq!(f.account.state(), AccountState::SignedOut);
}

#[test]
fn the_new_pages_follow_the_active_session_too() {
    let slot: SessionSlot = Arc::default();
    let catalog = AccountCatalog::new(Arc::new(FakeCatalog("anonymous")), slot.clone());
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    let label = |r: Result<Explore, CatalogError>| match r {
        Err(CatalogError::Unavailable(label)) => label,
        other => format!("{other:?}"),
    };
    assert_eq!(label(rt.block_on(catalog.explore())), "anonymous");
    *slot.write().expect("lock") = Some(Arc::new(FakeCatalog("signed-in")));
    assert_eq!(label(rt.block_on(catalog.explore())), "signed-in");
    assert_eq!(
        rt.block_on(catalog.liked_songs()),
        Err(CatalogError::Unavailable("signed-in".to_owned()))
    );
    // A page the fake does not implement keeps the trait default.
    assert_eq!(
        rt.block_on(catalog.history()),
        Err(CatalogError::Unavailable("history".to_owned()))
    );
}

#[test]
fn the_catalog_switches_live_between_anonymous_and_signed_in() {
    let slot: SessionSlot = Arc::default();
    let catalog = AccountCatalog::new(Arc::new(FakeCatalog("anonymous")), slot.clone());
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    let which = |c: &AccountCatalog| match rt.block_on(c.home()) {
        Err(CatalogError::Unavailable(label)) => label,
        other => format!("{other:?}"),
    };
    assert_eq!(which(&catalog), "anonymous");
    *slot.write().expect("lock") = Some(Arc::new(FakeCatalog("signed-in")));
    assert_eq!(which(&catalog), "signed-in");
    let search = rt.block_on(catalog.search("x"));
    assert_eq!(
        search,
        Err(CatalogError::Unavailable("signed-in".to_owned()))
    );
    *slot.write().expect("lock") = None;
    assert_eq!(which(&catalog), "anonymous");
}
