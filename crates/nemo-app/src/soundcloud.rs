//! Building the SoundCloud services (soundcloud-full, ADR 0005): only when the switch is on and
//! the user confirmed the current notice. A closed gate means nothing is built, so no request is
//! made, no host contacted and no SoundCloud entry shown.

use std::collections::BTreeMap;
use std::sync::Arc;

use nemo_core::catalog::Catalog;
use nemo_core::services::StreamOpener;
use nemo_core::url_resolver::UrlResolver;
use nemo_soundcloud::{
    OPT_IN, ReqwestHttp, ScClient, ScHttp, ScStreamOpener, SoundcloudCatalog, streams,
};
use tokio::runtime::Handle;

use crate::config::OptInChoice;
use crate::opt_in_gate::build_if_open;

/// The services of one SoundCloud client.
pub struct SoundcloudParts {
    /// Search and pages, for `Backend.soundcloud`.
    pub catalog: Arc<dyn Catalog>,
    /// Page address to stream, for the player.
    pub resolver: Arc<dyn UrlResolver>,
    /// Opens the HLS streams the resolver finds.
    pub opener: ScStreamOpener,
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
        let early = opener.clone();
        // Builds the media client (loads the system certificates) before the first play.
        handle.spawn_blocking(move || {
            let _ = early.warm_up();
        });
        Some(SoundcloudParts {
            catalog: Arc::new(SoundcloudCatalog::new(client)),
            resolver: Arc::new(resolver),
            opener,
        })
    })
    .flatten()
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
