//! Google account sign-in for YouTube Music (ADR 0001), all on this computer.
//!
//! Two ways in: the `nemo-signin` helper (Google's own page in a `WebView2` window, which hands
//! back the session cookies) and a cookie the user pastes. Either way the cookie is checked
//! against YouTube Music, stored in Windows Credential Manager and used for the account-aware
//! catalog. The cookie is never passed to `yt-dlp`, never logged, and never written to a file.
//! Messages never contain cookies, names or ids.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::account::{AccountError, AccountInfo, AccountService, AccountState, SignOutReport};
use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, BrowseId, Catalog, CatalogError,
    Explore, FeedShelf, HistorySection, HomeFeed, PlaylistDetail, PlaylistSummary, PodcastDetail,
    SearchResults, Shelf,
};
use nemo_core::secret::Secret;
use nemo_core::track::{Thumbnail, Track};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::runtime::Handle;

use crate::credentials::{SecretStore, Vault};
use crate::session::{SessionBackend, normalize_cookie};

/// How long the sign-in window may stay open.
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_mins(10);
/// File name of the sign-in helper.
pub const HELPER_NAME: &str = "nemo-signin.exe";
/// Most bytes read from the helper's stdout.
const MAX_HELPER_OUTPUT: u64 = 64 * 1024;

/// The catalog that is in use while someone is signed in. Shared between the account and the
/// account-aware catalog, so switching needs no restart.
pub type SessionSlot = Arc<RwLock<Option<Arc<dyn Catalog>>>>;

/// Where the account keeps its local files and its helper.
#[derive(Debug, Clone)]
pub struct AccountConfig {
    /// The sign-in helper program.
    pub helper: PathBuf,
    /// The helper's own `WebView2` profile folder. Deleted on sign-out.
    pub webview_dir: PathBuf,
    /// The cached account header (name, avatar). Deleted on sign-out.
    pub cache_file: PathBuf,
}

/// The sign-in helper next to the running program, if there is one.
#[must_use]
pub fn default_helper_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join(HELPER_NAME))
}

// ---------------------------------------------------------------------------------------------
// Account cache (display data only, no tokens)
// ---------------------------------------------------------------------------------------------

fn info_to_json(info: &AccountInfo) -> String {
    json!({
        "name": info.name,
        "handle": info.handle,
        "avatar": info.avatar.iter().map(|t| json!({
            "url": t.url, "width": t.width, "height": t.height
        })).collect::<Vec<_>>(),
    })
    .to_string()
}

fn info_from_json(text: &str) -> Option<AccountInfo> {
    let value: Value = serde_json::from_str(text).ok()?;
    let name = value.get("name")?.as_str()?.to_owned();
    let number = |t: &Value, key: &str| {
        t.get(key)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0)
    };
    Some(AccountInfo {
        name,
        handle: value
            .get("handle")
            .and_then(Value::as_str)
            .map(str::to_owned),
        avatar: value
            .get("avatar")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|t| {
                        Some(Thumbnail {
                            url: t.get("url")?.as_str()?.to_owned(),
                            width: number(t, "width"),
                            height: number(t, "height"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

// ---------------------------------------------------------------------------------------------
// The sign-in helper
// ---------------------------------------------------------------------------------------------

/// Read the cookie out of the helper's one output line.
fn parse_helper_output(output: &str) -> Option<Secret<String>> {
    let value: Value = serde_json::from_str(output.lines().next()?.trim()).ok()?;
    let cookie = value.get("cookie")?.as_str()?;
    (!cookie.is_empty()).then(|| Secret::new(cookie.to_owned()))
}

/// Run the helper and wait for its answer. The child is killed if this future is dropped.
async fn run_helper(
    helper: &Path,
    webview_dir: &Path,
    timeout: Duration,
) -> Result<Secret<String>, AccountError> {
    let mut child = Command::new(helper)
        .arg("--data-dir")
        .arg(webview_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| {
            AccountError::Unavailable("the sign-in helper cannot be started".to_owned())
        })?;
    let Some(stdout) = child.stdout.take() else {
        return Err(AccountError::Unavailable(
            "no output from the sign-in helper".to_owned(),
        ));
    };
    let work = async {
        let mut text = String::new();
        let read = stdout
            .take(MAX_HELPER_OUTPUT)
            .read_to_string(&mut text)
            .await;
        let status = child.wait().await;
        (read, status, text)
    };
    let Ok((read, status, text)) = tokio::time::timeout(timeout, work).await else {
        // The child is killed when it is dropped here.
        return Err(AccountError::Cancelled);
    };
    let status =
        status.map_err(|_| AccountError::Unavailable("the sign-in helper failed".to_owned()))?;
    match status.code() {
        Some(0) if read.is_ok() => parse_helper_output(&text).ok_or_else(|| {
            AccountError::Unavailable("the sign-in helper gave no session".to_owned())
        }),
        Some(2) => Err(AccountError::Cancelled),
        Some(3) => Err(AccountError::Unavailable(
            "the WebView2 runtime is missing".to_owned(),
        )),
        _ => Err(AccountError::Unavailable(
            "the sign-in window failed".to_owned(),
        )),
    }
}

// ---------------------------------------------------------------------------------------------
// The account service
// ---------------------------------------------------------------------------------------------

struct Shared {
    state: Mutex<AccountState>,
    slot: SessionSlot,
    vault: Vault,
    backend: Arc<dyn SessionBackend>,
    config: AccountConfig,
    timeout: Duration,
}

impl Shared {
    fn set_state(&self, state: AccountState) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = state;
        }
    }

    fn state(&self) -> AccountState {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn set_session(&self, session: Option<Arc<dyn Catalog>>) {
        if let Ok(mut guard) = self.slot.write() {
            *guard = session;
        }
    }

    /// Check, store and activate a cookie. On failure nothing stays stored.
    async fn complete(&self, raw: &Secret<String>) -> Result<AccountInfo, AccountError> {
        let cookie = normalize_cookie(raw.expose()).ok_or(AccountError::Rejected)?;
        let cookie = Secret::new(cookie);
        let info = self.backend.probe(cookie.clone()).await?;
        self.vault
            .save(cookie.expose())
            .map_err(|e| AccountError::Storage(e.0.to_owned()))?;
        if let Some(dir) = self.config.cache_file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&self.config.cache_file, info_to_json(&info));
        self.set_session(Some(self.backend.catalog(cookie)));
        self.set_state(AccountState::SignedIn(info.clone()));
        Ok(info)
    }

    /// Run `complete` and put the old state back when it fails.
    async fn finish(
        &self,
        before: AccountState,
        cookie: Result<Secret<String>, AccountError>,
    ) -> Result<AccountInfo, AccountError> {
        let result = match cookie {
            Ok(cookie) => self.complete(&cookie).await,
            Err(err) => Err(err),
        };
        if result.is_err() {
            self.set_state(before);
        }
        result
    }

    fn wipe(&self) -> Result<SignOutReport, AccountError> {
        self.set_session(None);
        let credentials = self.vault.wipe();
        let mut files_removed = 0;
        if std::fs::remove_dir_all(&self.config.webview_dir).is_ok() {
            files_removed += 1;
        }
        if std::fs::remove_file(&self.config.cache_file).is_ok() {
            files_removed += 1;
        }
        self.set_state(AccountState::SignedOut);
        match credentials {
            Ok(credentials_removed) => Ok(SignOutReport {
                credentials_removed,
                files_removed,
                ended_remotely: false,
            }),
            Err(err) => Err(AccountError::Storage(err.0.to_owned())),
        }
    }
}

/// Google account sign-in backed by Windows Credential Manager.
pub struct GoogleAccount {
    shared: Arc<Shared>,
    handle: Handle,
}

impl GoogleAccount {
    /// Create the account service and load a saved session, if any.
    ///
    /// With a saved session and a cached account header the state is `SignedIn` at once and no
    /// request is made. Without the header, the session is checked in the background.
    #[must_use]
    pub fn new(
        config: AccountConfig,
        store: Arc<dyn SecretStore>,
        backend: Arc<dyn SessionBackend>,
        slot: SessionSlot,
        handle: Handle,
    ) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(AccountState::SignedOut),
            slot,
            vault: Vault::new(store),
            backend,
            config,
            timeout: SIGN_IN_TIMEOUT,
        });
        let saved = shared.vault.load().ok().flatten();
        if let Some(cookie) = saved {
            let cookie = Secret::new(cookie);
            shared.set_session(Some(shared.backend.catalog(cookie.clone())));
            let cached = std::fs::read_to_string(&shared.config.cache_file)
                .ok()
                .and_then(|t| info_from_json(&t));
            if let Some(info) = cached {
                shared.set_state(AccountState::SignedIn(info));
            } else {
                shared.set_state(AccountState::SigningIn);
                let task = Arc::clone(&shared);
                handle.spawn(async move { task.recheck(cookie).await });
            }
        }
        Self { shared, handle }
    }

    /// Change how long the sign-in window may stay open (tests use a short time).
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        if let Some(shared) = Arc::get_mut(&mut self.shared) {
            shared.timeout = timeout;
        }
        self
    }

    fn run<T, F>(&self, work: F) -> BoxFuture<'static, Result<T, AccountError>>
    where
        T: Send + 'static,
        F: std::future::Future<Output = Result<T, AccountError>> + Send + 'static,
    {
        let task = self.handle.spawn(work);
        Box::pin(async move {
            task.await
                .unwrap_or_else(|_| Err(AccountError::Network("request was cancelled".to_owned())))
        })
    }
}

impl Shared {
    /// Look up the account header of a saved session that has no cached header.
    async fn recheck(&self, cookie: Secret<String>) {
        match self.backend.probe(cookie).await {
            Ok(info) => {
                if let Some(dir) = self.config.cache_file.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(&self.config.cache_file, info_to_json(&info));
                self.set_state(AccountState::SignedIn(info));
            }
            Err(AccountError::Rejected) => {
                let _ = self.wipe();
            }
            Err(_) => {
                // Offline or YouTube is down: keep the session, the catalog retries on use.
                self.set_state(AccountState::SignedIn(AccountInfo {
                    name: "YouTube Music account".to_owned(),
                    handle: None,
                    avatar: Vec::new(),
                }));
            }
        }
    }
}

impl AccountService for GoogleAccount {
    fn state(&self) -> AccountState {
        self.shared.state()
    }

    fn sign_in(&self) -> BoxFuture<'_, Result<AccountInfo, AccountError>> {
        let shared = Arc::clone(&self.shared);
        self.run(async move {
            let before = shared.state();
            shared.set_state(AccountState::SigningIn);
            let cookie = run_helper(
                &shared.config.helper,
                &shared.config.webview_dir,
                shared.timeout,
            )
            .await;
            shared.finish(before, cookie).await
        })
    }

    fn sign_in_with_cookie(
        &self,
        cookie: Secret<String>,
    ) -> BoxFuture<'_, Result<AccountInfo, AccountError>> {
        let shared = Arc::clone(&self.shared);
        self.run(async move {
            let before = shared.state();
            shared.set_state(AccountState::SigningIn);
            shared.finish(before, Ok(cookie)).await
        })
    }

    fn sign_out(&self) -> BoxFuture<'_, Result<SignOutReport, AccountError>> {
        let shared = Arc::clone(&self.shared);
        self.run(async move { shared.wipe() })
    }
}

// ---------------------------------------------------------------------------------------------
// The account-aware catalog
// ---------------------------------------------------------------------------------------------

/// Uses the signed-in session while one is active and the anonymous catalog otherwise. The
/// choice is made per request, so signing in or out takes effect at once.
pub struct AccountCatalog {
    anonymous: Arc<dyn Catalog>,
    slot: SessionSlot,
}

impl AccountCatalog {
    /// `slot` is the one given to [`GoogleAccount::new`].
    #[must_use]
    pub fn new(anonymous: Arc<dyn Catalog>, slot: SessionSlot) -> Self {
        Self { anonymous, slot }
    }

    fn current(&self) -> Arc<dyn Catalog> {
        self.slot
            .read()
            .ok()
            .and_then(|s| s.clone())
            .unwrap_or_else(|| Arc::clone(&self.anonymous))
    }
}

impl Catalog for AccountCatalog {
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.search(query).await })
    }

    fn album<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.album(id).await })
    }

    fn artist<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.artist(id).await })
    }

    fn playlist<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.playlist(id).await })
    }

    fn home(&self) -> BoxFuture<'_, Result<Vec<Shelf>, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.home().await })
    }

    fn home_feed(&self) -> BoxFuture<'_, Result<HomeFeed, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.home_feed().await })
    }

    fn library_playlists(&self) -> BoxFuture<'_, Result<Vec<PlaylistSummary>, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.library_playlists().await })
    }

    fn liked_songs(&self) -> BoxFuture<'_, Result<Vec<Track>, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.liked_songs().await })
    }

    fn library_albums(&self) -> BoxFuture<'_, Result<Vec<AlbumSummary>, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.library_albums().await })
    }

    fn library_artists(&self) -> BoxFuture<'_, Result<Vec<ArtistSummary>, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.library_artists().await })
    }

    fn history(&self) -> BoxFuture<'_, Result<Vec<HistorySection>, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.history().await })
    }

    fn explore(&self) -> BoxFuture<'_, Result<Explore, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.explore().await })
    }

    fn mood_playlists<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<Vec<FeedShelf>, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.mood_playlists(id).await })
    }

    fn podcast<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PodcastDetail, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.podcast(id).await })
    }
    fn rate_song<'a>(
        &'a self,
        video_id: &'a nemo_core::track::VideoId,
        liked: bool,
    ) -> BoxFuture<'a, Result<(), CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.rate_song(video_id, liked).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::fake::MemoryStore;

    const COOKIE: &str = "SAPISID=synthetic; other=1";

    struct FakeCatalog(&'static str);

    impl Catalog for FakeCatalog {
        fn search<'a>(
            &'a self,
            _query: &'a str,
        ) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
            let label = self.0;
            Box::pin(async move { Err(CatalogError::Unavailable(label.to_owned())) })
        }
        fn album<'a>(
            &'a self,
            _id: &'a BrowseId,
        ) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
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

    fn fixture(
        name: &str,
        result: Result<AccountInfo, AccountError>,
        helper: Option<&str>,
        store: Option<Arc<MemoryStore>>,
    ) -> Fixture {
        let dir = std::env::temp_dir().join(format!("nemo-ytm-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let helper_path = dir.join("helper.cmd");
        if let Some(script) = helper {
            std::fs::write(&helper_path, script).expect("script");
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

    #[cfg(windows)]
    #[test]
    fn sign_in_with_a_helper_stores_the_session() {
        let f = fixture(
            "flow",
            Ok(info()),
            Some("@echo {\"cookie\":\"SAPISID=synthetic; a=b\"}\r\n@exit /b 0\r\n"),
            None,
        );
        assert_eq!(f.account.state(), AccountState::SignedOut);
        let got = f.runtime.block_on(f.account.sign_in()).expect("sign in");
        assert_eq!(got, info());
        assert_eq!(f.account.state(), AccountState::SignedIn(info()));
        assert!(f.slot.read().expect("lock").is_some());
        assert!(
            f.store
                .entries
                .lock()
                .expect("lock")
                .contains_key("session-0")
        );
    }

    #[cfg(windows)]
    #[test]
    fn closing_the_window_cancels_and_keeps_the_old_state() {
        let f = fixture("cancel", Ok(info()), Some("@exit /b 2\r\n"), None);
        let err = f.runtime.block_on(f.account.sign_in()).expect_err("cancel");
        assert_eq!(err, AccountError::Cancelled);
        assert_eq!(f.account.state(), AccountState::SignedOut);
        assert!(f.store.entries.lock().expect("lock").is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn a_missing_webview_runtime_is_reported() {
        let f = fixture("runtime", Ok(info()), Some("@exit /b 3\r\n"), None);
        let err = f
            .runtime
            .block_on(f.account.sign_in())
            .expect_err("missing");
        assert!(matches!(err, AccountError::Unavailable(m) if m.contains("WebView2")));
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
        assert!(f.store.entries.lock().expect("lock").is_empty());
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
        assert!(f.store.entries.lock().expect("lock").is_empty());
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
        assert!(f.store.entries.lock().expect("lock").is_empty());
        assert!(!f.dir.join("webview").exists());
        assert!(!f.dir.join("account").join("account.json").exists());
        assert!(f.slot.read().expect("lock").is_none());
        assert_eq!(f.account.state(), AccountState::SignedOut);
        // Signing out twice is harmless.
        let again = f.runtime.block_on(f.account.sign_out()).expect("again");
        assert_eq!(again, SignOutReport::default());
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
}
