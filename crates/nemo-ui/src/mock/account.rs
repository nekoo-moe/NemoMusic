//! The mock Google account and the plain-thread delay it waits on.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use super::thumbs;
use nemo_core::BoxFuture;
use nemo_core::account::{AccountError, AccountInfo, AccountService, AccountState, SignOutReport};
use nemo_core::secret::Secret;

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
