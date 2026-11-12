//! Google account sign-in for YouTube Music (ADR 0001).
//!
//! Everything stays on this computer: the session lives in the OS credential store (Windows
//! Credential Manager or the Linux Secret Service), and the account data the app fetches is
//! cached locally. Implementations never log cookies, tokens,
//! e-mail addresses or account ids.

use crate::BoxFuture;
use crate::secret::Secret;
use crate::track::Thumbnail;

/// What the UI may show about the signed-in account. No ids, no e-mail, no tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountInfo {
    /// Display name, as YouTube Music shows it.
    pub name: String,
    /// Channel handle such as `@name`, if YouTube Music shows one.
    pub handle: Option<String>,
    /// Avatar sizes.
    pub avatar: Vec<Thumbnail>,
}

/// Sign-in state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AccountState {
    /// Nobody is signed in. The catalog works anonymously.
    #[default]
    SignedOut,
    /// The sign-in window is open or a pasted cookie is being checked.
    SigningIn,
    /// Signed in. Home, library, likes and history come from this account.
    SignedIn(AccountInfo),
}

/// Signing in or out failed. Messages are short and user-safe.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AccountError {
    /// The user closed the sign-in window before finishing.
    #[error("sign-in was cancelled")]
    Cancelled,
    /// Google refused this sign-in window. Use the cookie paste fallback.
    #[error("Google blocked the sign-in window; paste a cookie instead")]
    Blocked,
    /// The session was not accepted by YouTube Music.
    #[error("YouTube Music did not accept this sign-in")]
    Rejected,
    /// The sign-in helper or WebView2 runtime is missing.
    #[error("the sign-in window is not available: {0}")]
    Unavailable(String),
    /// The OS credential store failed.
    #[error("cannot store the sign-in on this computer: {0}")]
    Storage(String),
    /// The network request failed.
    #[error("network error: {0}")]
    Network(String),
}

/// What sign-out removed, for the confirmation message.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignOutReport {
    /// OS credential store entries removed.
    pub credentials_removed: usize,
    /// Cache files and folders removed.
    pub files_removed: usize,
    /// Whether the session was also ended on Google's side.
    pub ended_remotely: bool,
}

/// Google account sign-in.
pub trait AccountService: Send + Sync {
    /// The state now. Cheap; safe to call every frame.
    fn state(&self) -> AccountState;

    /// Open the sign-in window and wait for the user to finish.
    fn sign_in(&self) -> BoxFuture<'_, Result<AccountInfo, AccountError>>;

    /// Sign in with a cookie header the user copied from their own browser (fallback).
    fn sign_in_with_cookie(
        &self,
        cookie: Secret<String>,
    ) -> BoxFuture<'_, Result<AccountInfo, AccountError>>;

    /// Remove every local credential and cached account data, and end the session remotely
    /// when possible.
    fn sign_out(&self) -> BoxFuture<'_, Result<SignOutReport, AccountError>>;
}

/// An account service that is never signed in. Used by tests and when sign-in is unavailable.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoAccount;

impl AccountService for NoAccount {
    fn state(&self) -> AccountState {
        AccountState::SignedOut
    }

    fn sign_in(&self) -> BoxFuture<'_, Result<AccountInfo, AccountError>> {
        Box::pin(async { Err(AccountError::Unavailable("not built in".to_owned())) })
    }

    fn sign_in_with_cookie(
        &self,
        _cookie: Secret<String>,
    ) -> BoxFuture<'_, Result<AccountInfo, AccountError>> {
        Box::pin(async { Err(AccountError::Unavailable("not built in".to_owned())) })
    }

    fn sign_out(&self) -> BoxFuture<'_, Result<SignOutReport, AccountError>> {
        Box::pin(async { Ok(SignOutReport::default()) })
    }
}
