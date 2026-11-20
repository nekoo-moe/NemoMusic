//! Building the SoundCloud services (soundcloud-full, ADR 0005): only when the switch is on and
//! the user confirmed the current notice. A closed gate means nothing is built, so no request is
//! made, no host contacted and no SoundCloud entry shown.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use nemo_core::catalog::Catalog;
use nemo_core::secret_store::SecretStore;
use nemo_core::services::StreamOpener;
use nemo_core::source_account::SourceAccount;
use nemo_core::source_writes::SourceWrites;
use nemo_core::url_resolver::UrlResolver;
use nemo_soundcloud::{
    AccountFiles, OPT_IN, ReqwestHttp, ScAccount, ScClient, ScHttp, ScStreamOpener, SessionCaches,
    SignInWindow, SoundcloudCatalog, SoundcloudWrites, streams,
};
use nemo_ytm::url_resolver::YtDlpUrlResolver;
use nemo_ytm::ytdlp::YtDlpResolver;
use tokio::runtime::Handle;

use crate::config::OptInChoice;
use crate::opt_in_gate::build_if_open;
use crate::paths::AppPaths;

mod fallback;

pub use fallback::FallbackResolver;

/// Where the running SoundCloud client is kept, so the settings can switch it off at once (ADR
/// 0004, opt-in rule 6). Shared between the settings callback and the service start-up.
#[derive(Clone, Default)]
pub struct ClientSlot(Arc<Mutex<Option<Arc<ScClient>>>>);

impl ClientSlot {
    fn set(&self, client: Arc<ScClient>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(client);
    }

    /// Switch the client on or off, if it was built. Nothing happens before the services start.
    pub fn apply(&self, open: bool) {
        let client = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(client) = client {
            client.set_enabled(open);
        }
    }
}

/// The services of one SoundCloud client.
pub struct SoundcloudParts {
    /// Search and pages, for `Backend.soundcloud`.
    pub catalog: Arc<dyn Catalog>,
    /// Page address to stream, for the player.
    pub resolver: Arc<dyn UrlResolver>,
    /// Opens the HLS streams the resolver finds.
    pub opener: ScStreamOpener,
    /// The client, shared with the account.
    pub client: Arc<ScClient>,
    /// The sign-out step that drops this session's track records and kept streams.
    pub caches: SessionCaches,
}

/// Build the services when the gate is open. `http` makes the HTTP layer; it is not called while
/// the gate is closed. The client id is looked up in the background, then the media client is
/// built and the media connection opened, so the first play does not wait for them.
///
/// `ytdlp` is asked only when the native lookup cannot read SoundCloud's answer; see
/// [`FallbackResolver`].
pub fn build(
    choices: &BTreeMap<String, OptInChoice>,
    handle: &Handle,
    http: impl FnOnce() -> Option<Arc<dyn ScHttp>>,
    ytdlp: Option<Arc<dyn UrlResolver>>,
) -> Option<SoundcloudParts> {
    build_if_open(choices, &OPT_IN, || {
        let client = Arc::new(ScClient::new(http()?));
        let (resolver, opener) = streams(Arc::clone(&client), handle.clone());
        let caches = SessionCaches::new(Arc::clone(&client), resolver.clone());
        let (warm, early) = (Arc::clone(&client), opener.clone());
        handle.spawn(async move {
            // The client id first. Only once it is known, the media client is built (it loads
            // the system certificates) and the connection to the media host is opened, so the
            // first play pays for neither. A failure only means the first request does the work.
            if warm.warm_up().await.is_ok() {
                let _ = tokio::task::spawn_blocking(move || {
                    let _ = early.warm_up();
                    early.preconnect();
                })
                .await;
            }
        });
        Some(SoundcloudParts {
            catalog: Arc::new(SoundcloudCatalog::new(Arc::clone(&client))),
            resolver: match ytdlp {
                Some(ytdlp) => Arc::new(FallbackResolver::new(Arc::new(resolver), ytdlp)),
                None => Arc::new(resolver),
            },
            opener,
            client,
            caches,
        })
    })
    .flatten()
}

/// The SoundCloud services of the app: the real HTTP layer, and `yt-dlp` (the program the YouTube
/// resolver uses) behind the native resolver. `None` while the gate is closed.
pub fn for_app(
    choices: &BTreeMap<String, OptInChoice>,
    handle: &Handle,
    ytdlp: &YtDlpResolver,
) -> Option<SoundcloudParts> {
    let page_ytdlp: Arc<dyn UrlResolver> = Arc::new(YtDlpUrlResolver::new(ytdlp.clone()));
    build(choices, handle, || real_http(handle), Some(page_ytdlp))
}

/// The SoundCloud account (when SoundCloud runs): the client goes into `slot` so the settings can
/// switch it off, and the account is built with the credential `store`. `None` without
/// SoundCloud or without a credential store, so no sign-in is offered and no token is kept
/// anywhere.
pub fn account_for(
    sc: Option<&SoundcloudParts>,
    store: Option<Arc<dyn SecretStore>>,
    paths: &AppPaths,
    handle: &Handle,
    slot: &ClientSlot,
) -> Option<Arc<dyn SourceAccount>> {
    let parts = sc?;
    slot.set(Arc::clone(&parts.client));
    build_account(parts, store, paths, handle)
}

/// The SoundCloud account: sign-in window, token in the credential `store`, sign-out that also
/// drops the session's caches. The window is this same executable started again (`main`).
pub fn build_account(
    parts: &SoundcloudParts,
    store: Option<Arc<dyn SecretStore>>,
    paths: &AppPaths,
    handle: &Handle,
) -> Option<Arc<dyn SourceAccount>> {
    let store = store?;
    let window = SignInWindow::new(
        crate::backend::signin_program(),
        vec![nemo_signin::SIGNIN_ARG.into()],
        paths.webview_soundcloud.clone(),
    );
    let files = AccountFiles {
        webview_dir: paths.webview_soundcloud.clone(),
        cache_file: paths.account.join("soundcloud.json"),
    };
    let account = ScAccount::new(
        Arc::clone(&parts.client),
        store,
        Arc::new(window),
        files,
        handle.clone(),
    );
    account.register_wipe(Arc::new(parts.caches.clone()));
    Some(Arc::new(account))
}

/// The account's writes (likes, unfollow, playlists) over the one SoundCloud client. They need
/// a signed-in account; without one every write says so and sends nothing.
pub fn writes_for(parts: &SoundcloudParts) -> Arc<dyn SourceWrites> {
    Arc::new(SoundcloudWrites::new(Arc::clone(&parts.client)))
}

/// The player's opener: SoundCloud HLS streams go to their reader, everything else to `files`.
pub fn opener(files: Arc<dyn StreamOpener>, sc: Option<&SoundcloudParts>) -> Arc<dyn StreamOpener> {
    match sc {
        Some(parts) => Arc::new(RoutingOpener::new(files, Arc::new(parts.opener.clone()))),
        None => files,
    }
}

/// The real HTTP layer, or `None` (logged) when the TLS backend cannot start.
pub fn real_http(handle: &Handle) -> Option<Arc<dyn ScHttp>> {
    match ReqwestHttp::new(handle.clone()) {
        Ok(http) => Some(Arc::new(http)),
        Err(err) => {
            tracing::warn!(error = %err, "SoundCloud disabled");
            None
        }
    }
}

/// Sends a SoundCloud HLS stream to its reader and every other stream to the ranged file reader
/// (YouTube Music, and SoundCloud's progressive MP3 files).
pub struct RoutingOpener {
    files: Arc<dyn StreamOpener>,
    soundcloud: Arc<dyn StreamOpener>,
}

impl RoutingOpener {
    /// A router over the two readers.
    pub fn new(files: Arc<dyn StreamOpener>, soundcloud: Arc<dyn StreamOpener>) -> Self {
        Self { files, soundcloud }
    }
}

impl StreamOpener for RoutingOpener {
    fn open(
        &self,
        stream: &nemo_core::stream::ResolvedStream,
    ) -> Result<Box<dyn nemo_core::services::ByteSource>, nemo_core::stream::StreamError> {
        let hls = matches!(stream.transport, nemo_core::stream::Transport::Hls { .. });
        if hls && stream.info.origin == nemo_core::audio::Origin::SoundCloud {
            self.soundcloud.open(stream)
        } else {
            self.files.open(stream)
        }
    }
}

#[cfg(test)]
mod tests;
