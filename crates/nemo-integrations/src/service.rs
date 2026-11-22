//! The integrations service: the switches the settings screen flips, and the fan-out of play
//! events to the workers.
//!
//! Discord presence is built (discord-presence spec): one worker thread that exists only while
//! its switch is on. Media controls and Last.fm are still switches without a worker: Last.fm is
//! unavailable and its events are dropped.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::hooks::{PlayHookEvent, PlayObserver};
use nemo_core::integrations::{
    BrowsingSection, IntegrationError, IntegrationsService, LastFmStatus, NativeWindow,
    UpdateNotice, WindowContext, is_newer,
};
use nemo_core::opt_in::OptInInfo;
use nemo_core::secret::Secret;

use crate::discord::ipc::{Connector, IpcConnector};
use crate::discord::{Browsing, Config, Input, Place, Timing, Worker};
use crate::info;
use crate::store::{
    META_DISCORD, META_DISCORD_CONFIRMED, META_DISCORD_PRIVATE, META_MEDIA_CONTROLS, Store,
};

/// The Discord application id the owner registered (2026-11-15). It is public: Discord shows
/// it to anyone who sees the presence, so it is not a secret. While it is empty, Discord
/// presence is neither offered nor started.
pub const DISCORD_APPLICATION_ID: &str = "1571523010022735972";

const LASTFM_NOT_BUILT: &str = "Last.fm is not built yet";
/// How long the app waits for the presence to clear when it ends.
const EXIT_WAIT: Duration = Duration::from_millis(500);

/// Where the Discord worker connects, and its times.
struct DiscordLink {
    connector: Arc<dyn Connector>,
    timing: Timing,
}

impl std::fmt::Debug for DiscordLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscordLink").finish_non_exhaustive()
    }
}

/// What the worker is told besides play events.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Facts {
    window: WindowContext,
    private: bool,
    place: Place,
}

/// OS media controls, Last.fm and Discord presence behind one service.
#[derive(Debug)]
pub struct Integrations {
    media_controls: AtomicBool,
    discord: AtomicBool,
    store: Option<Store>,
    link: Option<DiscordLink>,
    worker: Mutex<Option<Worker>>,
    facts: Mutex<Facts>,
}

/// Build the integrations service with no database and no connection to Discord: every switch
/// lives in memory and nothing leaves the computer. Media controls start on (criterion 9),
/// Discord and Last.fm off.
#[must_use]
pub fn build() -> Integrations {
    build_with(None, None)
}

/// Build the service for the app: the switches are kept in `integrations.sqlite3` inside `dir`
/// (without it they live in memory), and Discord presence talks to the Discord app on this
/// computer while its switch is on and confirmed.
#[must_use]
pub fn build_in(dir: &Path) -> Integrations {
    let store = match Store::open(dir) {
        Ok(store) => Some(store),
        Err(err) => {
            tracing::warn!(error = %err, "cannot open the integrations database; switches are not kept");
            None
        }
    };
    let link = DiscordLink {
        connector: Arc::new(IpcConnector::system()),
        timing: Timing::default(),
    };
    build_with(store, Some(link))
}

/// Build the service with a connector of your choice (tests use a fake Discord).
#[must_use]
pub fn build_with_connector(
    store: Option<Store>,
    connector: Arc<dyn Connector>,
    timing: Timing,
) -> Integrations {
    build_with(store, Some(DiscordLink { connector, timing }))
}

fn build_with(store: Option<Store>, link: Option<DiscordLink>) -> Integrations {
    let stored = |key: &str, default: bool| {
        store
            .as_ref()
            .and_then(|s| s.switch(key, default).ok())
            .unwrap_or(default)
    };
    let confirmed = store
        .as_ref()
        .and_then(|s| s.meta_get(META_DISCORD_CONFIRMED).ok().flatten())
        .is_some_and(|v| v == info::DISCORD.version.to_string());
    let discord_on = discord_offered() && confirmed && stored(META_DISCORD, false);
    let this = Integrations {
        media_controls: AtomicBool::new(stored(
            META_MEDIA_CONTROLS,
            info::MEDIA_CONTROLS.default_on,
        )),
        discord: AtomicBool::new(discord_on),
        facts: Mutex::new(Facts {
            window: WindowContext::default(),
            private: stored(META_DISCORD_PRIVATE, false),
            place: Place::ThisDevice,
        }),
        store,
        link,
        worker: Mutex::new(None),
    };
    if discord_on {
        this.start_discord();
    }
    this
}

/// Whether a Discord application id is built in.
const fn discord_offered() -> bool {
    !DISCORD_APPLICATION_ID.is_empty()
}

fn lastfm_unavailable() -> IntegrationError {
    IntegrationError::Unavailable(LASTFM_NOT_BUILT.to_owned())
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

const fn browsing(section: Option<BrowsingSection>) -> Option<Browsing> {
    match section {
        Some(BrowsingSection::Library) => Some(Browsing::Library),
        Some(BrowsingSection::Search) => Some(Browsing::Search),
        None => None,
    }
}

impl Integrations {
    fn facts(&self) -> Facts {
        *lock(&self.facts)
    }

    fn remember(&self, key: &str, on: bool) {
        if let Some(store) = &self.store
            && let Err(err) = store.set_switch(key, on)
        {
            tracing::warn!(error = %err, "cannot keep an integrations switch");
        }
    }

    /// Start the Discord worker with the facts known now. Does nothing without a connector or
    /// when it already runs.
    fn start_discord(&self) {
        let Some(link) = &self.link else { return };
        let mut slot = lock(&self.worker);
        if slot.is_some() {
            return;
        }
        let config = Config {
            client_id: DISCORD_APPLICATION_ID.to_owned(),
            timing: link.timing,
            connector: Arc::clone(&link.connector),
        };
        match Worker::start(config) {
            Ok(worker) => {
                let facts = self.facts();
                worker.send(Input::Private(facts.private));
                worker.send(Input::Place(facts.place));
                worker.send(Input::Window {
                    active: facts.window.focused,
                });
                worker.send(Input::Section(browsing(facts.window.section)));
                worker.send(Input::SleepTimer(facts.window.sleep_timer));
                *slot = Some(worker);
            }
            Err(err) => tracing::warn!(error = %err, "cannot start the Discord presence thread"),
        }
    }

    /// End the Discord worker: it clears the presence and closes the connection by itself.
    fn stop_discord(&self, wait: Duration) {
        let worker = lock(&self.worker).take();
        if let Some(worker) = worker {
            worker.stop(wait);
        }
    }

    fn tell_discord(&self, input: Input) {
        if let Some(worker) = lock(&self.worker).as_ref() {
            worker.send(input);
        }
    }

    /// The sound moved to a Cast device or a Listen Together room, or back. Nothing sends the
    /// name of the device or the room.
    pub fn set_place(&self, place: Place) {
        let changed = {
            let mut facts = lock(&self.facts);
            let changed = facts.place != place;
            facts.place = place;
            changed
        };
        if changed {
            self.tell_discord(Input::Place(place));
        }
    }
}

impl Drop for Integrations {
    fn drop(&mut self) {
        self.stop_discord(EXIT_WAIT);
    }
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
        self.remember(META_MEDIA_CONTROLS, on);
    }

    fn discord(&self) -> bool {
        discord_offered() && self.discord.load(Ordering::Relaxed)
    }

    fn set_discord(&self, on: bool) {
        let on = on && discord_offered();
        self.discord.store(on, Ordering::Relaxed);
        self.remember(META_DISCORD, on);
        if on {
            // The switch counts only with the version of the notice the user confirmed.
            if let Some(store) = &self.store
                && let Err(err) =
                    store.meta_set(META_DISCORD_CONFIRMED, &info::DISCORD.version.to_string())
            {
                tracing::warn!(error = %err, "cannot keep the confirmed Discord notice");
            }
            self.start_discord();
        } else {
            self.stop_discord(Duration::ZERO);
        }
    }

    fn discord_private(&self) -> bool {
        self.facts().private
    }

    fn set_discord_private(&self, on: bool) {
        lock(&self.facts).private = on;
        self.remember(META_DISCORD_PRIVATE, on);
        self.tell_discord(Input::Private(on));
    }

    fn window_context(&self, context: WindowContext) {
        let before = {
            let mut facts = lock(&self.facts);
            let before = facts.window;
            if before == context {
                return;
            }
            facts.window = context;
            before
        };
        if before.focused != context.focused {
            self.tell_discord(Input::Window {
                active: context.focused,
            });
        }
        if before.section != context.section {
            self.tell_discord(Input::Section(browsing(context.section)));
        }
        if before.sleep_timer != context.sleep_timer {
            self.tell_discord(Input::SleepTimer(context.sleep_timer));
        }
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

    fn check_update(
        &self,
        current: &str,
        dismissed: Option<&str>,
    ) -> BoxFuture<'static, Option<UpdateNotice>> {
        let current = current.to_owned();
        let dismissed = dismissed.map(ToOwned::to_owned);
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .ok()?;
            let url = "https://api.github.com/repos/nekoo-moe/NemoMusic/releases/latest";
            let resp = client
                .get(url)
                .header(reqwest::header::USER_AGENT, format!("NemoMusic/{current}"))
                .send()
                .await
                .ok()?;
            if !resp.status().is_success() {
                return None;
            }
            let text = resp.text().await.ok()?;
            let val: serde_json::Value = serde_json::from_str(&text).ok()?;
            let tag_name = val.get("tag_name")?.as_str()?;
            let html_url = val.get("html_url")?.as_str()?;
            if is_newer(&current, tag_name, dismissed.as_deref()) {
                Some(UpdateNotice {
                    version: tag_name.to_owned(),
                    url: html_url.to_owned(),
                })
            } else {
                None
            }
        })
    }
}

impl PlayObserver for Integrations {
    /// Hands the event to each running worker without blocking. With every switch off no
    /// worker runs and the event is dropped.
    fn on_event(&self, event: &PlayHookEvent) {
        self.tell_discord(Input::Play(Box::new(event.clone())));
    }
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
    fn discord_is_offered_with_the_built_in_application_id_and_starts_off() {
        assert!(!DISCORD_APPLICATION_ID.is_empty());
        assert!(DISCORD_APPLICATION_ID.bytes().all(|b| b.is_ascii_digit()));
        let s = build();
        assert!(!s.discord());
        let ids: Vec<&str> = s.infos().iter().map(|i| i.id).collect();
        assert_eq!(ids, ["media_controls", "lastfm", "discord"]);
        s.set_discord(true);
        assert!(s.discord());
        s.set_discord(false);
        assert!(!s.discord());
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

#[cfg(test)]
mod discord_tests;
