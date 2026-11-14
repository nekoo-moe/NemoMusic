//! Unit tests for the ranged HTTP stream.

use std::sync::Arc;

use nemo_core::audio::{Codec, Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::stream::Transport;

use super::*;
use crate::test_server::{Request, Response, TestServer, range_response};

/// Deterministic bytes that differ between neighbouring offsets.
fn sample(len: usize) -> Arc<Vec<u8>> {
    Arc::new(
        (0..len)
            .map(|i| u8::try_from((i * 7 + i / 251) % 256).expect("fits u8"))
            .collect(),
    )
}

fn serve(data: &Arc<Vec<u8>>) -> TestServer {
    let data = Arc::clone(data);
    TestServer::start(move |request, _| range_response(&data, request))
}

fn open(server: &TestServer) -> HttpStream {
    HttpStream::open(&server.url("/audio"), false).expect("opens")
}

fn ranges(server: &TestServer) -> Vec<(u64, Option<u64>)> {
    server
        .requests()
        .iter()
        .filter_map(|r| r.range)
        .collect::<Vec<_>>()
}

#[test]
fn window_is_between_read_ahead_and_chunk_cap() {
    assert_eq!(window(1), READ_AHEAD_BYTES);
    assert_eq!(window(4096), READ_AHEAD_BYTES);
    assert_eq!(window(3 * 1024 * 1024), 3 * 1024 * 1024);
    assert_eq!(window(u64::MAX), MAX_CHUNK_BYTES);
}

#[test]
fn content_range_parsing() {
    assert_eq!(
        parse_content_range("bytes 0-99/1000"),
        Some(ContentRange::Span {
            first: 0,
            last: 99,
            total: Some(1000)
        })
    );
    assert_eq!(
        parse_content_range("bytes 5-6/*"),
        Some(ContentRange::Span {
            first: 5,
            last: 6,
            total: None
        })
    );
    assert_eq!(
        parse_content_range("bytes */1000"),
        Some(ContentRange::Unsatisfied { total: 1000 })
    );
    for bad in ["", "bytes", "items 0-1/2", "bytes a-b/c", "bytes 1/2"] {
        assert_eq!(parse_content_range(bad), None, "{bad:?}");
    }
}

#[test]
fn reads_the_whole_resource_in_bounded_ranges() {
    let data = sample(3 * 1024 * 1024 + 123);
    let server = serve(&data);
    let mut stream = open(&server);
    assert_eq!(stream.byte_len(), Some(data.len() as u64));
    let mut all = Vec::new();
    stream.read_to_end(&mut all).expect("reads");
    assert_eq!(all.len(), data.len());
    assert!(all == *data, "content differs");
    let seen = ranges(&server);
    assert!(seen.len() >= 3, "several windows expected, got {seen:?}");
    for (first, last) in seen {
        let last = last.expect("bounded range");
        assert!(last - first < MAX_CHUNK_BYTES);
    }
}

#[test]
fn a_huge_read_buffer_never_asks_for_more_than_ten_mib() {
    let data = sample(25 * 1024 * 1024);
    let server = serve(&data);
    let mut stream = open(&server);
    let mut big = vec![0_u8; 20 * 1024 * 1024];
    stream.seek(SeekFrom::Start(4 * 1024 * 1024)).expect("seek");
    let n = stream.read(&mut big).expect("reads");
    assert!(n > 0 && n as u64 <= MAX_CHUNK_BYTES);
    for (first, last) in ranges(&server) {
        assert!(last.expect("bounded") - first < MAX_CHUNK_BYTES);
    }
}

#[test]
fn seeks_backwards_and_forwards() {
    let data = sample(3 * 1024 * 1024 + 7);
    let server = serve(&data);
    let mut stream = open(&server);
    let mut head = [0_u8; 64];
    stream.read_exact(&mut head).expect("head");
    assert_eq!(head[..], data[..64]);

    // Forward, outside the buffered window.
    assert_eq!(
        stream.seek(SeekFrom::Start(2_500_000)).expect("seek"),
        2_500_000
    );
    let mut mid = [0_u8; 100];
    stream.read_exact(&mut mid).expect("mid");
    assert_eq!(mid[..], data[2_500_000..2_500_100]);

    // Backwards, outside the buffered window.
    stream.seek(SeekFrom::Start(10)).expect("seek");
    let mut early = [0_u8; 50];
    stream.read_exact(&mut early).expect("early");
    assert_eq!(early[..], data[10..60]);

    // Relative and from the end.
    stream.seek(SeekFrom::Current(1000)).expect("seek");
    let mut rel = [0_u8; 10];
    stream.read_exact(&mut rel).expect("rel");
    assert_eq!(rel[..], data[1060..1070]);
    let end = stream.seek(SeekFrom::End(-4096)).expect("seek");
    assert_eq!(end, data.len() as u64 - 4096);
    let mut tail = Vec::new();
    stream.read_to_end(&mut tail).expect("tail");
    assert_eq!(tail[..], data[data.len() - 4096..]);
}

#[test]
fn seeking_before_the_start_fails() {
    let data = sample(2048);
    let server = serve(&data);
    let mut stream = open(&server);
    assert!(stream.seek(SeekFrom::Current(-1)).is_err());
    assert!(stream.seek(SeekFrom::End(-5000)).is_err());
}

#[test]
fn eof_gives_zero_without_another_request() {
    let data = sample(10_000);
    let server = serve(&data);
    let mut stream = open(&server);
    let before = server.requests().len();
    let mut one = [0_u8; 8];
    stream.seek(SeekFrom::Start(10_000)).expect("seek");
    assert_eq!(stream.read(&mut one).expect("eof"), 0);
    stream.seek(SeekFrom::Start(99_999)).expect("seek past end");
    assert_eq!(stream.read(&mut one).expect("eof"), 0);
    assert_eq!(server.requests().len(), before);
    // Seeking back still works.
    stream.seek(SeekFrom::Start(9_990)).expect("seek");
    let mut rest = Vec::new();
    stream.read_to_end(&mut rest).expect("rest");
    assert_eq!(rest[..], data[9_990..]);
}

#[test]
fn a_416_answer_is_end_of_file_and_teaches_the_length() {
    let data = sample(5000);
    let server = serve(&data);
    let mut stream = open(&server);
    // Forget what we know, so the next read goes to the server.
    stream.len = None;
    stream.seek(SeekFrom::Start(7000)).expect("seek");
    let mut one = [0_u8; 8];
    assert_eq!(stream.read(&mut one).expect("416 is EOF"), 0);
    assert_eq!(stream.byte_len(), Some(5000));
    let last = server.requests().last().cloned().expect("a request");
    assert_eq!(last.range, Some((7000, Some(7000 + READ_AHEAD_BYTES - 1))));
}

#[test]
fn a_416_without_a_size_still_ends_the_stream() {
    let data = sample(5000);
    let payload = Arc::clone(&data);
    let server = TestServer::start(move |request: &Request, _| match request.range {
        Some((start, _)) if start >= 5000 => Response::new(416, Vec::new()),
        _ => range_response(&payload, request),
    });
    let mut stream = open(&server);
    stream.len = None;
    stream.seek(SeekFrom::Start(6000)).expect("seek");
    let mut one = [0_u8; 8];
    assert_eq!(stream.read(&mut one).expect("EOF"), 0);
    assert_eq!(stream.byte_len(), Some(6000));
}

#[test]
fn transient_errors_are_retried_twice() {
    let data = sample(4096);
    let payload = Arc::clone(&data);
    let server = TestServer::start(move |request: &Request, index| {
        if index < 2 {
            Response::new(500, Vec::new())
        } else {
            range_response(&payload, request)
        }
    });
    let mut stream = open(&server);
    let mut all = Vec::new();
    stream.read_to_end(&mut all).expect("reads");
    assert_eq!(all, *data);
    assert_eq!(server.requests().len(), 3);
}

#[test]
fn three_failures_in_a_row_give_a_network_error() {
    let server = TestServer::start(|_, _| Response::new(500, Vec::new()));
    let error = HttpStream::open(&server.url("/audio"), false).unwrap_err();
    assert!(matches!(error, StreamError::Network(_)), "{error:?}");
    assert_eq!(server.requests().len(), 3);
}

#[test]
fn missing_or_gone_streams_are_not_available_and_not_retried() {
    for status in [404_u16, 410] {
        let server = TestServer::start(move |_, _| Response::new(status, Vec::new()));
        let error = HttpStream::open(&server.url("/audio"), false).unwrap_err();
        assert_eq!(error, StreamError::NotAvailable);
        assert_eq!(server.requests().len(), 1);
    }
    let server = TestServer::start(|_, _| Response::new(403, Vec::new()));
    let error = HttpStream::open(&server.url("/audio"), false).unwrap_err();
    assert!(matches!(error, StreamError::Network(_)));
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn a_server_that_ignores_ranges_works_only_from_the_start() {
    let data = sample(3 * 1024 * 1024);
    let payload = Arc::clone(&data);
    let server = TestServer::start(move |_, _| Response::new(200, payload.to_vec()));
    let mut stream = open(&server);
    assert_eq!(stream.byte_len(), Some(data.len() as u64));
    let mut head = [0_u8; 16];
    stream.read_exact(&mut head).expect("first window");
    assert_eq!(head[..], data[..16]);
    stream.seek(SeekFrom::Start(2 * 1024 * 1024)).expect("seek");
    assert!(stream.read(&mut head).is_err());
}

#[test]
fn redirects_to_plain_http_are_not_followed() {
    let server = TestServer::start(|_, _| {
        Response::new(302, Vec::new()).header("Location", "http://127.0.0.1:1/elsewhere")
    });
    let error = HttpStream::open(&server.url("/audio"), false).unwrap_err();
    assert!(matches!(error, StreamError::Network(_)), "{error:?}");
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn plain_http_is_refused_by_the_public_opener() {
    let data = sample(100);
    let server = serve(&data);
    let stream = ResolvedStream {
        url: Secret::new(server.url("/audio")),
        info: StreamInfo {
            origin: Origin::YoutubeMusic,
            codec: Codec::Opus,
            bitrate_kbps: None,
            format: None,
        },
        expires_at: None,
        transport: Transport::File,
        preview: false,
        audio_language: None,
        audio_tracks: Vec::new(),
    };
    let error = HttpStreamOpener::new()
        .open(&stream)
        .err()
        .expect("refused");
    assert!(matches!(error, StreamError::Network(_)));
    assert!(server.requests().is_empty(), "no request may be sent");
    // The test-only opener reaches the loopback server.
    let source = HttpStreamOpener::allow_http_for_tests()
        .open(&stream)
        .expect("opens over loopback");
    assert_eq!(source.byte_len(), Some(100));
}

#[test]
fn errors_and_debug_never_show_the_url() {
    let server = TestServer::start(|_, _| Response::new(500, Vec::new()));
    let url = server.url("/secret-path?sig=SECRET");
    let error = HttpStream::open(&url, false).unwrap_err();
    let text = format!("{error} {error:?}");
    assert!(
        !text.contains("127.0.0.1") && !text.contains("SECRET"),
        "{text}"
    );

    let data = sample(100);
    let ok_server = serve(&data);
    let stream = HttpStream::open(&ok_server.url("/a?sig=SECRET"), false).expect("opens");
    let text = format!("{stream:?}");
    assert!(
        !text.contains("127.0.0.1") && !text.contains("SECRET"),
        "{text}"
    );
}
