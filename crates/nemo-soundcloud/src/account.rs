//! SoundCloud sign-in and sign-out (soundcloud-full criteria 7 to 12, ADR 0005), all on this
//! computer.
//!
//! Two ways in: the sign-in window (SoundCloud's own page, run by `nemo-signin` in a second
//! process, which hands back the value of the `oauth_token` cookie) and a token the user pastes.
//! Either way the token is checked with one `GET /me` before it is kept, stored only through the
//! [`SecretStore`] (the OS credential store) under [`TOKEN_KEY`], and sent only to the API host.
//! The name and avatar are cached in one small file so the next start needs no request.
//!
//! Sign-out, and a session SoundCloud ended, remove the token, the cached header, the window's
//! profile folder and every registered cache, and keep going after a failed step. Nothing here
//! logs or returns a token, a name or an id in an error.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError, Weak};

use nemo_core::BoxFuture;
use nemo_core::account::{SignOutReport, SignOutWipe, WipeScope};
use nemo_core::secret::Secret;
use nemo_core::secret_store::SecretStore;
use nemo_core::source_account::{
    SourceAccount, SourceAccountError, SourceAccountInfo, SourceAccountState,
};
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::task::AbortHandle;

use crate::client::{ClientError, ScClient};
use crate::map::thumbnails;
use crate::model::parse_one;
use crate::signin::{TokenSource, normalize_token};

/// The credential-store key of the session token (`<crate>/<purpose>`).
pub const TOKEN_KEY: &str = "soundcloud/oauth-token";

/// Shown when SoundCloud gives no user name.
const FALLBACK_NAME: &str = "SoundCloud account";

/// The folders and files the account owns. Sign-out deletes all of them.
#[derive(Debug, Clone)]
pub struct AccountFiles {
    /// The sign-in window's own webview profile folder.
    pub webview_dir: PathBuf,
    /// The cached name and avatar.
    pub cache_file: PathBuf,
}

/// The part of `GET /me` the app keeps. Everything else (names, e-mail, birth date, counters) is
/// ignored by `serde`, so it is never held.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct Me {
    id: u64,
    username: Option<String>,
    avatar_url: Option<String>,
}

impl Me {
    fn info(&self) -> SourceAccountInfo {
        SourceAccountInfo {
            name: self
                .username
                .clone()
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| FALLBACK_NAME.to_owned()),
            avatar: thumbnails(self.avatar_url.as_deref()),
        }
    }
}

/// A [`ClientError`] of the sign-in check as the error the user reads.
fn check_error(e: ClientError) -> SourceAccountError {
    match e {
        ClientError::SessionEnded | ClientError::Refused | ClientError::NotFound => {
            SourceAccountError::Rejected
        }
        ClientError::RateLimited => {
            SourceAccountError::Network("SoundCloud asked to slow down; try again later".to_owned())
        }
        ClientError::Timeout => SourceAccountError::Network("the request timed out".to_owned()),
        ClientError::Off => {
            SourceAccountError::Unavailable("SoundCloud is switched off".to_owned())
        }
        ClientError::Network | ClientError::NoClientId | ClientError::Parse => {
            SourceAccountError::Network("could not reach SoundCloud".to_owned())
        }
    }
}

/// The sign-in window's call while it runs, and the state to go back to when it is stopped.
struct PendingWindow {
    abort: AbortHandle,
    before: SourceAccountState,
}

struct Shared {
    state: Mutex<SourceAccountState>,
    /// The open sign-in window's call, if any.
    pending: Mutex<Option<PendingWindow>>,
    /// The id of the signed-in user, for the account's own list calls.
    user: Mutex<Option<u64>>,
    client: Arc<ScClient>,
    store: Arc<dyn SecretStore>,
    window: Arc<dyn TokenSource>,
    files: AccountFiles,
    /// Sign-out steps of other modules, run in the order they were registered.
    steps: Mutex<Vec<Arc<dyn SignOutWipe>>>,
}

impl Shared {
    fn set_state(&self, state: SourceAccountState) {
        *self.state.lock().unwrap_or_else(PoisonError::into_inner) = state;
    }

    fn state(&self) -> SourceAccountState {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Stop the sign-in window if its call is still running (the child process ends with it) and
    /// put the state from before it back. Returns the state to treat as "before" for the next call.
    fn stop_window(&self) -> Option<SourceAccountState> {
        let pending = self
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()?;
        if pending.abort.is_finished() {
            return None;
        }
        pending.abort.abort();
        self.set_state(pending.before.clone());
        Some(pending.before)
    }

    fn user(&self) -> Option<u64> {
        *self.user.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn set_user(&self, id: Option<u64>) {
        *self.user.lock().unwrap_or_else(PoisonError::into_inner) = id;
        self.client.auth().set_user(id);
    }

    /// Write the cached header. When it cannot be written the session still works and the next
    /// start asks again.
    fn cache(&self, me: &Me) {
        if let Some(dir) = self.files.cache_file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string(me) {
            let _ = std::fs::write(&self.files.cache_file, text);
        }
    }

    fn cached(&self) -> Option<Me> {
        let text = std::fs::read_to_string(&self.files.cache_file).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Run every registered step for all accounts. Failed steps are named in `report`.
    fn run_steps(&self, report: &mut SignOutReport) {
        let steps = self
            .steps
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        for step in &steps {
            report.run_step(step.as_ref(), WipeScope::All);
        }
        for name in &report.failed_steps {
            tracing::warn!(step = *name, "a SoundCloud sign-out step failed");
        }
    }

    /// `GET /me` with `token`, without touching the stored session.
    async fn me(&self, token: &Secret<String>) -> Result<Me, ClientError> {
        let body = self.client.get_as("/me", &[], Some(token)).await?;
        parse_one(&body)
    }

    /// Check, store and activate `raw`. On failure nothing stays stored and the sign-in window's
    /// profile is dropped, so the next try starts clean.
    async fn complete(
        &self,
        raw: &Secret<String>,
    ) -> Result<SourceAccountInfo, SourceAccountError> {
        let token = normalize_token(raw.expose())
            .map(Secret::new)
            .ok_or(SourceAccountError::Rejected)?;
        let me = match self.me(&token).await {
            Ok(me) => me,
            Err(e) => {
                let error = check_error(e);
                if error == SourceAccountError::Rejected {
                    let _ = std::fs::remove_dir_all(&self.files.webview_dir);
                }
                return Err(error);
            }
        };
        self.store
            .set(TOKEN_KEY, token.expose().as_bytes())
            .map_err(|e| SourceAccountError::Storage(e.0.to_owned()))?;
        // Another account than before: its data must not show under the new one.
        if self.user().is_some_and(|before| before != me.id) {
            self.run_steps(&mut SignOutReport::default());
        }
        self.client.auth().set(token);
        self.cache(&me);
        self.set_user(Some(me.id));
        let info = me.info();
        self.set_state(SourceAccountState::SignedIn(info.clone()));
        Ok(info)
    }

    /// Run `complete` and put the old state back when it fails.
    async fn finish(
        &self,
        before: SourceAccountState,
        token: Result<Secret<String>, SourceAccountError>,
    ) -> Result<SourceAccountInfo, SourceAccountError> {
        let result = match token {
            Ok(token) => self.complete(&token).await,
            Err(e) => Err(e),
        };
        if result.is_err() {
            self.set_state(before);
        }
        result
    }

    /// Remove the session: stop sending the token, run every step, delete the profile folder,
    /// the cached header and, last, the stored token. A failed step does not stop the others.
    fn wipe(&self, after: SourceAccountState) -> Result<SignOutReport, SourceAccountError> {
        self.client.auth().clear();
        let mut report = SignOutReport::default();
        self.run_steps(&mut report);
        if std::fs::remove_dir_all(&self.files.webview_dir).is_ok() {
            report.files_removed += 1;
        }
        if std::fs::remove_file(&self.files.cache_file).is_ok() {
            report.files_removed += 1;
        }
        self.set_user(None);
        let removed = self.store.delete(TOKEN_KEY);
        self.set_state(after);
        match removed {
            Ok(existed) => {
                report.credentials_removed = usize::from(existed);
                Ok(report)
            }
            Err(e) => Err(SourceAccountError::Storage(e.0.to_owned())),
        }
    }

    /// SoundCloud ended the session: same removal as a sign-out, then `Expired`.
    fn expire(&self) {
        // The failure is already reported by the state: the user signs in again.
        let _ = self.wipe(SourceAccountState::Expired);
    }

    /// Look up the name of a saved session that has no cached header.
    async fn recheck(&self) {
        match self.client.get("/me", &[]).await {
            Ok(body) => {
                if let Ok(me) = parse_one::<Me>(&body) {
                    self.cache(&me);
                    self.set_user(Some(me.id));
                    self.set_state(SourceAccountState::SignedIn(me.info()));
                }
            }
            // The client has already ended the session and run `expire`.
            Err(ClientError::SessionEnded) => {}
            Err(_) => {
                // Offline or SoundCloud is down: keep the session, the next call retries.
                self.set_state(SourceAccountState::SignedIn(SourceAccountInfo {
                    name: FALLBACK_NAME.to_owned(),
                    avatar: Vec::new(),
                }));
            }
        }
    }
}

/// SoundCloud sign-in backed by the OS credential store.
pub struct ScAccount {
    shared: Arc<Shared>,
    handle: Handle,
}

impl std::fmt::Debug for ScAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScAccount").finish_non_exhaustive()
    }
}

impl ScAccount {
    /// Create the account and load a saved session, if any.
    ///
    /// With a saved token and a cached header the state is `SignedIn` at once and no request is
    /// made. Without the header the name is looked up in the background. A saved token that
    /// cannot be read counts as signed out.
    #[must_use]
    pub fn new(
        client: Arc<ScClient>,
        store: Arc<dyn SecretStore>,
        window: Arc<dyn TokenSource>,
        files: AccountFiles,
        handle: Handle,
    ) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(SourceAccountState::SignedOut),
            pending: Mutex::new(None),
            user: Mutex::new(None),
            client,
            store,
            window,
            files,
            steps: Mutex::new(Vec::new()),
        });
        let weak: Weak<Shared> = Arc::downgrade(&shared);
        shared.client.auth().on_rejected(Arc::new(move || {
            if let Some(shared) = weak.upgrade() {
                shared.expire();
            }
        }));
        let saved = shared
            .store
            .get(TOKEN_KEY)
            .ok()
            .flatten()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .and_then(|text| normalize_token(&text));
        if let Some(token) = saved {
            shared.client.auth().set(Secret::new(token));
            if let Some(me) = shared.cached() {
                shared.set_user(Some(me.id));
                shared.set_state(SourceAccountState::SignedIn(me.info()));
            } else {
                shared.set_state(SourceAccountState::SigningIn);
                let task = Arc::clone(&shared);
                handle.spawn(async move { task.recheck().await });
            }
        }
        Self { shared, handle }
    }

    /// Add a step that sign-out (and an ended session) runs before it removes the token. Each
    /// module that keeps account data registers one; steps run in registration order.
    pub fn register_wipe(&self, step: Arc<dyn SignOutWipe>) {
        self.shared
            .steps
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(step);
    }

    /// The id of the signed-in user, for the account's own list calls. `None` while signed out.
    #[must_use]
    pub fn user_id(&self) -> Option<u64> {
        self.shared.user()
    }

    fn run<T, F>(&self, work: F) -> BoxFuture<'static, Result<T, SourceAccountError>>
    where
        T: Send + 'static,
        F: std::future::Future<Output = Result<T, SourceAccountError>> + Send + 'static,
    {
        join(self.handle.spawn(work))
    }
}

/// Wait for a spawned call. A call that was stopped on purpose (the user pasted a token while the
/// window was open) ends as cancelled.
fn join<T: Send + 'static>(
    task: tokio::task::JoinHandle<Result<T, SourceAccountError>>,
) -> BoxFuture<'static, Result<T, SourceAccountError>> {
    Box::pin(async move {
        task.await.unwrap_or_else(|e| {
            Err(if e.is_cancelled() {
                SourceAccountError::Cancelled
            } else {
                SourceAccountError::Network("request failed".to_owned())
            })
        })
    })
}

impl SourceAccount for ScAccount {
    fn state(&self) -> SourceAccountState {
        self.shared.state()
    }

    fn sign_in(&self) -> BoxFuture<'_, Result<SourceAccountInfo, SourceAccountError>> {
        let shared = Arc::clone(&self.shared);
        // A second window would share the first one's profile folder: the first one ends.
        let before = shared.stop_window().unwrap_or_else(|| shared.state());
        shared.set_state(SourceAccountState::SigningIn);
        let task = self.handle.spawn({
            let shared = Arc::clone(&shared);
            let before = before.clone();
            async move {
                let token = shared.window.token().await;
                shared.finish(before, token).await
            }
        });
        *shared
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(PendingWindow {
            abort: task.abort_handle(),
            before,
        });
        join(task)
    }

    fn sign_in_hint(&self) -> bool {
        self.shared.window.hint()
    }

    fn recheck_sign_in(&self) {
        self.shared.window.recheck();
    }

    fn sign_in_with_token(
        &self,
        token: Secret<String>,
    ) -> BoxFuture<'_, Result<SourceAccountInfo, SourceAccountError>> {
        let shared = Arc::clone(&self.shared);
        let stopped = shared.stop_window();
        self.run(async move {
            let before = stopped.unwrap_or_else(|| shared.state());
            shared.set_state(SourceAccountState::SigningIn);
            shared.finish(before, Ok(token)).await
        })
    }

    fn sign_out(&self) -> BoxFuture<'_, Result<SignOutReport, SourceAccountError>> {
        let shared = Arc::clone(&self.shared);
        let _ = shared.stop_window();
        self.run(async move { shared.wipe(SourceAccountState::SignedOut) })
    }
}

#[cfg(test)]
mod tests;
