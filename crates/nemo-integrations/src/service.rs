//! The integrations service: the switches the settings screen flips, and the fan-out of play
//! events to the workers.
//!
//! Skeleton: no worker exists yet, so the switches live in memory, Last.fm is unavailable and
//! the observer drops every event. Later tasks add the workers and the parameters of `build`
//! the design lists (paths, secret store, player, images, runtime handle).

use std::sync::atomic::{AtomicBool, Ordering};

use nemo_core::BoxFuture;
use nemo_core::hooks::{PlayHookEvent, PlayObserver};
use nemo_core::integrations::{IntegrationError, IntegrationsService, LastFmStatus, NativeWindow};
use nemo_core::opt_in::OptInInfo;
use nemo_core::secret::Secret;

use crate::info;

/// The Discord application id. Empty until the owner registers the application; while it is
/// empty Discord presence is neither offered nor started.
pub const DISCORD_APPLICATION_ID: &str = "";

const LASTFM_NOT_BUILT: &str = "Last.fm is not built yet";

/// OS media controls, Last.fm and Discord presence behind one service.
#[derive(Debug)]
pub struct Integrations {
    media_controls: AtomicBool,
    discord: AtomicBool,
}

/// Build the integrations service. Media controls start on (criterion 9), Discord and Last.fm
/// off.
#[must_use]
pub fn build() -> Integrations {
    Integrations {
        media_controls: AtomicBool::new(info::MEDIA_CONTROLS.default_on),
        discord: AtomicBool::new(false),
    }
}

/// Whether a Discord application id is built in.
const fn discord_offered() -> bool {
    !DISCORD_APPLICATION_ID.is_empty()
}

fn lastfm_unavailable() -> IntegrationError {
    IntegrationError::Unavailable(LASTFM_NOT_BUILT.to_owned())
}

impl IntegrationsService for Integrations {
    fn infos(&self) -> Vec<OptInInfo> {
        let mut rows = vec![info::MEDIA_CONTROLS, info::LASTFM];
        if discord_offered() {
            rows.push(info::DISCORD);
        }
        rows
    }

    fn media_controls(&self) -> bool {
        self.media_controls.load(Ordering::Relaxed)
    }

    fn set_media_controls(&self, on: bool) {
        self.media_controls.store(on, Ordering::Relaxed);
    }

    fn discord(&self) -> bool {
        discord_offered() && self.discord.load(Ordering::Relaxed)
    }

    fn set_discord(&self, on: bool) {
        self.discord
            .store(on && discord_offered(), Ordering::Relaxed);
    }

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
        Err(lastfm_unavailable())
    }

    fn lastfm_begin(&self) -> BoxFuture<'_, Result<String, IntegrationError>> {
        Box::pin(async { Err(lastfm_unavailable()) })
    }

    fn lastfm_finish(&self) -> BoxFuture<'_, Result<String, IntegrationError>> {
        Box::pin(async { Err(lastfm_unavailable()) })
    }

    fn lastfm_disconnect(&self) -> BoxFuture<'_, Result<(), IntegrationError>> {
        Box::pin(async { Err(lastfm_unavailable()) })
    }

    fn window_ready(&self, _window: NativeWindow) {}
}

impl PlayObserver for Integrations {
    /// Hands the event to each running worker without blocking. No worker runs yet, so the
    /// event is dropped.
    fn on_event(&self, _event: &PlayHookEvent) {}
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use nemo_core::hooks::{PlayHookEvent, PlayObserver};
    use nemo_core::integrations::{IntegrationsService, LastFmStatus};
    use nemo_core::secret::Secret;

    use super::{DISCORD_APPLICATION_ID, build};

    #[test]
    fn media_controls_start_on_and_can_be_turned_off() {
        let s = build();
        assert!(s.media_controls());
        s.set_media_controls(false);
        assert!(!s.media_controls());
    }

    #[test]
    fn discord_is_not_offered_without_an_application_id() {
        assert!(DISCORD_APPLICATION_ID.is_empty());
        let s = build();
        s.set_discord(true);
        assert!(!s.discord());
        let ids: Vec<&str> = s.infos().iter().map(|i| i.id).collect();
        assert_eq!(ids, ["media_controls", "lastfm"]);
    }

    #[test]
    fn lastfm_is_off_and_refuses_keys_until_it_is_built() {
        let s = build();
        assert_eq!(s.lastfm_status(), LastFmStatus::NoKeys);
        assert_eq!(s.lastfm_queue_len(), 0);
        let keys = s.lastfm_save_keys(Secret::new("k".into()), Secret::new("s".into()));
        assert!(keys.is_err());
        s.on_event(&PlayHookEvent::DurationKnown(Duration::from_secs(1)));
    }
}
