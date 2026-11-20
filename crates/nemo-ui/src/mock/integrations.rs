//! A fake integrations service for the Settings screens and their tests: only the Discord
//! switches do anything, and the window facts it is told are kept for the test to read. The
//! rest answers like [`NoIntegrations`].

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use nemo_core::BoxFuture;
use nemo_core::integrations::{
    IntegrationError, IntegrationsService, LastFmStatus, NativeWindow, NoIntegrations,
    WindowContext,
};
use nemo_core::opt_in::OptInInfo;
use nemo_core::secret::Secret;

/// Offers one opt-in row, `info`, and keeps its switches in memory.
#[derive(Debug)]
pub struct MockIntegrations {
    info: OptInInfo,
    discord: AtomicBool,
    private: AtomicBool,
    window: Mutex<Option<WindowContext>>,
}

impl MockIntegrations {
    /// A service that offers `info` as the Discord row, with both switches off.
    #[must_use]
    pub const fn new(info: OptInInfo) -> Self {
        Self {
            info,
            discord: AtomicBool::new(false),
            private: AtomicBool::new(false),
            window: Mutex::new(None),
        }
    }

    /// The last window facts the UI reported, if any.
    #[must_use]
    pub fn last_window(&self) -> Option<WindowContext> {
        self.window.lock().ok().and_then(|w| *w)
    }
}

impl IntegrationsService for MockIntegrations {
    fn infos(&self) -> Vec<OptInInfo> {
        vec![self.info]
    }

    fn media_controls(&self) -> bool {
        false
    }

    fn set_media_controls(&self, _on: bool) {}

    fn discord(&self) -> bool {
        self.discord.load(Ordering::Relaxed)
    }

    fn set_discord(&self, on: bool) {
        self.discord.store(on, Ordering::Relaxed);
    }

    fn discord_private(&self) -> bool {
        self.private.load(Ordering::Relaxed)
    }

    fn set_discord_private(&self, on: bool) {
        self.private.store(on, Ordering::Relaxed);
    }

    fn window_context(&self, context: WindowContext) {
        if let Ok(mut last) = self.window.lock() {
            *last = Some(context);
        }
    }

    fn lastfm_status(&self) -> LastFmStatus {
        NoIntegrations.lastfm_status()
    }

    fn lastfm_queue_len(&self) -> usize {
        0
    }

    fn lastfm_save_keys(
        &self,
        key: Secret<String>,
        secret: Secret<String>,
    ) -> Result<(), IntegrationError> {
        NoIntegrations.lastfm_save_keys(key, secret)
    }

    fn lastfm_begin(&self) -> BoxFuture<'_, Result<String, IntegrationError>> {
        NoIntegrations.lastfm_begin()
    }

    fn lastfm_finish(&self) -> BoxFuture<'_, Result<String, IntegrationError>> {
        NoIntegrations.lastfm_finish()
    }

    fn lastfm_disconnect(&self) -> BoxFuture<'_, Result<(), IntegrationError>> {
        NoIntegrations.lastfm_disconnect()
    }

    fn window_ready(&self, _window: NativeWindow) {}
}
