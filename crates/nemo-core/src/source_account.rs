//! Sign-in to a second source such as SoundCloud (soundcloud-account spec, ADR 0005).
//!
//! The Google account of YouTube Music has its own contract in [`crate::account`]. A source
//! account is smaller: one session token that lives in the OS credential store, a display name
//! and an avatar. Implementations never log the token, the name or an id.

use crate::BoxFuture;
use crate::account::SignOutReport;
use crate::secret::Secret;
use crate::track::Thumbnail;

/// What the UI may show about the signed-in account. No ids, no tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAccountInfo {
    /// Display name, as the source shows it.
    pub name: String,
    /// Avatar sizes.
    pub avatar: Vec<Thumbnail>,
}

/// Sign-in state of a source account.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SourceAccountState {
    /// Nobody is signed in. The source works for public items.
    #[default]
    SignedOut,
    /// The sign-in window is open, or a token is being checked.
    SigningIn,
    /// Signed in. The library and the account's own reads come from this account.
    SignedIn(SourceAccountInfo),
    /// The source rejected the session. The token was removed from this computer; the user signs
    /// in again. Public items keep working.
    Expired,
}

/// Signing in or out failed. Messages are short and user-safe: never a token, a name or an id.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceAccountError {
    /// The user closed the sign-in window before finishing.
    #[error("sign-in was cancelled")]
    Cancelled,
    /// The source did not accept this sign-in.
    #[error("this sign-in was not accepted")]
    Rejected,
    /// The sign-in helper or the webview runtime is missing.
    #[error("the sign-in window is not available: {0}")]
    Unavailable(String),
    /// The OS credential store failed.
    #[error("cannot store the sign-in on this computer: {0}")]
    Storage(String),
    /// The network request failed, or the source asked to slow down.
    #[error("network error: {0}")]
    Network(String),
}

/// Sign-in to one source.
pub trait SourceAccount: Send + Sync {
    /// The state now. Cheap; safe to call every frame.
    fn state(&self) -> SourceAccountState;

    /// Open the source's own sign-in page in the sign-in window and wait for the user to finish.
    fn sign_in(&self) -> BoxFuture<'_, Result<SourceAccountInfo, SourceAccountError>>;

    /// Whether the sign-in window has waited long without finding a session, so the app offers
    /// "I am signed in, check again" next to the token paste. Cheap; `false` when no window is
    /// open and for sources whose window never says so.
    fn sign_in_hint(&self) -> bool {
        false
    }

    /// Ask the open sign-in window to look for the session now. Does nothing without a window.
    fn recheck_sign_in(&self) {}

    /// Sign in with a token the user copied from their own browser (fallback when the window is
    /// blocked). The value is checked, then kept only in the credential store. A sign-in window
    /// that is still open is closed first; its own call then ends with
    /// [`SourceAccountError::Cancelled`].
    fn sign_in_with_token(
        &self,
        token: Secret<String>,
    ) -> BoxFuture<'_, Result<SourceAccountInfo, SourceAccountError>>;

    /// Delete the token and every cached account datum. Keeps going after a failed step and
    /// names it in the report.
    fn sign_out(&self) -> BoxFuture<'_, Result<SignOutReport, SourceAccountError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_state_is_signed_out() {
        assert_eq!(SourceAccountState::default(), SourceAccountState::SignedOut);
    }

    #[test]
    fn error_texts_hold_no_secret_shaped_value() {
        for e in [
            SourceAccountError::Cancelled,
            SourceAccountError::Rejected,
            SourceAccountError::Unavailable("the sign-in helper cannot be started".to_owned()),
        ] {
            let text = e.to_string();
            assert!(!text.contains("OAuth") && !text.chars().any(|c| c.is_ascii_digit()));
        }
    }
}
