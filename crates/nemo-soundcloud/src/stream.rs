//! Finding the stream of a SoundCloud track: [`ScStreamResolver`] implements the `nemo-core`
//! [`UrlResolver`] without `yt-dlp` (soundcloud-full, ADR 0005).
//!
//! The page address of a track leads to the record the catalog kept (or, for a page the catalog
//! has not seen, one `/resolve` call). The record lists the track's streams. The best playable
//! one is chosen with `rank_formats` (the adaptive `abr` list and encrypted streams never are
//! candidates), one call turns it into a signed media address, and the answer is kept in memory
//! for [`STREAM_TTL`]. An encrypted-only track is `NotAvailable`: no bypass is tried (`coc.md`).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use nemo_core::BoxFuture;
use nemo_core::audio::{Codec, Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::stream::{ResolvedStream, StreamError, Transport};
use nemo_core::track::MediaUrl;
use nemo_core::url_resolver::{
    FormatCandidate, FormatPolicy, MediaEntry, MediaQuery, ResolverState, UrlResolver, rank_formats,
};
use tokio::runtime::Handle;

use crate::client::{ClientError, ScClient};
use crate::hosts::HOSTS;
use crate::model::{Item, ScTranscoding, parse_item, parse_one};
use crate::opener::{ScStreamOpener, Shared};
use crate::records::TrackRecord;

/// How long a looked-up address is reused. The address itself stops working about 4 minutes
/// after the lookup (observed 2026-11-19), so this stays under that.
pub const STREAM_TTL: Duration = Duration::from_mins(3);

/// A transcoding and what ranking needs to know about it.
struct Offer<'a> {
    transcoding: &'a ScTranscoding,
    candidate: FormatCandidate,
}

/// The codec and nominal bitrate a preset names: `aac_160k`, `mp3_0_0`, `opus_0_0`. `None` for
/// the adaptive `abr_*` list and presets this app does not know.
fn preset(name: &str) -> Option<(Codec, Option<u32>)> {
    let (family, rest) = name.split_once('_')?;
    let codec = match family {
        "aac" => Codec::Aac,
        "mp3" => Codec::Mp3,
        "opus" => Codec::Opus,
        _ => return None,
    };
    let kbps = rest.strip_suffix('k').and_then(|n| n.parse().ok());
    Some((codec, kbps))
}

/// The bitrate an address states: `.../abc.128.mp3` or `.128.opus`.
fn kbps_in_address(url: &str) -> Option<u32> {
    let path = url.split(['?', '#']).next()?;
    let (stem, ext) = path.rsplit_once('.')?;
    if !matches!(ext, "mp3" | "opus") {
        return None;
    }
    stem.rsplit_once('.')?
        .1
        .parse()
        .ok()
        .filter(|n| (8..=512).contains(n))
}

/// Every transcoding that could be played, with the encrypted ones marked.
fn offers(record: &TrackRecord) -> Vec<Offer<'_>> {
    let mut offers = Vec::new();
    for transcoding in &record.transcodings {
        let Some((codec, bitrate_kbps)) = preset(&transcoding.preset) else {
            continue;
        };
        let protocol = transcoding.format.protocol.as_str();
        let (hls, encrypted) = match protocol {
            "progressive" => (false, false),
            "hls" => (true, false),
            _ => (true, true),
        };
        offers.push(Offer {
            transcoding,
            candidate: FormatCandidate {
                index: offers.len(),
                codec,
                bitrate_kbps,
                hls,
                encrypted,
                preview: record.preview || transcoding.snipped,
            },
        });
    }
    offers
}

/// `ClientError` as a stream error, with texts that never hold an address.
fn stream_error(e: ClientError) -> StreamError {
    match e {
        ClientError::NotFound | ClientError::Refused => StreamError::NotAvailable,
        ClientError::RateLimited => {
            StreamError::Network("too many requests; try again later".to_owned())
        }
        ClientError::Network | ClientError::Timeout => {
            StreamError::Network("could not reach SoundCloud".to_owned())
        }
        ClientError::Parse => StreamError::Parse("unexpected answer from SoundCloud".to_owned()),
        ClientError::NoClientId => {
            StreamError::Network("SoundCloud changed its web app".to_owned())
        }
        ClientError::SessionEnded => {
            StreamError::Network("Sign in to SoundCloud again, then play this".to_owned())
        }
        ClientError::Off => StreamError::Network("SoundCloud is switched off".to_owned()),
    }
}

struct Inner {
    client: Arc<ScClient>,
    shared: Arc<Shared>,
    cache: Mutex<HashMap<String, (Instant, ResolvedStream)>>,
    warming: Mutex<HashSet<String>>,
    policy: Mutex<FormatPolicy>,
}

impl Inner {
    fn cached(&self, page: &str) -> Option<ResolvedStream> {
        let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        let (at, stream) = cache.get(page)?;
        (at.elapsed() < STREAM_TTL).then(|| stream.clone())
    }

    /// The record of `page`: the one the catalog kept, else one `/resolve` call.
    async fn record(&self, page: &MediaUrl) -> Result<TrackRecord, StreamError> {
        if let Some(record) = self.client.records().get(page.as_str()) {
            return Ok(record);
        }
        let body = self
            .client
            .get("/resolve", &[("url", page.as_str())])
            .await
            .map_err(stream_error)?;
        let value = parse_one(&body).map_err(stream_error)?;
        let Some(Item::Track(track)) = parse_item(value) else {
            return Err(StreamError::NotAvailable);
        };
        let record = TrackRecord::of(&track).ok_or(StreamError::NotAvailable)?;
        self.client.records().insert(page.as_str(), record.clone());
        Ok(record)
    }

    async fn resolve(
        &self,
        page: &MediaUrl,
        policy: &FormatPolicy,
    ) -> Result<ResolvedStream, StreamError> {
        if let Some(hit) = self.cached(page.as_str()) {
            return Ok(hit);
        }
        *self.policy.lock().unwrap_or_else(PoisonError::into_inner) = policy.clone();
        let record = self.record(page).await?;
        let offers = offers(&record);
        let candidates: Vec<FormatCandidate> = offers.iter().map(|o| o.candidate).collect();
        let pick = rank_formats(&candidates, policy).ok_or(StreamError::NotAvailable)?;
        let offer = offers.get(pick).ok_or(StreamError::NotAvailable)?;
        let stream = self.look_up(&record, offer).await?;
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(page.as_str().to_owned(), (Instant::now(), stream.clone()));
        Ok(stream)
    }

    /// One call: the signed media address of the chosen stream.
    async fn look_up(
        &self,
        record: &TrackRecord,
        offer: &Offer<'_>,
    ) -> Result<ResolvedStream, StreamError> {
        let auth = record.authorization.as_ref().map(|a| a.expose().as_str());
        let params: Vec<(&str, &str)> = auth
            .map(|a| ("track_authorization", a))
            .into_iter()
            .collect();
        let body = self
            .client
            .get_api_url(&offer.transcoding.url, &params)
            .await
            .map_err(stream_error)?;
        let answer: LookupAnswer = parse_one(&body).map_err(stream_error)?;
        // The address must be https on a SoundCloud host: it is fetched without the client id.
        MediaUrl::parse(&answer.url, HOSTS)
            .map_err(|_| StreamError::Parse("unexpected stream address".to_owned()))?;
        let c = &offer.candidate;
        let bitrate_kbps = c.bitrate_kbps.or_else(|| kbps_in_address(&answer.url));
        Ok(ResolvedStream {
            url: Secret::new(answer.url),
            info: StreamInfo {
                origin: Origin::SoundCloud,
                codec: c.codec,
                bitrate_kbps,
                format: None,
            },
            expires_at: SystemTime::now().checked_add(STREAM_TTL),
            transport: if c.hls {
                Transport::Hls { live: false }
            } else {
                Transport::File
            },
            preview: c.preview,
            audio_language: None,
            audio_tracks: Vec::new(),
        })
    }

    /// Resolve and prefetch the next track, once at a time per page.
    async fn warm(self: Arc<Self>, page: MediaUrl) {
        let policy = self
            .policy
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Ok(stream) = self.resolve(&page, &policy).await {
            let shared = Arc::clone(&self.shared);
            // The blocking client must not run on the async threads.
            let _ = tokio::task::spawn_blocking(move || shared.prefetch(&stream)).await;
        }
        self.warming
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(page.as_str());
    }
}

/// The answer of a stream lookup.
#[derive(serde::Deserialize)]
struct LookupAnswer {
    url: String,
}

/// The SoundCloud stream resolver. Cheap to clone.
#[derive(Clone)]
pub struct ScStreamResolver {
    inner: Arc<Inner>,
    handle: Handle,
}

impl std::fmt::Debug for ScStreamResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScStreamResolver").finish_non_exhaustive()
    }
}

impl ScStreamResolver {
    /// A resolver over `client`. `shared` is the state it shares with the opener; `handle` is the
    /// runtime that runs the look-ups started by [`UrlResolver::warm`].
    #[must_use]
    pub fn new(client: Arc<ScClient>, shared: Arc<Shared>, handle: Handle) -> Self {
        Self {
            inner: Arc::new(Inner {
                client,
                shared,
                cache: Mutex::default(),
                warming: Mutex::default(),
                policy: Mutex::new(FormatPolicy {
                    playable: vec![Codec::Aac, Codec::Mp3, Codec::Opus],
                    allow_hls: true,
                }),
            }),
            handle,
        }
    }
}

impl ScStreamResolver {
    /// Forget every kept stream address and its prefetch (sign-out: some belong to private
    /// tracks). Returns how many addresses were kept.
    pub fn forget_all(&self) -> usize {
        let removed = {
            let mut cache = self
                .inner
                .cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let removed = cache.len();
            cache.clear();
            removed
        };
        removed + self.inner.shared.clear()
    }
}

impl UrlResolver for ScStreamResolver {
    fn resolve<'a>(
        &'a self,
        url: &'a MediaUrl,
        policy: &'a FormatPolicy,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        Box::pin(self.inner.resolve(url, policy))
    }

    /// Searches and pages come from the catalog, not from here.
    fn list<'a>(
        &'a self,
        _query: &'a MediaQuery,
    ) -> BoxFuture<'a, Result<Vec<MediaEntry>, StreamError>> {
        Box::pin(async { Err(StreamError::NotAvailable) })
    }

    fn state(&self) -> ResolverState {
        ResolverState::Ready {
            version: "native".to_owned(),
        }
    }

    fn warm(&self, url: &MediaUrl) {
        let fresh = self
            .inner
            .cached(url.as_str())
            .is_some_and(|s| self.inner.shared.has(s.url.expose()));
        if fresh {
            return;
        }
        let started = self
            .inner
            .warming
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(url.as_str().to_owned());
        if started {
            self.handle.spawn(Arc::clone(&self.inner).warm(url.clone()));
        }
    }

    fn stream_failed(&self, url: &MediaUrl) {
        let dropped = self
            .inner
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(url.as_str());
        if let Some((_, stream)) = dropped {
            self.inner.shared.forget(stream.url.expose());
        }
    }
}

/// The resolver and the opener of one SoundCloud client, sharing the prefetch.
#[must_use]
pub fn streams(client: Arc<ScClient>, handle: Handle) -> (ScStreamResolver, ScStreamOpener) {
    let shared = Arc::new(Shared::new());
    let resolver = ScStreamResolver::new(client, Arc::clone(&shared), handle);
    (resolver, ScStreamOpener::new(shared))
}

#[cfg(test)]
mod tests;
