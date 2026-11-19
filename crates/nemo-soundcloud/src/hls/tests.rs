//! Tests for the HLS reader over a loopback server and the recorded AAC segments.

use std::fmt::Write as _;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use nemo_audio::stream_decoder::StreamDecoder;
use nemo_core::audio::Codec;
use nemo_core::services::ByteSource;

use super::*;
use crate::opener::Shared;
use crate::test_server::{Response, TestServer};

const INIT: &[u8] = include_bytes!("../../tests/fixtures/hls/init.mp4");
const SEGMENTS: [&[u8]; 3] = [
    include_bytes!("../../tests/fixtures/hls/data000.m4s"),
    include_bytes!("../../tests/fixtures/hls/data001.m4s"),
    include_bytes!("../../tests/fixtures/hls/data002.m4s"),
];

const PLAYLIST: &str = "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:2\n#EXT-X-MEDIA-SEQUENCE:0\n\
#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:2.020136,\ndata000.m4s\n\
#EXTINF:1.996916,\ndata001.m4s\n#EXTINF:0.023220,\ndata002.m4s\n#EXT-X-ENDLIST\n";

fn expected() -> Vec<u8> {
    let mut all = INIT.to_vec();
    for s in SEGMENTS {
        all.extend_from_slice(s);
    }
    all
}

/// Serves the playlist and the segments under `/hls/`.
fn serve() -> TestServer {
    TestServer::start(|path, _| serve_path(path))
}

fn serve_path(path: &str) -> Response {
    match path {
        "/hls/out.m3u8" => Response::new(200, PLAYLIST),
        "/hls/init.mp4" => Response::new(200, INIT),
        "/hls/data000.m4s" => Response::new(200, SEGMENTS[0]),
        "/hls/data001.m4s" => Response::new(200, SEGMENTS[1]),
        "/hls/data002.m4s" => Response::new(200, SEGMENTS[2]),
        _ => Response::new(404, ""),
    }
}

fn open(server: &TestServer) -> HlsSource {
    let shared = Shared::with(Rules::loopback());
    let client = shared.rules().client().expect("client");
    let list = shared
        .playlist(&client, &server.url("/hls/out.m3u8"))
        .expect("playlist");
    HlsSource::start(client, shared.rules(), list, None).expect("source")
}

#[test]
fn the_segments_arrive_as_one_forward_byte_stream() {
    let server = serve();
    let mut source = open(&server);
    let mut all = Vec::new();
    source.read_to_end(&mut all).expect("whole stream");
    assert_eq!(all, expected(), "init segment, then the segments in order");
    assert_eq!(
        source.read(&mut [0; 4]).expect("eof"),
        0,
        "the end stays the end"
    );
    assert!(!source.is_seekable() && source.byte_len().is_none());
}

#[test]
fn the_decoder_reads_the_recorded_aac_stream_without_an_index() {
    let server = serve();
    let source = open(&server);
    let mut decoder = StreamDecoder::open(Box::new(source), None).expect("decoder opens");
    let info = *decoder.info();
    assert_eq!(info.codec, Codec::Aac);
    // The init segment states no length, and symphonia would report the first fragment's. The
    // playlist's `EXTINF` sum is the real length of the recorded tone (about 4.04 s).
    assert_eq!(info.duration, Some(Duration::from_micros(4_040_272)));
    assert_eq!((info.format.sample_rate, info.format.channels), (44_100, 2));
    let mut frames = 0_usize;
    let mut block = Vec::new();
    while decoder.next_block(&mut block).expect("decodes") {
        frames += block.len() / 2;
    }
    // Four seconds of tone, less the encoder delay and padding the container trims or not.
    assert!((170_000..=180_500).contains(&frames), "{frames} frames");
    assert!(block.iter().all(|s| s.is_finite()));
}

#[test]
fn reading_stops_a_few_segments_ahead() {
    let seen = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&seen);
    let many = TestServer::start(move |path, _| {
        if path == "/hls/many.m3u8" {
            let mut text = String::from("#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n");
            for n in 0..30 {
                let _ = writeln!(text, "#EXTINF:2.0,\nseg{n}.m4s");
            }
            text.push_str("#EXT-X-ENDLIST\n");
            Response::new(200, text)
        } else {
            if path.starts_with("/hls/seg") || path.ends_with("init.mp4") {
                counter.fetch_add(1, Ordering::SeqCst);
            }
            Response::new(200, vec![7_u8; 1000])
        }
    });
    let shared = Shared::with(Rules::loopback());
    let client = shared.rules().client().expect("client");
    let list = shared
        .playlist(&client, &many.url("/hls/many.m3u8"))
        .expect("playlist");
    let mut source = HlsSource::start(client, shared.rules(), list, None).expect("source");
    let mut one = [0_u8; 1];
    source.read_exact(&mut one).expect("first byte");
    std::thread::sleep(Duration::from_millis(400));
    let fetched = seen.load(Ordering::SeqCst);
    assert!(
        fetched <= 1 + 1 + READ_AHEAD_SEGMENTS + 1,
        "{fetched} of 31 files fetched"
    );
    drop(source);
    std::thread::sleep(Duration::from_millis(100));
    let after = seen.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        seen.load(Ordering::SeqCst),
        after,
        "nothing more is fetched once the source is dropped"
    );
}

#[test]
fn a_429_with_a_short_wait_is_retried() {
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    let server = TestServer::start(move |path, _| {
        if path == "/hls/data001.m4s" && counter.fetch_add(1, Ordering::SeqCst) == 0 {
            return Response::new(429, "").header("Retry-After", "0");
        }
        serve_path(path)
    });
    let mut all = Vec::new();
    open(&server).read_to_end(&mut all).expect("stream");
    assert_eq!(all, expected());
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[test]
fn three_server_errors_end_the_stream_with_an_error() {
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    let server = TestServer::start(move |path, _| {
        if path == "/hls/data001.m4s" {
            counter.fetch_add(1, Ordering::SeqCst);
            return Response::new(500, "");
        }
        serve_path(path)
    });
    let mut all = Vec::new();
    let err = open(&server)
        .read_to_end(&mut all)
        .expect_err("the stream fails");
    assert_eq!(err.to_string(), "the SoundCloud stream failed");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        SEGMENT_TRIES_FOR_TEST,
        "the call and two retries"
    );
    assert!(
        all.len() >= INIT.len() + SEGMENTS[0].len(),
        "what arrived before is delivered"
    );
}

const SEGMENT_TRIES_FOR_TEST: usize = fetch::SEGMENT_TRIES;

#[test]
fn a_gone_segment_ends_the_stream_at_once_and_a_long_wait_is_not_waited_for() {
    for (status, retry) in [(403_u16, None), (429, Some("3600"))] {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        let server = TestServer::start(move |path, _| {
            if path == "/hls/data001.m4s" {
                counter.fetch_add(1, Ordering::SeqCst);
                let mut r = Response::new(status, "");
                if let Some(wait) = retry {
                    r = r.header("Retry-After", wait);
                }
                return r;
            }
            serve_path(path)
        });
        let began = Instant::now();
        let mut all = Vec::new();
        assert!(open(&server).read_to_end(&mut all).is_err());
        assert_eq!(hits.load(Ordering::SeqCst), 1, "no retry for {status}");
        assert!(began.elapsed() < Duration::from_secs(5));
    }
}

#[test]
fn only_staying_or_skipping_forward_is_a_seek() {
    let server = serve();
    let mut source = open(&server);
    let all = expected();
    let mut first = [0_u8; 10];
    source.read_exact(&mut first).expect("read");
    assert_eq!(source.stream_position().expect("here"), 10);
    assert_eq!(source.seek(SeekFrom::Start(10)).expect("same place"), 10);
    assert_eq!(
        source.seek(SeekFrom::Start(5000)).expect("skip ahead"),
        5000
    );
    let mut next = [0_u8; 4];
    source.read_exact(&mut next).expect("read after skip");
    assert_eq!(next, all[5000..5004]);
    for back in [
        SeekFrom::Start(0),
        SeekFrom::Current(-100),
        SeekFrom::End(0),
    ] {
        let err = source.seek(back).expect_err("cannot go back");
        assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
    }
    assert_eq!(
        source
            .seek(SeekFrom::Start(u64::MAX / 2))
            .expect("past the end"),
        all.len() as u64
    );
}

#[test]
fn head_fetch_gets_the_init_and_the_first_segment_and_the_source_uses_it() {
    let server = serve();
    let shared = Shared::with(Rules::loopback());
    let client = shared.rules().client().expect("client");
    let list = shared
        .playlist(&client, &server.url("/hls/out.m3u8"))
        .expect("playlist");
    let head = fetch_head(&client, &list, &shared.rules()).expect("head");
    assert_eq!(head.init.as_deref(), Some(INIT));
    assert_eq!(head.first, SEGMENTS[0]);
    let before = server.paths().len();
    let mut all = Vec::new();
    HlsSource::start(client, shared.rules(), list, Some(head))
        .expect("source")
        .read_to_end(&mut all)
        .expect("stream");
    assert_eq!(all, expected());
    let later: Vec<String> = server.paths().split_off(before);
    assert_eq!(
        later,
        ["/hls/data001.m4s", "/hls/data002.m4s"],
        "init and the first segment were not fetched again"
    );
}

#[test]
fn the_rules_allow_only_soundcloud_https_and_resolve_relative_addresses() {
    let real = Rules::production();
    assert!(real.allows("https://playback.media-streaming.soundcloud.cloud/a/b.m4s?x=1"));
    assert!(real.allows("https://cf-preview-media.sndcdn.com/preview/0/30/a.128.mp3"));
    for bad in [
        "http://playback.media-streaming.soundcloud.cloud/a",
        "https://evil.example/a",
        "https://soundcloud.com.evil.example/a",
        "https://user@soundcloud.com/a",
        "file:///etc/passwd",
    ] {
        assert!(!real.allows(bad), "{bad}");
    }
    assert_eq!(
        real.resolve(
            "https://playback.media-streaming.soundcloud.cloud/a/b/playlist.m3u8?x=1",
            "init.mp4"
        )
        .as_deref(),
        Some("https://playback.media-streaming.soundcloud.cloud/a/b/init.mp4")
    );
    assert_eq!(
        real.resolve(
            "https://playback.media-streaming.soundcloud.cloud/a/p.m3u8",
            "https://evil.example/x"
        ),
        None
    );
    assert_eq!(real.retry_delays.len() + 1, fetch::SEGMENT_TRIES);
}
