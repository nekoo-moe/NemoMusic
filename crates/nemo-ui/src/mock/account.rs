//! The mock Google account and the plain-thread delay it waits on.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use super::thumbs;
use nemo_core::BoxFuture;
use nemo_core::account::{AccountError, AccountInfo, AccountService, AccountState, SignOutReport};
use nemo_core::secret::Secret;
use nemo_core::source_account::{
    SourceAccount, SourceAccountError, SourceAccountInfo, SourceAccountState,
};

/// A future that is ready after a delay, without a runtime: a plain thread sleeps and wakes it.
struct Delay {
    shared: Arc<Mutex<DelayState>>,
}

#[derive(Default)]
struct DelayState {
    done: bool,
    waker: Option<std::task::Waker>,
}

impl Delay {
    fn new(after: Duration) -> Self {
        let shared = Arc::new(Mutex::new(DelayState::default()));
        let worker = Arc::clone(&shared);
        std::thread::spawn(move || {
            std::thread::sleep(after);
            let mut s = worker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            s.done = true;
            if let Some(w) = s.waker.take() {
                w.wake();
            }
        });
        Self { shared }
    }
}

impl std::future::Future for Delay {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<()> {
        let mut s = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if s.done {
            std::task::Poll::Ready(())
        } else {
            s.waker = Some(cx.waker().clone());
            std::task::Poll::Pending
        }
    }
}

/// A fake Google account: signing in (window or pasted cookie) takes a moment, then the account
/// "Mira Okonkwo" is signed in; signing out clears it. Nothing is stored and no cookie is kept.
pub struct MockAccount {
    state: Arc<Mutex<AccountState>>,
    delay: Duration,
}

impl MockAccount {
    /// Signs in after one second, like a quick real sign-in.
    #[must_use]
    pub fn new() -> Self {
        Self::with_delay(Duration::from_secs(1))
    }

    /// Signs in after `delay`.
    #[must_use]
    pub fn with_delay(delay: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(AccountState::SignedOut)),
            delay,
        }
    }

    /// Start already signed in (snapshots of the signed-in card).
    #[must_use]
    pub fn signed_in() -> Self {
        let a = Self::new();
        *a.lock() = AccountState::SignedIn(Self::info());
        a
    }

    fn lock(&self) -> MutexGuard<'_, AccountState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn info() -> AccountInfo {
        AccountInfo {
            name: "Mira Okonkwo".to_owned(),
            handle: Some("@miraokonkwo".to_owned()),
            avatar: thumbs(7),
        }
    }

    fn finish(&self, ok: bool) -> BoxFuture<'_, Result<AccountInfo, AccountError>> {
        *self.lock() = AccountState::SigningIn;
        let wait = Delay::new(self.delay);
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            wait.await;
            let mut s = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if ok {
                let info = Self::info();
                *s = AccountState::SignedIn(info.clone());
                Ok(info)
            } else {
                *s = AccountState::SignedOut;
                Err(AccountError::Rejected)
            }
        })
    }
}

impl Default for MockAccount {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountService for MockAccount {
    fn state(&self) -> AccountState {
        self.lock().clone()
    }

    fn sign_in(&self) -> BoxFuture<'_, Result<AccountInfo, AccountError>> {
        self.finish(true)
    }

    fn sign_in_with_cookie(
        &self,
        cookie: Secret<String>,
    ) -> BoxFuture<'_, Result<AccountInfo, AccountError>> {
        // The mock accepts any non-empty cookie and keeps nothing.
        self.finish(!cookie.expose().trim().is_empty())
    }

    fn sign_out(&self) -> BoxFuture<'_, Result<SignOutReport, AccountError>> {
        *self.lock() = AccountState::SignedOut;
        Box::pin(async {
            Ok(SignOutReport {
                credentials_removed: 1,
                files_removed: 12,
                ended_remotely: true,
                ..SignOutReport::default()
            })
        })
    }
}

/// A fake SoundCloud account: signing in (window or pasted token) takes a moment, then "Mira
/// Okonkwo" is signed in; signing out clears it. Nothing is stored and no token is kept.
pub struct MockScAccount {
    state: Arc<Mutex<SourceAccountState>>,
    delay: Duration,
    /// A window that was started in `SigningIn` and never answers: it reports the "found nothing
    /// for a while" hint (the picture of that state).
    waits_long: bool,
}

impl MockScAccount {
    /// Signed out; signs in after one second, like a quick real sign-in.
    #[must_use]
    pub fn new() -> Self {
        Self::with_state(SourceAccountState::SignedOut)
    }

    /// Start in `state` (snapshots of each card).
    #[must_use]
    pub fn with_state(state: SourceAccountState) -> Self {
        Self {
            waits_long: matches!(state, SourceAccountState::SigningIn),
            state: Arc::new(Mutex::new(state)),
            delay: Duration::from_secs(1),
        }
    }

    /// Start already signed in.
    #[must_use]
    pub fn signed_in() -> Self {
        Self::with_state(SourceAccountState::SignedIn(Self::info()))
    }

    fn lock(&self) -> MutexGuard<'_, SourceAccountState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The account every mock sign-in gives.
    #[must_use]
    pub fn info() -> SourceAccountInfo {
        SourceAccountInfo {
            name: "Mira Okonkwo".to_owned(),
            avatar: thumbs(7),
        }
    }

    fn finish(&self, ok: bool) -> BoxFuture<'_, Result<SourceAccountInfo, SourceAccountError>> {
        *self.lock() = SourceAccountState::SigningIn;
        let wait = Delay::new(self.delay);
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            wait.await;
            let mut s = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if ok {
                let info = Self::info();
                *s = SourceAccountState::SignedIn(info.clone());
                Ok(info)
            } else {
                *s = SourceAccountState::SignedOut;
                Err(SourceAccountError::Rejected)
            }
        })
    }
}

impl Default for MockScAccount {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceAccount for MockScAccount {
    fn state(&self) -> SourceAccountState {
        self.lock().clone()
    }

    fn sign_in(&self) -> BoxFuture<'_, Result<SourceAccountInfo, SourceAccountError>> {
        self.finish(true)
    }

    fn sign_in_hint(&self) -> bool {
        self.waits_long && matches!(*self.lock(), SourceAccountState::SigningIn)
    }

    fn sign_in_with_token(
        &self,
        token: Secret<String>,
    ) -> BoxFuture<'_, Result<SourceAccountInfo, SourceAccountError>> {
        // The mock accepts any non-empty token and keeps nothing.
        self.finish(!token.expose().trim().is_empty())
    }

    fn sign_out(&self) -> BoxFuture<'_, Result<SignOutReport, SourceAccountError>> {
        *self.lock() = SourceAccountState::SignedOut;
        Box::pin(async {
            Ok(SignOutReport {
                credentials_removed: 1,
                files_removed: 2,
                items_removed: 14,
                ..SignOutReport::default()
            })
        })
    }
}

/// Fake SoundCloud writes: every change is taken at once and nothing is kept.
pub struct MockScWrites;

impl nemo_core::source_writes::SourceWrites for MockScWrites {
    fn set_track_liked<'a>(
        &'a self,
        _track: &'a nemo_core::track::TrackId,
        _liked: bool,
    ) -> BoxFuture<'a, Result<(), nemo_core::source_writes::WriteError>> {
        Box::pin(async { Ok(()) })
    }

    fn set_playlist_liked<'a>(
        &'a self,
        _playlist: &'a nemo_core::catalog::BrowseId,
        _liked: bool,
    ) -> BoxFuture<'a, Result<(), nemo_core::source_writes::WriteError>> {
        Box::pin(async { Ok(()) })
    }

    fn unfollow<'a>(
        &'a self,
        _user: &'a nemo_core::catalog::BrowseId,
    ) -> BoxFuture<'a, Result<(), nemo_core::source_writes::WriteError>> {
        Box::pin(async { Ok(()) })
    }

    fn create_playlist<'a>(
        &'a self,
        _title: &'a str,
        _first: Option<&'a nemo_core::track::TrackId>,
    ) -> BoxFuture<
        'a,
        Result<nemo_core::source_writes::NewPlaylist, nemo_core::source_writes::WriteError>,
    > {
        Box::pin(async {
            Ok(nemo_core::source_writes::NewPlaylist {
                id: nemo_core::catalog::BrowseId("sc:playlist:9000001".to_owned()),
            })
        })
    }

    fn add_to_playlist<'a>(
        &'a self,
        _playlist: &'a nemo_core::catalog::BrowseId,
        _track: &'a nemo_core::track::TrackId,
    ) -> BoxFuture<'a, Result<(), nemo_core::source_writes::WriteError>> {
        Box::pin(async { Ok(()) })
    }

    fn remove_from_playlist<'a>(
        &'a self,
        _playlist: &'a nemo_core::catalog::BrowseId,
        _track: &'a nemo_core::track::TrackId,
    ) -> BoxFuture<'a, Result<(), nemo_core::source_writes::WriteError>> {
        Box::pin(async { Ok(()) })
    }

    fn rename_playlist<'a>(
        &'a self,
        _playlist: &'a nemo_core::catalog::BrowseId,
        _title: &'a str,
    ) -> BoxFuture<'a, Result<(), nemo_core::source_writes::WriteError>> {
        Box::pin(async { Ok(()) })
    }
}
