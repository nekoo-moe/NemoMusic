//! Opening a SoundCloud HLS stream, and the prefetch that makes the next tracks start at once.
//!
//! [`Shared`] is the state the resolver and the opener share: the fetch rules, one HTTP client,
//! and the playlists (with the init segment and first segment when they were fetched ahead of
//! time) of the last [`SLOTS`] streams, kept for [`KEEP`]. The player warms two entries ahead, so
//! the prefetch holds about 400 KB, and a replay of a track just played needs no new playlist.
//! The connection to the media host is opened ahead of the first play ([`Shared::preconnect`]).

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use nemo_core::services::{ByteSource, StreamOpener};
use nemo_core::stream::{ResolvedStream, StreamError, Transport};
use reqwest::blocking::Client;

use crate::hls::playlist::{self, MediaPlaylist, PlaylistError};
use crate::hls::{Failure, Head, HlsSource, Rules, fetch, fetch_head};

/// The media host the HLS audio comes from.
pub const MEDIA_HOST: &str = "https://playback.media-streaming.soundcloud.cloud/";

/// Prefetched streams kept.
pub const SLOTS: usize = 3;
/// How long a prefetched playlist stays usable. A stream address stops working a few minutes
/// after its lookup (about 4 min observed on 2026-11-19).
pub const KEEP: Duration = Duration::from_secs(150);
/// The least time between two connections opened ahead to the media host. The pool keeps an idle
/// connection for 90 s.
pub const PRECONNECT_EVERY: Duration = Duration::from_secs(45);

struct Entry {
    stream_url: String,
    list: MediaPlaylist,
    /// The init segment and first segment, when they were fetched ahead of time.
    head: Option<Head>,
    at: Instant,
}

/// State shared by the resolver and the opener.
pub struct Shared {
    rules: Rules,
    slots: Mutex<Vec<Entry>>,
    /// One blocking client for every stream: building it loads the system certificates, and
    /// reusing it keeps the connection to the media host open between tracks.
    client: Mutex<Option<Client>>,
    /// An address on the media host that is asked with `HEAD` to open the connection (it answers
    /// 403 without a signature, which is all the connection needs).
    preconnect_url: String,
    /// When the connection was last opened ahead.
    preconnected: Mutex<Option<Instant>>,
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared").finish_non_exhaustive()
    }
}

/// Plain texts; never an address.
fn stream_error(failure: Failure) -> StreamError {
    match failure {
        Failure::Gone => StreamError::NotAvailable,
        Failure::Busy => StreamError::Network("SoundCloud asked to slow down".to_owned()),
        Failure::Network => StreamError::Network("could not read the stream".to_owned()),
    }
}

fn playlist_error(error: PlaylistError) -> StreamError {
    match error {
        PlaylistError::Encrypted => StreamError::NotAvailable,
        _ => StreamError::Parse("unsupported stream playlist".to_owned()),
    }
}

impl Shared {
    /// State with the real fetch rules.
    #[must_use]
    pub fn new() -> Self {
        Self::with(Rules::production())
    }

    /// State with other rules (tests).
    #[must_use]
    pub fn with(rules: Rules) -> Self {
        Self::with_preconnect(rules, MEDIA_HOST.to_owned())
    }

    /// State with other rules and another address to open the connection with (tests).
    #[must_use]
    pub const fn with_preconnect(rules: Rules, preconnect_url: String) -> Self {
        Self {
            rules,
            slots: Mutex::new(Vec::new()),
            client: Mutex::new(None),
            preconnect_url,
            preconnected: Mutex::new(None),
        }
    }

    /// The shared blocking client, built on first use. Call it from a plain thread, never from
    /// an async task.
    ///
    /// # Errors
    /// [`StreamError::Network`] when the TLS backend cannot start.
    pub fn client(&self) -> Result<Client, StreamError> {
        let mut slot = self.client.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(client) = slot.as_ref() {
            return Ok(client.clone());
        }
        let client = self.rules.client().map_err(stream_error)?;
        *slot = Some(client.clone());
        Ok(client)
    }

    /// The fetch rules.
    #[must_use]
    pub const fn rules(&self) -> Rules {
        self.rules
    }

    /// Fetch and read the playlist at `url`.
    ///
    /// # Errors
    /// [`StreamError::NotAvailable`] for a gone address or a key, otherwise `Network` or `Parse`
    /// with a fixed text.
    pub fn playlist(&self, client: &Client, url: &str) -> Result<MediaPlaylist, StreamError> {
        let bytes = fetch::get(client, url, &self.rules).map_err(stream_error)?;
        let text = String::from_utf8_lossy(&bytes);
        playlist::parse(&text, &|rel| self.rules.resolve(url, rel)).map_err(playlist_error)
    }

    /// Fetch the playlist, init segment and first segment of `stream` and keep them for the
    /// opener. Blocks: call it from a blocking thread.
    ///
    /// # Errors
    /// As [`Shared::playlist`], or the failure of the head fetch.
    pub fn prefetch(&self, stream: &ResolvedStream) -> Result<(), StreamError> {
        if !matches!(stream.transport, Transport::Hls { live: false }) {
            return Ok(());
        }
        let url = stream.url.expose();
        let client = self.client()?;
        let list = self.playlist(&client, url)?;
        let head = fetch_head(&client, &list, &self.rules).map_err(stream_error)?;
        self.store(url, list, Some(head));
        Ok(())
    }

    /// Keep a playlist (and its head) for the next open of `url`.
    fn store(&self, url: &str, list: MediaPlaylist, head: Option<Head>) {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        slots.retain(|e| e.stream_url != url && e.at.elapsed() < KEEP);
        slots.push(Entry {
            stream_url: url.to_owned(),
            list,
            head,
            at: Instant::now(),
        });
        while slots.len() > SLOTS {
            slots.remove(0);
        }
    }

    /// Whether a fresh prefetch of `url` is waiting.
    #[must_use]
    pub fn has(&self, url: &str) -> bool {
        let slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        slots
            .iter()
            .any(|e| e.stream_url == url && e.head.is_some() && e.at.elapsed() < KEEP)
    }

    /// The kept playlist of `url` (and its head), if it is fresh. The entry stays for a replay.
    fn reuse(&self, url: &str) -> Option<(MediaPlaylist, Option<Head>)> {
        let slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = slots.iter().find(|e| e.stream_url == url)?;
        (entry.at.elapsed() < KEEP).then(|| (entry.list.clone(), entry.head.clone()))
    }

    /// Whether the connection to the media host should be opened ahead now, and note the time if
    /// so. At most once per [`PRECONNECT_EVERY`].
    #[must_use]
    pub fn preconnect_due(&self) -> bool {
        let mut last = self
            .preconnected
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if last.is_some_and(|at| at.elapsed() < PRECONNECT_EVERY) {
            return false;
        }
        *last = Some(Instant::now());
        true
    }

    /// Open the connection to the media host ahead of a play: one `HEAD` of the host's root, which
    /// answers 403 without a signature. The HTTP client keeps the connection (and its TLS session)
    /// for the first playlist and segments. Blocks: call it from a blocking thread. The answer is
    /// not looked at; a failure only means the first play opens the connection itself.
    pub fn preconnect(&self) {
        if let Ok(client) = self.client() {
            let _ = client.head(&self.preconnect_url).send();
        }
    }

    /// Drop every prefetch; returns how many were waiting.
    pub fn clear(&self) -> usize {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        let removed = slots.len();
        slots.clear();
        removed
    }

    /// Forget the prefetch of `url` (its address stopped working).
    pub fn forget(&self, url: &str) {
        self.slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|e| e.stream_url != url);
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        // A blocking client panics when it is dropped inside an async runtime, and the last
        // owner of this state can be an async task, so it is dropped on a thread of its own.
        let client = self
            .client
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(client) = client {
            let _ = std::thread::Builder::new()
                .name("nemo-sc-drop".into())
                .spawn(move || drop(client));
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

/// Opens SoundCloud HLS streams. Streams that are plain files (progressive MP3) are not its
/// business: `nemo-app` sends those to the ranged file reader.
#[derive(Debug, Clone)]
pub struct ScStreamOpener {
    shared: Arc<Shared>,
}

impl ScStreamOpener {
    /// An opener over the state the resolver shares.
    #[must_use]
    pub const fn new(shared: Arc<Shared>) -> Self {
        Self { shared }
    }
}

impl ScStreamOpener {
    /// Open the connection to the media host ahead of the first play, if that was not done
    /// lately. Blocks: call it from a blocking thread.
    pub fn preconnect(&self) {
        if self.shared.preconnect_due() {
            self.shared.preconnect();
        }
    }

    /// Build the HTTP client now, so the first play does not pay for loading the system
    /// certificates. Blocks for a moment: call it from a blocking thread. It makes no request.
    ///
    /// # Errors
    /// [`StreamError::Network`] when the TLS backend cannot start.
    pub fn warm_up(&self) -> Result<(), StreamError> {
        self.shared.client().map(|_| ())
    }

    /// The shared prefetch state, to ask whether a stream is waiting.
    #[must_use]
    pub const fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }
}

impl StreamOpener for ScStreamOpener {
    fn open(&self, stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        if !matches!(stream.transport, Transport::Hls { live: false }) {
            return Err(StreamError::NotAvailable);
        }
        let url = stream.url.expose();
        let rules = self.shared.rules();
        let client = self.shared.client()?;
        let (list, head) = if let Some(kept) = self.shared.reuse(url) {
            kept
        } else {
            let list = self.shared.playlist(&client, url)?;
            self.shared.store(url, list.clone(), None);
            (list, None)
        };
        let source = HlsSource::start(client, rules, list, head)
            .map_err(|_| StreamError::Network("could not start the stream reader".to_owned()))?;
        Ok(Box::new(source))
    }
}

#[cfg(test)]
mod tests;
