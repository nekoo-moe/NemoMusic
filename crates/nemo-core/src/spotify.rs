//! The user's Spotify session, used by the Spotify lyrics provider and Canvas.
//!
//! No I/O here. `nemo-spotify` implements [`SpotifyAccount`]. The `sp_dc` session cookie is a
//! full web session: it travels only as [`Secret<String>`] and lives only in the OS credential
//! store, so no type here can print it.

use crate::BoxFuture;
use crate::secret::Secret;

/// Where the Spotify session stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SpotifyState {
    /// No session is stored.
    #[default]
    SignedOut,
    /// The sign-in window is open or a pasted cookie is being checked.
    SigningIn,
    /// A session is stored and works.
    SignedIn,
    /// The last sign-in or token request failed. Short, user-safe text; never the cookie or a
    /// token.
    Failed(String),
}

/// A Spotify request failed. The texts hold no cookie, token or URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SpotifyError {
    /// No Spotify session is stored.
    #[error("not signed in to Spotify")]
    SignedOut,
    /// Spotify refused to give an access token for the stored session.
    #[error("Spotify refused the session")]
    TokenRefused,
    /// Spotify answered with this HTTP status.
    #[error("Spotify answered with HTTP status {0}")]
    Http(u16),
    /// The answer could not be read.
    #[error("the Spotify answer could not be read")]
    Parse,
    /// The OS credential store failed.
    #[error("the Spotify session could not be stored on this computer")]
    Store,
}

/// The user's Spotify session.
pub trait SpotifyAccount: Send + Sync {
    /// Where the session stands. Cheap; safe to call every frame.
    fn state(&self) -> SpotifyState;

    /// Sign in through the sign-in window on Spotify's own page.
    ///
    /// # Errors
    /// A [`SpotifyError`] when the window gives no session, Spotify refuses it, or it cannot
    /// be stored.
    fn sign_in_window(&self) -> BoxFuture<'_, Result<(), SpotifyError>>;

    /// Sign in with the `sp_dc` cookie the user copied from their own browser (fallback).
    ///
    /// # Errors
    /// A [`SpotifyError`] when Spotify refuses the session or it cannot be stored.
    fn sign_in_cookie(&self, sp_dc: Secret<String>) -> BoxFuture<'_, Result<(), SpotifyError>>;

    /// Sign out: delete the stored session, the cached Spotify lyrics, the track map and the
    /// Canvas clips.
    fn sign_out(&self);
}

#[cfg(test)]
mod tests {
    use super::{SpotifyError, SpotifyState};
    use crate::secret::Secret;

    const COOKIE: &str = "AQBsecretcookievalue";

    #[test]
    fn debug_of_every_type_prints_no_secret() {
        let texts = [
            format!("{:?}", Secret::new(COOKIE.to_owned())),
            format!("{:?}", SpotifyState::SignedOut),
            format!("{:?}", SpotifyState::SigningIn),
            format!("{:?}", SpotifyState::SignedIn),
            format!(
                "{:?}",
                SpotifyState::Failed("Spotify refused the session".to_owned())
            ),
            format!("{:?}", SpotifyError::SignedOut),
            format!("{:?}", SpotifyError::TokenRefused),
            format!("{:?}", SpotifyError::Http(401)),
            format!("{:?}", SpotifyError::Parse),
            format!("{:?}", SpotifyError::Store),
        ];
        for text in texts {
            assert!(!text.contains(COOKIE), "{text}");
        }
    }

    #[test]
    fn error_texts_are_fixed() {
        assert_eq!(
            SpotifyError::Http(429).to_string(),
            "Spotify answered with HTTP status 429"
        );
        assert_eq!(SpotifyState::default(), SpotifyState::SignedOut);
    }
}
