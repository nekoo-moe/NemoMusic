//! The contract of the integrations service: OS media controls, Last.fm and Discord presence.
//!
//! No I/O here. `nemo-integrations` implements [`IntegrationsService`]; the UI reads and flips
//! the switches through it and draws each row from the [`OptInInfo`] it returns.
//! [`NoIntegrations`] stands in until the app builds the real service.

use crate::BoxFuture;
use crate::opt_in::OptInInfo;
use crate::secret::Secret;

/// A native window handle for the OS media controls: the `HWND` on Windows, unused elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NativeWindow(pub usize);

/// Where the Last.fm connection stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LastFmStatus {
    /// No API key and secret are stored.
    #[default]
    NoKeys,
    /// Keys are stored; the user can connect.
    Ready,
    /// The authorization page was opened; waiting for the user to approve it.
    AwaitingApproval,
    /// Connected as this Last.fm user (a public name, not a secret).
    Connected {
        /// The Last.fm user name.
        user: String,
    },
    /// Last.fm refused the session; the user must connect again. The queue is kept.
    NeedsReconnect,
}

/// An integration request failed. The texts hold no keys, session keys or request bodies.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IntegrationError {
    /// The integration is not available on this computer or in this build.
    #[error("not available: {0}")]
    Unavailable(String),
    /// The network request failed.
    #[error("network error: {0}")]
    Network(String),
    /// The service refused the request.
    #[error("refused: {0}")]
    Rejected(String),
    /// Local storage (the credential store or the queue database) failed.
    #[error("storage error: {0}")]
    Storage(String),
}

/// OS media controls, Last.fm scrobbling and Discord presence, as the settings screen uses them.
pub trait IntegrationsService: Send + Sync {
    /// The opt-in rows of the integrations, in display order.
    fn infos(&self) -> Vec<OptInInfo>;

    /// Whether OS media controls are on.
    fn media_controls(&self) -> bool;

    /// Switch OS media controls on or off; the choice is stored.
    fn set_media_controls(&self, on: bool);

    /// Whether Discord presence is on. `false` and not offered while no application id is
    /// built in.
    fn discord(&self) -> bool;

    /// Switch Discord presence on or off; the choice is stored.
    fn set_discord(&self, on: bool);

    /// Where the Last.fm connection stands.
    fn lastfm_status(&self) -> LastFmStatus;

    /// Scrobbles waiting to be sent.
    fn lastfm_queue_len(&self) -> usize;

    /// Store the user's Last.fm API key and secret in the credential store.
    ///
    /// # Errors
    /// [`IntegrationError::Storage`] when the credential store fails, or
    /// [`IntegrationError::Unavailable`] when Last.fm is not available.
    fn lastfm_save_keys(
        &self,
        key: Secret<String>,
        secret: Secret<String>,
    ) -> Result<(), IntegrationError>;

    /// Start connecting: returns the authorization URL to open in the browser.
    ///
    /// # Errors
    /// An [`IntegrationError`] when no keys are stored or Last.fm cannot be reached.
    fn lastfm_begin(&self) -> BoxFuture<'_, Result<String, IntegrationError>>;

    /// Finish connecting after the user approved: returns the Last.fm user name.
    ///
    /// # Errors
    /// An [`IntegrationError`] when Last.fm refuses the session or cannot be reached.
    fn lastfm_finish(&self) -> BoxFuture<'_, Result<String, IntegrationError>>;

    /// Disconnect: deletes the session key and the queue.
    ///
    /// # Errors
    /// [`IntegrationError::Storage`] when the session key or the queue cannot be deleted.
    fn lastfm_disconnect(&self) -> BoxFuture<'_, Result<(), IntegrationError>>;

    /// The main window exists; OS media controls attach to it.
    fn window_ready(&self, window: NativeWindow);
}

/// An integrations service with nothing behind it: every switch is off, nothing is offered and
/// every Last.fm request fails with [`IntegrationError::Unavailable`].
#[derive(Debug, Clone, Copy, Default)]
pub struct NoIntegrations;

const NOT_BUILT: &str = "integrations are not built in";

fn unavailable() -> IntegrationError {
    IntegrationError::Unavailable(NOT_BUILT.to_owned())
}

impl IntegrationsService for NoIntegrations {
    fn infos(&self) -> Vec<OptInInfo> {
        Vec::new()
    }

    fn media_controls(&self) -> bool {
        false
    }

    fn set_media_controls(&self, _on: bool) {}

    fn discord(&self) -> bool {
        false
    }

    fn set_discord(&self, _on: bool) {}

    fn lastfm_status(&self) -> LastFmStatus {
        LastFmStatus::NoKeys
    }

    fn lastfm_queue_len(&self) -> usize {
        0
    }

    fn lastfm_save_keys(
        &self,
        _key: Secret<String>,
        _secret: Secret<String>,
    ) -> Result<(), IntegrationError> {
        Err(unavailable())
    }

    fn lastfm_begin(&self) -> BoxFuture<'_, Result<String, IntegrationError>> {
        Box::pin(async { Err(unavailable()) })
    }

    fn lastfm_finish(&self) -> BoxFuture<'_, Result<String, IntegrationError>> {
        Box::pin(async { Err(unavailable()) })
    }

    fn lastfm_disconnect(&self) -> BoxFuture<'_, Result<(), IntegrationError>> {
        Box::pin(async { Err(unavailable()) })
    }

    fn window_ready(&self, _window: NativeWindow) {}
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use super::{
        IntegrationError, IntegrationsService, LastFmStatus, NativeWindow, NoIntegrations,
    };
    use crate::secret::Secret;

    /// Poll a future that is ready at once.
    fn ready<T>(fut: impl Future<Output = T>) -> Option<T> {
        match pin!(fut).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(v) => Some(v),
            Poll::Pending => None,
        }
    }

    #[test]
    fn no_integrations_offers_nothing_and_stays_off() {
        let s = NoIntegrations;
        assert!(s.infos().is_empty());
        s.set_media_controls(true);
        s.set_discord(true);
        s.window_ready(NativeWindow(1));
        assert!(!s.media_controls() && !s.discord());
        assert_eq!(s.lastfm_status(), LastFmStatus::NoKeys);
        assert_eq!(s.lastfm_queue_len(), 0);
    }

    #[test]
    fn no_integrations_refuses_every_lastfm_request() {
        let s = NoIntegrations;
        let keys = s.lastfm_save_keys(Secret::new("k".to_owned()), Secret::new("s".to_owned()));
        assert!(matches!(keys, Err(IntegrationError::Unavailable(_))));
        assert!(matches!(
            ready(s.lastfm_begin()),
            Some(Err(IntegrationError::Unavailable(_)))
        ));
        assert!(matches!(
            ready(s.lastfm_finish()),
            Some(Err(IntegrationError::Unavailable(_)))
        ));
        assert!(matches!(
            ready(s.lastfm_disconnect()),
            Some(Err(IntegrationError::Unavailable(_)))
        ));
    }
}
