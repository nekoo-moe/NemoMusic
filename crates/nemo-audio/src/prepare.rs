//! Opening a track on a helper thread: resolve, open, probe. Never on the control thread.

use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use nemo_core::audio::{Codec, Origin, StreamInfo};
use nemo_core::segment::SegmentSource;
use nemo_core::services::{ByteSource, StreamOpener};
use nemo_core::stream::{StreamError, StreamRequest, StreamResolver};
use nemo_core::track::{Track, VideoId};

use crate::block_on::block_on;
use crate::decoder::DecodeError;
use crate::player::Msg;
use crate::source::FileSource;
use crate::stream_decoder::StreamDecoder;

/// A track that is open and ready to decode.
pub struct Prepared {
    /// The opened stream.
    pub decoder: StreamDecoder,
    /// Measured facts about the source, for the signal path.
    pub info: StreamInfo,
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
        return Ok(Prepared { decoder, info });
    }
    let Some(video) = &track.video_id else {
        return Err("this track has no playable source".into());
    };
    let resolve = || block_on(deps.resolver.resolve_with(video, &deps.request));
    let stream = resolve().map_err(|e| describe(&e))?;
    let source: Box<dyn ByteSource> = match deps.opener.open(&stream) {
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

/// Probe an opened YouTube stream and describe it for the signal path.
fn finish_stream(
    source: Box<dyn ByteSource>,
    stream: &nemo_core::stream::ResolvedStream,
) -> Result<Prepared, String> {
    let decoder = StreamDecoder::open(source, None).map_err(|e| describe_decode(&e))?;
    let probed = decoder.info();
    let codec = if probed.codec == Codec::Unknown {
        stream.info.codec
    } else {
        probed.codec
    };
    let info = StreamInfo {
        origin: Origin::YoutubeMusic,
        codec,
        bitrate_kbps: stream.info.bitrate_kbps,
        format: Some(probed.format),
    };
    Ok(Prepared { decoder, info })
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
