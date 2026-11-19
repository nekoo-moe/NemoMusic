//! The session token the client sends, and what happens when SoundCloud rejects it.
//!
//! The token lives in memory here while someone is signed in (the copy that survives a restart is
//! in the OS credential store, kept by [`crate::account`]). It is sent only as
//! `Authorization: OAuth <token>` to the API host (criterion 9) and never printed: [`Secret`]
//! shows `[redacted]` in `Debug`, and nothing here logs.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use nemo_core::secret::Secret;

/// Called once when SoundCloud rejects the session. It must not block.
pub type RejectedHook = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct State {
    token: Option<Secret<String>>,
    /// Raised by every [`Auth::set`], so a rejection of an old token cannot sign out a new one.
    epoch: u64,
}

/// The token slot of one client.
#[derive(Default)]
pub struct Auth {
    state: Mutex<State>,
    hook: Mutex<Option<RejectedHook>>,
    /// The id of the signed-in user, for the account's own list calls; 0 for nobody.
    user: AtomicU64,
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Auth")
            .field("signed_in", &self.is_signed_in())
            .finish()
    }
}

impl Auth {
    /// Use `token` for every API call from now on.
    pub fn set(&self, token: Secret<String>) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.token = Some(token);
        state.epoch += 1;
    }

    /// Stop sending a token, and forget whose it was.
    pub fn clear(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.token = None;
        state.epoch += 1;
        self.user.store(0, Ordering::SeqCst);
    }

    /// Remember whose token is set (`None` for nobody). The account sets it after `GET /me`.
    pub fn set_user(&self, id: Option<u64>) {
        self.user.store(id.unwrap_or(0), Ordering::SeqCst);
    }

    /// The id of the signed-in user, while a token is set.
    #[must_use]
    pub fn user_id(&self) -> Option<u64> {
        let signed_in = self.is_signed_in();
        let id = self.user.load(Ordering::SeqCst);
        (signed_in && id != 0).then_some(id)
    }

    /// The token now and its epoch, if one is set.
    #[must_use]
    pub fn current(&self) -> Option<(Secret<String>, u64)> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.token.clone().map(|t| (t, state.epoch))
    }

    /// Whether a token is set.
    #[must_use]
    pub fn is_signed_in(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .token
            .is_some()
    }

    /// Run `hook` when the session is rejected (replaces an earlier hook).
    pub fn on_rejected(&self, hook: RejectedHook) {
        *self.hook.lock().unwrap_or_else(PoisonError::into_inner) = Some(hook);
    }

    /// SoundCloud rejected the token of `epoch`. When it is still the current one, it is dropped
    /// and the hook runs once; an older token's rejection changes nothing.
    pub(crate) fn reject(&self, epoch: u64) {
        let dropped = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let current = state.epoch == epoch && state.token.is_some();
            if current {
                state.token = None;
                state.epoch += 1;
                self.user.store(0, Ordering::SeqCst);
            }
            current
        };
        if dropped {
            let hook = self
                .hook
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            if let Some(hook) = hook {
                hook();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn counting(auth: &Auth) -> Arc<AtomicUsize> {
        let count = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&count);
        auth.on_rejected(Arc::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        }));
        count
    }

    #[test]
    fn a_rejection_drops_the_token_and_runs_the_hook_once() {
        let auth = Auth::default();
        let count = counting(&auth);
        auth.set(Secret::new("token-one".to_owned()));
        let (_, epoch) = auth.current().expect("token");
        auth.reject(epoch);
        auth.reject(epoch);
        assert!(!auth.is_signed_in());
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn the_rejection_of_an_old_token_keeps_the_new_one() {
        let auth = Auth::default();
        let count = counting(&auth);
        auth.set(Secret::new("token-one".to_owned()));
        let (_, old) = auth.current().expect("token");
        auth.set(Secret::new("token-two".to_owned()));
        auth.reject(old);
        assert!(auth.is_signed_in());
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn the_user_id_exists_only_while_a_token_is_set() {
        let auth = Auth::default();
        auth.set_user(Some(7_000_001));
        assert_eq!(auth.user_id(), None, "no token");
        auth.set(Secret::new("token-one".to_owned()));
        assert_eq!(auth.user_id(), Some(7_000_001));
        let (_, epoch) = auth.current().expect("token");
        auth.reject(epoch);
        assert_eq!(auth.user_id(), None, "a rejected session forgets the user");
        auth.set(Secret::new("token-two".to_owned()));
        assert_eq!(auth.user_id(), None);
        auth.set_user(Some(7_000_002));
        auth.clear();
        assert_eq!(auth.user_id(), None);
    }

    #[test]
    fn debug_never_shows_the_token() {
        let auth = Auth::default();
        auth.set(Secret::new("token-one".to_owned()));
        let text = format!("{auth:?} {:?}", auth.current());
        assert!(!text.contains("token-one"), "{text}");
    }
}
