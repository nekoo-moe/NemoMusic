//! Tests for the opener and the prefetch over a loopback server and the recorded AAC segments.

use std::time::SystemTime;

use nemo_core::audio::{Codec, Origin, StreamInfo};
use nemo_core::secret::Secret;

use super::*;
use crate::hls::Rules;
use crate::test_server::{Response, TestServer};

const INIT: &[u8] = include_bytes!("../../tests/fixtures/hls/init.mp4");
const SEGMENT: &[u8] = include_bytes!("../../tests/fixtures/hls/data000.m4s");

const PLAYLIST: &str =
    "#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:2.0,\ndata000.m4s\n#EXT-X-ENDLIST\n";
const KEYED: &str = "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://k\"\n#EXT-X-MAP:URI=\"init.mp4\"\n\
#EXTINF:2.0,\ndata000.m4s\n#EXT-X-ENDLIST\n";

fn serve(playlist: &'static str) -> TestServer {
    TestServer::start(
        move |path, _| match path.split('?').next().unwrap_or(path) {
            "/hls/out.m3u8" => Response::new(200, playlist),
            "/hls/init.mp4" => Response::new(200, INIT),
            "/hls/data000.m4s" => Response::new(200, SEGMENT),
            "/hls/gone.m3u8" => Response::new(403, ""),
            _ => Response::new(404, ""),
        },
    )
}

fn stream(url: String, transport: Transport) -> ResolvedStream {
    ResolvedStream {
        url: Secret::new(url),
        info: StreamInfo {
            origin: Origin::SoundCloud,
            codec: Codec::Aac,
            bitrate_kbps: Some(160),
            format: None,
        },
        expires_at: SystemTime::now().checked_add(Duration::from_mins(3)),
        transport,
        preview: false,
        audio_language: None,
        audio_tracks: Vec::new(),
    }
}

fn hls() -> Transport {
    Transport::Hls { live: false }
}

fn opener() -> ScStreamOpener {
    ScStreamOpener::new(Arc::new(Shared::with(Rules::loopback())))
}

fn read_all(source: &mut dyn ByteSource) -> Vec<u8> {
    let mut all = Vec::new();
    source.read_to_end(&mut all).expect("stream");
    all
}

#[test]
fn an_hls_stream_opens_and_reads_init_then_segment() {
    let server = serve(PLAYLIST);
    let mut source = opener()
        .open(&stream(server.url("/hls/out.m3u8"), hls()))
        .expect("source");
    let mut expected = INIT.to_vec();
    expected.extend_from_slice(SEGMENT);
    assert_eq!(read_all(source.as_mut()), expected);
}

#[test]
fn only_hls_is_this_openers_business() {
    let server = serve(PLAYLIST);
    let o = opener();
    for transport in [Transport::File, Transport::Hls { live: true }] {
        let err = o
            .open(&stream(server.url("/hls/out.m3u8"), transport))
            .err();
        assert_eq!(err, Some(StreamError::NotAvailable));
    }
    assert!(server.paths().is_empty(), "nothing was requested");
}

#[test]
fn a_playlist_with_a_key_is_not_available_and_no_segment_is_fetched() {
    let server = serve(KEYED);
    let err = opener()
        .open(&stream(server.url("/hls/out.m3u8"), hls()))
        .err();
    assert_eq!(err, Some(StreamError::NotAvailable));
    assert_eq!(
        server.paths(),
        ["/hls/out.m3u8"],
        "the app never reads an encrypted stream"
    );
}

#[test]
fn an_expired_address_is_not_available_and_a_dead_one_is_a_network_error() {
    let server = serve(PLAYLIST);
    let err = opener()
        .open(&stream(server.url("/hls/gone.m3u8"), hls()))
        .err();
    assert_eq!(
        err,
        Some(StreamError::NotAvailable),
        "403: the player looks the stream up again"
    );
    let err = opener()
        .open(&stream("http://127.0.0.1:1/x.m3u8".to_owned(), hls()))
        .err();
    assert!(matches!(err, Some(StreamError::Network(text)) if !text.contains("127.0.0.1")));
}

#[test]
fn an_address_off_the_allowed_hosts_is_never_fetched() {
    let o = ScStreamOpener::new(Arc::new(Shared::new()));
    for url in [
        "https://evil.example/a.m3u8",
        "http://playback.media-streaming.soundcloud.cloud/a.m3u8",
    ] {
        let err = o.open(&stream(url.to_owned(), hls())).err();
        assert!(
            matches!(err, Some(StreamError::Network(_) | StreamError::Parse(_))),
            "{url}"
        );
    }
}

#[test]
fn a_prefetched_stream_opens_without_new_requests_for_the_first_bytes() {
    let server = serve(PLAYLIST);
    let o = opener();
    let url = server.url("/hls/out.m3u8");
    let s = stream(url.clone(), hls());
    assert!(!o.shared().has(&url));
    o.shared().prefetch(&s).expect("prefetch");
    assert!(o.shared().has(&url));
    let mut fetched = server.paths();
    fetched.sort();
    assert_eq!(
        fetched,
        ["/hls/data000.m4s", "/hls/init.mp4", "/hls/out.m3u8"]
    );
    let before = server.paths().len();
    let mut source = o.open(&s).expect("source");
    assert!(o.shared().has(&url), "kept for a replay");
    let mut expected = INIT.to_vec();
    expected.extend_from_slice(SEGMENT);
    assert_eq!(read_all(source.as_mut()), expected);
    assert_eq!(
        server.paths().len(),
        before,
        "playlist, init and first segment were not fetched again"
    );
}

#[test]
fn a_forgotten_prefetch_is_not_used_and_one_stream_is_one_slot() {
    let server = serve(PLAYLIST);
    let o = opener();
    let url = server.url("/hls/out.m3u8");
    let s = stream(url.clone(), hls());
    o.shared().prefetch(&s).expect("prefetch");
    o.shared().forget(&url);
    assert!(!o.shared().has(&url));
    // A file stream and a live stream are not prefetched.
    o.shared()
        .prefetch(&stream(url.clone(), Transport::File))
        .expect("ignored");
    assert!(!o.shared().has(&url));
    // The same stream twice is one slot.
    o.shared().prefetch(&s).expect("again");
    o.shared().prefetch(&s).expect("and again");
    assert!(o.shared().has(&url));
    let mut source = o.open(&s).expect("source");
    assert!(!read_all(source.as_mut()).is_empty());
    assert!(o.shared().has(&url), "still there for a replay");
}

#[test]
fn only_the_last_slots_are_kept() {
    let server = serve(PLAYLIST);
    let o = opener();
    let urls: Vec<String> = (0..=SLOTS)
        .map(|n| server.url(&format!("/hls/out.m3u8?track={n}")))
        .collect();
    let paths = server.paths().len();
    assert_eq!(paths, 0);
    for url in &urls {
        o.shared()
            .prefetch(&stream(url.clone(), hls()))
            .expect("prefetch");
    }
    assert!(!o.shared().has(&urls[0]), "the oldest went");
    for url in &urls[1..] {
        assert!(o.shared().has(url));
    }
}

#[test]
fn one_client_is_reused_and_dropping_the_state_inside_a_runtime_is_safe() {
    let shared = Shared::with(Rules::loopback());
    let a = shared.client().expect("client");
    let b = shared.client().expect("same client");
    drop((a, b));
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    rt.block_on(async move {
        // The blocking client would panic if dropped here; `Shared` drops it on its own thread.
        drop(shared);
    });
    std::thread::sleep(Duration::from_millis(100));
}

#[test]
fn a_replay_of_a_stream_needs_no_new_playlist() {
    let server = serve(PLAYLIST);
    let o = opener();
    let address = server.url("/hls/out.m3u8");
    let mut expected = INIT.to_vec();
    expected.extend_from_slice(SEGMENT);
    for _ in 0..2 {
        let mut source = o.open(&stream(address.clone(), hls())).expect("source");
        assert_eq!(read_all(source.as_mut()), expected);
    }
    let playlists = server
        .paths()
        .iter()
        .filter(|p| p.contains("out.m3u8"))
        .count();
    assert_eq!(playlists, 1, "the second open reused the kept playlist");
    // A kept playlist without a head is not a prefetch.
    assert!(!o.shared().has(&address));
}

#[test]
fn the_connection_to_the_media_host_is_opened_ahead_once_in_a_while() {
    let server = serve(PLAYLIST);
    let shared = Shared::with_preconnect(Rules::loopback(), server.url("/media-root"));
    assert!(shared.preconnect_due(), "the first time it is due");
    assert!(!shared.preconnect_due(), "and not again at once");
    shared.preconnect();
    assert_eq!(
        server.paths(),
        ["/media-root"],
        "one request, to the media host's root"
    );
}

#[test]
fn a_failed_preconnect_is_harmless() {
    let shared = Shared::with_preconnect(Rules::loopback(), "http://127.0.0.1:1/".to_owned());
    shared.preconnect();
    assert!(shared.client().is_ok(), "the client is still usable");
}
