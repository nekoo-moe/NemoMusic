//! Opening a SoundCloud HLS stream, and the one-slot prefetch that makes the next track start at
//! once.
//!
//! [`Shared`] is the state the resolver and the opener share: the fetch rules and the playlists
//! and first segments fetched ahead of time (at most [`SLOTS`], dropped after [`KEEP`]). The
//! player warms one entry ahead, so a prefetch holds about 200 KB.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use nemo_core::services::{ByteSource, StreamOpener};
use nemo_core::stream::{ResolvedStream, StreamError, Transport};
use reqwest::blocking::Client;

use crate::hls::playlist::{self, MediaPlaylist, PlaylistError};
use crate::hls::{Failure, Head, HlsSource, Rules, fetch, fetch_head};

/// Prefetched streams kept.
pub const SLOTS: usize = 2;
/// How long a prefetched playlist stays usable. A stream address stops working a few minutes
/// after its lookup (about 4 min observed on 2026-11-19).
pub const KEEP: Duration = Duration::from_secs(150);

struct Entry {
    stream_url: String,
    list: MediaPlaylist,
    head: Head,
    at: Instant,
}

/// State shared by the resolver and the opener.
pub struct Shared {
    rules: Rules,
    slots: Mutex<Vec<Entry>>,
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
    pub const fn with(rules: Rules) -> Self {
        Self {
            rules,
            slots: Mutex::new(Vec::new()),
        }
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
        let client = self.rules.client().map_err(stream_error)?;
        let list = self.playlist(&client, url)?;
        let head = fetch_head(&client, &list, &self.rules).map_err(stream_error)?;
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        slots.retain(|e| e.stream_url != *url && e.at.elapsed() < KEEP);
        slots.push(Entry {
            stream_url: url.clone(),
            list,
            head,
            at: Instant::now(),
        });
        while slots.len() > SLOTS {
            slots.remove(0);
        }
        Ok(())
    }

    /// Whether a fresh prefetch of `url` is waiting.
    #[must_use]
    pub fn has(&self, url: &str) -> bool {
        let slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        slots
            .iter()
            .any(|e| e.stream_url == url && e.at.elapsed() < KEEP)
    }

    /// Take the prefetch of `url`, if there is a fresh one.
    fn take(&self, url: &str) -> Option<(MediaPlaylist, Head)> {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        let at = slots.iter().position(|e| e.stream_url == url)?;
        let entry = slots.remove(at);
        (entry.at.elapsed() < KEEP).then_some((entry.list, entry.head))
    }

    /// Forget the prefetch of `url` (its address stopped working).
    pub fn forget(&self, url: &str) {
        self.slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|e| e.stream_url != url);
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

impl StreamOpener for ScStreamOpener {
    fn open(&self, stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        if !matches!(stream.transport, Transport::Hls { live: false }) {
            return Err(StreamError::NotAvailable);
        }
        let url = stream.url.expose();
        let rules = self.shared.rules();
        let client = rules.client().map_err(stream_error)?;
        let (list, head) = match self.shared.take(url) {
            Some((list, head)) => (list, Some(head)),
            None => (self.shared.playlist(&client, url)?, None),
        };
        let source = HlsSource::start(client, rules, list, head)
            .map_err(|_| StreamError::Network("could not start the stream reader".to_owned()))?;
        Ok(Box::new(source))
    }
}
