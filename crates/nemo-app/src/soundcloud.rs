//! Building the SoundCloud services (soundcloud-full, ADR 0005): only when the switch is on and
//! the user confirmed the current notice. A closed gate means nothing is built, so no request is
//! made, no host contacted and no SoundCloud entry shown.

use std::collections::BTreeMap;
use std::sync::Arc;

use nemo_core::catalog::Catalog;
use nemo_core::secret_store::SecretStore;
use nemo_core::services::StreamOpener;
use nemo_core::source_account::SourceAccount;
use nemo_core::url_resolver::UrlResolver;
use nemo_soundcloud::{
    AccountFiles, OPT_IN, ReqwestHttp, ScAccount, ScClient, ScHttp, ScStreamOpener, SessionCaches,
    SignInWindow, SoundcloudCatalog, streams,
};
use tokio::runtime::Handle;

use crate::config::OptInChoice;
use crate::opt_in_gate::build_if_open;
use crate::paths::AppPaths;

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
/// the gate is closed. The client id is looked up in the background, so the first play does not
/// wait for it.
pub fn build(
    choices: &BTreeMap<String, OptInChoice>,
    handle: &Handle,
    http: impl FnOnce() -> Option<Arc<dyn ScHttp>>,
) -> Option<SoundcloudParts> {
    build_if_open(choices, &OPT_IN, || {
        let client = Arc::new(ScClient::new(http()?));
        let warm = Arc::clone(&client);
        handle.spawn(async move {
            // A failure here only means the id is looked up at the first request instead.
            let _ = warm.warm_up().await;
        });
        let (resolver, opener) = streams(Arc::clone(&client), handle.clone());
        let caches = SessionCaches::new(Arc::clone(&client), resolver.clone());
        let early = opener.clone();
        // Builds the media client (loads the system certificates) before the first play.
        handle.spawn_blocking(move || {
            let _ = early.warm_up();
        });
        Some(SoundcloudParts {
            catalog: Arc::new(SoundcloudCatalog::new(Arc::clone(&client))),
            resolver: Arc::new(resolver),
            opener,
            client,
            caches,
        })
    })
    .flatten()
}

/// What the app takes from SoundCloud, all absent while the gate is closed.
pub struct Wired {
    /// Search and pages, for `Backend.soundcloud`.
    pub catalog: Option<Arc<dyn Catalog>>,
    /// The account, for `Backend.soundcloud_account`.
    pub account: Option<Arc<dyn SourceAccount>>,
    /// Page address to stream, for the player.
    pub resolver: Option<Arc<dyn UrlResolver>>,
    /// The player's opener: `files` alone, or SoundCloud's HLS reader in front of it.
    pub opener: Arc<dyn StreamOpener>,
}

/// Build everything of SoundCloud when the gate is open (see [`build`]), with the account when a
/// credential `store` exists. `files` opens every stream that is not SoundCloud's HLS.
pub fn wire(
    choices: &BTreeMap<String, OptInChoice>,
    handle: &Handle,
    store: Option<Arc<dyn SecretStore>>,
    paths: &AppPaths,
    files: Arc<dyn StreamOpener>,
) -> Wired {
    let Some(parts) = build(choices, handle, || real_http(handle)) else {
        return Wired {
            catalog: None,
            account: None,
            resolver: None,
            opener: files,
        };
    };
    Wired {
        account: build_account(&parts, store, paths, handle),
        opener: Arc::new(RoutingOpener::new(files, Arc::new(parts.opener.clone()))),
        resolver: Some(Arc::clone(&parts.resolver)),
        catalog: Some(parts.catalog),
    }
}

/// The SoundCloud account: sign-in window, token in the credential `store`, sign-out that also
/// drops the session's caches. The window is this same executable started again (`main`).
/// `None` without a credential store, so no sign-in is offered and no token is kept anywhere.
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
