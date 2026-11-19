//! Preparing the first play in the background (cold-start spec).
//!
//! A fresh process pays for things a warm one does not: the HTTP client of the stream opener
//! (about 40 ms to build) and the first start of `yt-dlp` (cold disk cache; on Windows a scan of
//! its files, seconds). [`WarmUp`] pays for them once, after the first frame, on the app
//! runtime. It asks the network for nothing: no request is made that the first play would not
//! make anyway. Dropping it ends it, and kills a `yt-dlp` run that is still going.

use std::time::{Duration, Instant};

use tokio::runtime::Handle;

use crate::stream::HttpStreamOpener;
use crate::task::AbortOnDrop;
use crate::timing;
use crate::ytdlp::{YtDlpResolver, managed};

/// The background warm-up. Keep it alive for the whole app; dropping it cancels what is left.
pub struct WarmUp {
    _task: AbortOnDrop<()>,
}

impl std::fmt::Debug for WarmUp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WarmUp").finish_non_exhaustive()
    }
}

impl WarmUp {
    /// Start the warm-up on `handle` after `pause` (so the first frame is never delayed): build
    /// the stream opener's HTTP client, then run `yt-dlp --version` once. When the app has no
    /// `yt-dlp` yet and the installer is running, it waits for the install and warms that copy.
    #[must_use]
    pub fn start(
        handle: &Handle,
        ytdlp: YtDlpResolver,
        opener: HttpStreamOpener,
        pause: Duration,
    ) -> Self {
        let task = handle.spawn(async move {
            tokio::time::sleep(pause).await;
            let began = Instant::now();
            let built = tokio::task::spawn_blocking(move || opener.prepare())
                .await
                .unwrap_or(false);
            timing::stage("warm-up http client", began, built);
            if !ytdlp.warm_up().await
                && managed::wait_for_install(&ytdlp.program_slot())
                    .await
                    .is_some()
            {
                ytdlp.warm_up().await;
            }
        });
        Self {
            _task: AbortOnDrop(task),
        }
    }
}

#[cfg(test)]
mod tests;
