//! Google account sign-in for YouTube Music (ADR 0001), all on this computer.
//!
//! Two ways in: the `nemo-signin` helper (Google's own page in a webview window, which hands
//! back the session cookies) and a cookie the user pastes. Either way the cookie is checked
//! against YouTube Music, stored in the OS credential store (Windows Credential Manager, or the
//! Secret Service on Linux) and used for the account-aware catalog. The cookie is never passed to `yt-dlp`, never logged, and never written to a file.
//! Messages never contain cookies, names or ids.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::account::{
    AccountError, AccountInfo, AccountService, AccountState, SignOutReport, SignOutWipe, WipeScope,
};
use nemo_core::catalog::Catalog;
use nemo_core::secret::Secret;
use nemo_core::secret_store::SecretStore;
use nemo_core::track::Thumbnail;
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::runtime::Handle;

use crate::credentials::Vault;
use crate::session::{SessionBackend, normalize_cookie};

mod catalog;

pub use catalog::AccountCatalog;

/// How long the sign-in window may stay open.
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_mins(10);
/// File name of the sign-in helper.
#[cfg(windows)]
pub const HELPER_NAME: &str = "nemo-signin.exe";
/// File name of the sign-in helper.
#[cfg(not(windows))]
pub const HELPER_NAME: &str = "nemo-signin";
/// What the user is told when the helper reports that its webview cannot start (exit code 3).
#[cfg(windows)]
pub const WEBVIEW_MISSING: &str = "the WebView2 runtime is missing";
/// What the user is told when the helper reports that its webview cannot start (exit code 3).
#[cfg(not(windows))]
pub const WEBVIEW_MISSING: &str = "the WebKitGTK webview is not available";
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
    /// The helper's own webview profile folder. Deleted on sign-out.
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
        Some(3) => Err(AccountError::Unavailable(WEBVIEW_MISSING.to_owned())),
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
    /// Sign-out steps of other crates, run in the order they were registered.
    steps: Mutex<Vec<Arc<dyn SignOutWipe>>>,
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

    /// Sign out: run every registered step for all accounts, then remove the local session.
    /// A failed step is named in the report and the others still run. The `Vault` entries go
    /// last, so a failed step can be retried while the session still exists.
    fn sign_out(&self) -> Result<SignOutReport, AccountError> {
        self.set_session(None);
        let steps = self
            .steps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut report = SignOutReport::default();
        for step in &steps {
            report.run_step(step.as_ref(), WipeScope::All);
        }
        for name in &report.failed_steps {
            tracing::warn!(step = *name, "a sign-out step failed");
        }
        self.wipe(report)
    }

    /// Remove the local session: the helper's folder, the cached header and, last, the `Vault`
    /// entries. Adds the counts to `report`.
    fn wipe(&self, mut report: SignOutReport) -> Result<SignOutReport, AccountError> {
        self.set_session(None);
        if std::fs::remove_dir_all(&self.config.webview_dir).is_ok() {
            report.files_removed += 1;
        }
        if std::fs::remove_file(&self.config.cache_file).is_ok() {
            report.files_removed += 1;
        }
        let credentials = self.vault.wipe();
        self.set_state(AccountState::SignedOut);
        match credentials {
            Ok(credentials_removed) => {
                report.credentials_removed = credentials_removed;
                Ok(report)
            }
            Err(err) => Err(AccountError::Storage(err.0.to_owned())),
        }
    }
}

/// Google account sign-in backed by the OS credential store.
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
            steps: Mutex::new(Vec::new()),
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

    /// Add a step that sign-out runs before it removes the session (ADR 0004, Decision 3).
    /// Each crate that keeps account data registers one; steps run in registration order.
    pub fn register_wipe(&self, step: Arc<dyn SignOutWipe>) {
        self.shared
            .steps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(step);
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
                let _ = self.wipe(SignOutReport::default());
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
        self.run(async move { shared.sign_out() })
    }
}

#[cfg(test)]
mod tests;
