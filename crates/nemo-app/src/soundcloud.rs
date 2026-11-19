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
use nemo_ytm::url_resolver::YtDlpUrlResolver;
use nemo_ytm::ytdlp::YtDlpResolver;
use tokio::runtime::Handle;

use crate::config::OptInChoice;
use crate::opt_in_gate::build_if_open;

mod fallback;

pub use fallback::FallbackResolver;

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
            catalog: Arc::new(SoundcloudCatalog::new(client)),
            resolver: match ytdlp {
                Some(ytdlp) => Arc::new(FallbackResolver::new(Arc::new(resolver), ytdlp)),
                None => Arc::new(resolver),
            },
            opener,
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
