//! Opening a track on a helper thread: resolve, open, probe. Never on the control thread.

use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use nemo_core::audio::{Codec, Origin, StreamInfo};
use nemo_core::downloads::OfflineStore;
use nemo_core::library::{LocalPlayback, LocalTrackLookup};
use nemo_core::segment::SegmentSource;
use nemo_core::services::{ByteSource, StreamOpener};
use nemo_core::stream::{ResolvedStream, StreamError, StreamRequest, StreamResolver};
use nemo_core::track::{MediaUrl, Track, VideoId};
use nemo_core::url_resolver::{FormatPolicy, UrlResolver};

use crate::block_on::block_on;
use crate::decoder::{DecodeError, supported_stream_codecs};
use crate::player::Msg;
use crate::source::FileSource;
use crate::stream_decoder::StreamDecoder;
use crate::timing;

/// A track that is open and ready to decode.
pub struct Prepared {
    /// The opened stream.
    pub decoder: StreamDecoder,
    /// Measured facts about the source, for the signal path.
    pub info: StreamInfo,
    /// Span and gain of a local library track (a CUE track plays one span of its file). The
    /// default (whole file, unity gain) for streams and for files the library does not know.
    pub playback: LocalPlayback,
}

impl Prepared {
    /// Length of what plays: the span when there is one, else the whole stream.
    #[must_use]
    pub fn duration(&self) -> Option<Duration> {
        let whole = self.decoder.info().duration;
        match self.playback.span {
            None => whole,
            Some(span) => span.end.or(whole).map(|end| end.saturating_sub(span.start)),
        }
    }
}

/// The services a prepare thread needs.
#[derive(Clone)]
pub struct PrepareDeps {
    /// Stream resolver.
    pub resolver: Arc<dyn StreamResolver>,
    /// The current stream request, passed to every resolve of this prepare.
    pub request: StreamRequest,
    /// Stream opener.
    pub opener: Arc<dyn StreamOpener>,
    /// Span and gain of local tracks. Asked here, on the prepare thread, never in the callback.
    pub local: Option<Arc<dyn LocalTrackLookup>>,
    /// Saved songs. Asked before the resolver: a hit plays the cached file with no stream
    /// lookup and no audio request (offline-cache criterion 3).
    pub offline: Option<Arc<dyn OfflineStore>>,
    /// Pages of other sites (SoundCloud) to streams. Asked on the prepare thread. `None`: such
    /// tracks do not play.
    pub url_resolver: Option<Arc<dyn UrlResolver>>,
}

/// Open `track` on a new thread and report the result to the control thread.
pub(crate) fn spawn_prepare(deps: PrepareDeps, tx: Sender<Msg>, token: u64, track: Track) {
    let spawned = thread::Builder::new()
        .name("nemo-prepare".into())
        .spawn(move || {
            let result = prepare(&deps, &track).map(Box::new);
            // The control thread is gone only at shutdown; the result is then moot.
            let _ = tx.send(Msg::Prepared { token, result });
        });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "could not start a prepare thread");
    }
}

/// Fetch SponsorBlock segments on a new thread.
pub(crate) fn spawn_segments(
    source: Arc<dyn SegmentSource>,
    tx: Sender<Msg>,
    token: u64,
    video: VideoId,
    duration: Duration,
) {
    let spawned = thread::Builder::new()
        .name("nemo-segments".into())
        .spawn(move || {
            // A failed lookup means "no segments": playback never depends on it.
            let segments = block_on(source.segments(&video, duration)).unwrap_or_default();
            // The control thread is gone only at shutdown; the result is then moot.
            let _ = tx.send(Msg::Segments { token, segments });
        });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "could not start a segment thread");
    }
}

fn describe(err: &StreamError) -> String {
    match err {
        StreamError::NotAvailable => "this track is not available".into(),
        StreamError::ResolverMissing => "the stream resolver is not installed".into(),
        // The inner text may name a host; keep the message generic.
        StreamError::Network(_) | StreamError::Parse(_) => "could not get the stream".into(),
    }
}

fn describe_decode(err: &DecodeError) -> String {
    match err {
        DecodeError::UnsupportedCodec => "this track uses a codec that cannot be played".into(),
        DecodeError::Open(_) => "could not open the file".into(),
        _ => "this track could not be decoded".into(),
    }
}

fn prepare(deps: &PrepareDeps, track: &Track) -> Result<Prepared, String> {
    if let Some(path) = &track.local_path {
        let source = FileSource::open(path).map_err(|_| "could not open the file".to_owned())?;
        let ext = path.extension().and_then(|e| e.to_str());
        let decoder =
            StreamDecoder::open(Box::new(source), ext).map_err(|e| describe_decode(&e))?;
        let info = local_info(&decoder);
        // May block on the library database: this is the prepare thread.
        let playback = deps
            .local
            .as_ref()
            .and_then(|lookup| lookup.playback(track))
            .unwrap_or_default();
        return Ok(Prepared {
            decoder,
            info,
            playback,
        });
    }
    let Some(video) = &track.video_id else {
        return prepare_page(deps, track);
    };
    if let Some(saved) = deps
        .offline
        .as_ref()
        .and_then(|s| open_saved(s.as_ref(), video))
    {
        return Ok(saved);
    }
    let resolve = || block_on(deps.resolver.resolve_with(video, &deps.request));
    let began = Instant::now();
    let stream = resolve().map_err(|e| describe(&e))?;
    timing::stage("resolve", began);
    let opening = Instant::now();
    let opened = deps.opener.open(&stream);
    timing::stage("open", opening);
    let source: Box<dyn ByteSource> = match opened {
        Ok(source) => source,
        Err(StreamError::NotAvailable | StreamError::Network(_)) => {
            // A stale or refused URL (HTTP 403, 404/410, connection reset): tell the resolver,
            // which drops its cached answer or switches method for this video, and try once more.
            deps.resolver.stream_failed(video);
            let stream = resolve().map_err(|e| describe(&e))?;
            let source = deps.opener.open(&stream).map_err(|e| {
                deps.resolver.stream_failed(video);
                describe(&e)
            })?;
            return finish_stream(source, &stream);
        }
        Err(e) => return Err(describe(&e)),
    };
    finish_stream(source, &stream)
}

/// Open a track that has a page address and no video id (SoundCloud): resolve the page, open the
/// stream, probe it. A stream that does not open is looked up once more, as for YouTube.
fn prepare_page(deps: &PrepareDeps, track: &Track) -> Result<Prepared, String> {
    let (Some(url), Some(resolver)) = (&track.source_url, &deps.url_resolver) else {
        return Err("this track has no playable source".into());
    };
    let policy = FormatPolicy {
        playable: supported_stream_codecs().to_vec(),
        allow_hls: true,
    };
    let resolve = || block_on(resolver.resolve(url, &policy));
    let began = Instant::now();
    let stream = resolve().map_err(|e| describe(&e))?;
    timing::stage("resolve", began);
    let opening = Instant::now();
    let opened = deps.opener.open(&stream);
    timing::stage("open", opening);
    match opened {
        Ok(source) => finish_stream(source, &stream),
        Err(StreamError::NotAvailable | StreamError::Network(_)) => {
            retry_page(resolver.as_ref(), url, &resolve, deps.opener.as_ref())
        }
        Err(e) => Err(describe(&e)),
    }
}

/// The address stopped working: tell the resolver, look the stream up again, open it once more.
fn retry_page(
    resolver: &dyn UrlResolver,
    url: &MediaUrl,
    resolve: &dyn Fn() -> Result<ResolvedStream, StreamError>,
    opener: &dyn StreamOpener,
) -> Result<Prepared, String> {
    resolver.stream_failed(url);
    let stream = resolve().map_err(|e| describe(&e))?;
    let source = opener.open(&stream).map_err(|e| {
        resolver.stream_failed(url);
        describe(&e)
    })?;
    finish_stream(source, &stream)
}

/// The saved copy of `video`, opened and probed, or `None` when the cache has none. A copy that
/// does not open or probe is marked unsaved and `None` is returned, so the caller streams the
/// song instead; in offline mode the gated resolver then refuses and the user sees why.
fn open_saved(store: &dyn OfflineStore, video: &VideoId) -> Option<Prepared> {
    let cached = store.open(video)?;
    // The one place the cache path is exposed (offline-cache design, Interfaces).
    let opened = FileSource::open(cached.path.expose())
        .ok()
        .and_then(|source| {
            let bytes = source.byte_len();
            StreamDecoder::open(Box::new(source), None)
                .ok()
                .map(|decoder| (decoder, bytes))
        });
    let Some((decoder, bytes)) = opened else {
        tracing::warn!("a saved song could not be opened; it is marked unsaved");
        store.mark_unsaved(video);
        return None;
    };
    let probed = decoder.info();
    let info = StreamInfo {
        origin: Origin::YoutubeMusic,
        codec: probed.codec,
        bitrate_kbps: measured_kbps(bytes, probed.duration),
        format: Some(probed.format),
    };
    Some(Prepared {
        decoder,
        info,
        playback: LocalPlayback::default(),
    })
}

/// `bytes * 8 / duration` in kbps, rounded. `None` (shown as "Unknown") when either is missing
/// or the duration is zero.
fn measured_kbps(bytes: Option<u64>, duration: Option<Duration>) -> Option<u32> {
    let millis = duration?.as_millis();
    if millis == 0 {
        return None;
    }
    // bits per millisecond is kbit per second.
    let kbps = (u128::from(bytes?) * 8 + millis / 2) / millis;
    u32::try_from(kbps).ok()
}

/// Probe an opened stream and describe it for the signal path. The origin is the resolver's.
fn finish_stream(source: Box<dyn ByteSource>, stream: &ResolvedStream) -> Result<Prepared, String> {
    let probing = Instant::now();
    let decoder = StreamDecoder::open(source, None).map_err(|e| describe_decode(&e))?;
    timing::stage("probe", probing);
    let probed = decoder.info();
    let codec = if probed.codec == Codec::Unknown {
        stream.info.codec
    } else {
        probed.codec
    };
    let info = StreamInfo {
        origin: stream.info.origin,
        codec,
        bitrate_kbps: stream.info.bitrate_kbps,
        format: Some(probed.format),
    };
    Ok(Prepared {
        decoder,
        info,
        playback: LocalPlayback::default(),
    })
}

fn local_info(decoder: &StreamDecoder) -> StreamInfo {
    let d = decoder.info();
    StreamInfo {
        origin: Origin::Local,
        codec: d.codec,
        bitrate_kbps: None,
        format: Some(d.format),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::measured_kbps;

    #[test]
    fn the_measured_bitrate_is_bytes_times_8_over_the_duration() {
        let minute = Some(Duration::from_mins(1));
        assert_eq!(measured_kbps(Some(1_200_000), minute), Some(160));
        assert_eq!(
            measured_kbps(Some(1_000), Some(Duration::from_millis(3))),
            Some(2667)
        );
        assert_eq!(measured_kbps(None, minute), None, "no size: Unknown");
        assert_eq!(
            measured_kbps(Some(1_000), None),
            None,
            "no duration: Unknown"
        );
        assert_eq!(measured_kbps(Some(1_000), Some(Duration::ZERO)), None);
        assert_eq!(
            measured_kbps(Some(u64::MAX), Some(Duration::from_millis(1))),
            None
        );
    }
}
