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
    /// Already [`MAX_ACCOUNTS`] accounts are signed in.
    #[error("no more accounts can be added; sign one out first")]
    LimitReached,
    /// No signed-in account has this id.
    #[error("this account is not signed in")]
    UnknownAccount,
}

/// The most accounts that can be signed in at once.
pub const MAX_ACCOUNTS: usize = 5;

/// Whether an account has YouTube Music Premium.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PremiumStatus {
    /// Not checked yet, or the check failed.
    #[default]
    Unknown,
    /// The account has Premium.
    Yes,
    /// The account has no Premium.
    No,
}

/// One signed-in account as the account switcher shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountEntry {
    /// The local slot of the account.
    pub id: AccountId,
    /// What the UI may show about it.
    pub info: AccountInfo,
    /// Whether it has Premium.
    pub premium: PremiumStatus,
}

/// Every signed-in account and which one is active.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccountList {
    /// The signed-in accounts, at most [`MAX_ACCOUNTS`].
    pub accounts: Vec<AccountEntry>,
    /// The active account, if anyone is signed in.
    pub active: Option<AccountId>,
    /// Raised on every switch. An answer to a request made under an older generation is
    /// dropped.
    pub generation: u64,
}

/// Several signed-in accounts and switching between them.
pub trait AccountSwitcher: Send + Sync {
    /// The accounts now. Cheap; safe to call every frame.
    fn list(&self) -> AccountList;

    /// Open the sign-in window for one more account; the others stay signed in.
    ///
    /// # Errors
    /// [`AccountError::LimitReached`] when [`MAX_ACCOUNTS`] are signed in, or the errors of a
    /// sign-in.
    fn add_account(&self) -> BoxFuture<'_, Result<AccountEntry, AccountError>>;

    /// Make `id` the active account. Returns the list with the raised generation.
    ///
    /// # Errors
    /// [`AccountError::UnknownAccount`] when no signed-in account has this id, or
    /// [`AccountError::Storage`] when its session cannot be read.
    fn switch_to(&self, id: AccountId) -> BoxFuture<'_, Result<AccountList, AccountError>>;

    /// Sign out one account and delete its data; the others stay signed in.
    ///
    /// # Errors
    /// [`AccountError::UnknownAccount`] when no signed-in account has this id, or the errors of
    /// a sign-out.
    fn sign_out_account(&self, id: AccountId)
    -> BoxFuture<'_, Result<SignOutReport, AccountError>>;
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
    /// Items the registered [`SignOutWipe`] steps removed, all steps together.
    pub items_removed: usize,
    /// Names of the [`SignOutWipe`] steps that failed. Sign-out still ran the others.
    pub failed_steps: Vec<&'static str>,
}

impl SignOutReport {
    /// Run one sign-out step for `scope` and add its outcome to the report: the items it
    /// removed, or its name when it failed. Never stops on a failure, so the caller runs every
    /// step.
    pub fn run_step(&mut self, step: &dyn SignOutWipe, scope: WipeScope) {
        match step.wipe(scope) {
            Ok(removed) => self.items_removed = self.items_removed.saturating_add(removed),
            Err(_) => self.failed_steps.push(step.name()),
        }
    }
}

/// One local account slot. A number this app gives the account, not a Google id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AccountId(pub u8);

/// Which account data a sign-out step deletes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WipeScope {
    /// Only the data of this account.
    Account(AccountId),
    /// The data of every account.
    All,
}

/// A sign-out step failed. The text is fixed: it never holds a path, name or id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct WipeError(pub &'static str);

/// One step of sign-out. Each crate that keeps account data registers one with the account
/// service, which runs every step when the user signs out (ADR 0004, Decision 3).
pub trait SignOutWipe: Send + Sync {
    /// Short, fixed name of the step, such as `"offline"`. Shown when the step fails.
    fn name(&self) -> &'static str;

    /// Delete the account data this step owns for `scope`. Returns how many items it removed.
    ///
    /// # Errors
    /// Returns a [`WipeError`] when some data could not be deleted.
    fn wipe(&self, scope: WipeScope) -> Result<usize, WipeError>;
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

impl AccountSwitcher for NoAccount {
    fn list(&self) -> AccountList {
        AccountList::default()
    }

    fn add_account(&self) -> BoxFuture<'_, Result<AccountEntry, AccountError>> {
        Box::pin(async { Err(AccountError::Unavailable("not built in".to_owned())) })
    }

    fn switch_to(&self, _id: AccountId) -> BoxFuture<'_, Result<AccountList, AccountError>> {
        Box::pin(async { Err(AccountError::UnknownAccount) })
    }

    fn sign_out_account(
        &self,
        _id: AccountId,
    ) -> BoxFuture<'_, Result<SignOutReport, AccountError>> {
        Box::pin(async { Err(AccountError::UnknownAccount) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Step {
        name: &'static str,
        result: Result<usize, WipeError>,
    }

    impl SignOutWipe for Step {
        fn name(&self) -> &'static str {
            self.name
        }

        fn wipe(&self, _scope: WipeScope) -> Result<usize, WipeError> {
            self.result
        }
    }

    #[test]
    fn step_names_and_counts_reach_the_report() {
        let steps = [
            Step {
                name: "offline",
                result: Ok(3),
            },
            Step {
                name: "ai",
                result: Err(WipeError("cannot delete")),
            },
            Step {
                name: "cast",
                result: Ok(2),
            },
        ];
        let mut report = SignOutReport::default();
        for step in &steps {
            report.run_step(step, WipeScope::Account(AccountId(1)));
        }
        assert_eq!(report.items_removed, 5);
        assert_eq!(report.failed_steps, vec!["ai"]);
    }

    #[test]
    fn a_wipe_error_shows_only_its_fixed_text() {
        assert_eq!(WipeError("cannot delete").to_string(), "cannot delete");
    }

    fn ready<T>(fut: impl std::future::Future<Output = T>) -> Option<T> {
        use std::task::{Context, Poll, Waker};
        match std::pin::pin!(fut).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(v) => Some(v),
            Poll::Pending => None,
        }
    }

    #[test]
    fn no_account_has_an_empty_list_and_switches_nowhere() {
        let list = NoAccount.list();
        assert_eq!(list, AccountList::default());
        assert!(list.accounts.is_empty() && list.active.is_none());
        assert_eq!(list.generation, 0);
        assert_eq!(PremiumStatus::default(), PremiumStatus::Unknown);
        assert_eq!(
            ready(NoAccount.switch_to(AccountId(0))),
            Some(Err(AccountError::UnknownAccount))
        );
        assert_eq!(
            ready(NoAccount.sign_out_account(AccountId(0))),
            Some(Err(AccountError::UnknownAccount))
        );
        assert!(matches!(
            ready(NoAccount.add_account()),
            Some(Err(AccountError::Unavailable(_)))
        ));
        assert_eq!(MAX_ACCOUNTS, 5);
    }

    #[test]
    fn account_list_errors_hold_no_name_or_id() {
        let texts = [
            AccountError::LimitReached.to_string(),
            AccountError::UnknownAccount.to_string(),
        ];
        assert_eq!(
            texts,
            [
                "no more accounts can be added; sign one out first",
                "this account is not signed in"
            ]
        );
        for t in texts {
            assert!(!t.contains('@') && !t.chars().any(|c| c.is_ascii_digit()));
        }
    }
}
