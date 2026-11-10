//! Reading a resolved stream over HTTPS with range requests.
//!
//! [`HttpStreamOpener`] gives the decoder a seekable [`ByteSource`]. Rules:
//! - HTTPS only, also across redirects. A redirect to another scheme is not followed.
//! - Every request asks for a byte range of at most [`MAX_CHUNK_BYTES`] (10 MiB). `yt-dlp` uses
//!   the same chunk size for `googlevideo` because the server slows down very large
//!   single requests (observed behaviour is in the task report, not guaranteed).
//! - The reader keeps the last fetched window in memory (at least [`READ_AHEAD_BYTES`], 1 MiB),
//!   so small decoder reads do not cause one request each.
//! - A failed request is retried [`MAX_RETRIES`] times for network errors, 429 and 5xx.
//! - The URL is never logged and never part of an error text.
//!
//! Reads block. The decoder thread calls them, never the audio callback. Do not call
//! [`StreamOpener::open`] from an async task: the blocking HTTP client starts its own runtime.

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom};
use std::time::Duration;

use nemo_core::services::{ByteSource, StreamOpener};
use nemo_core::stream::{ResolvedStream, StreamError};
use reqwest::blocking::Client;
use reqwest::header::{CONTENT_RANGE, RANGE};

use crate::http::{https_redirects, is_https};

/// Largest range one request may ask for, in bytes.
pub const MAX_CHUNK_BYTES: u64 = 10 * 1024 * 1024;
/// Smallest window fetched per request, in bytes.
pub const READ_AHEAD_BYTES: u64 = 1024 * 1024;
/// Extra attempts after the first one.
pub const MAX_RETRIES: u32 = 2;
/// Connect timeout and total timeout of one request.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Window size for a read of `want` bytes: at least the read-ahead, at most the chunk cap.
const fn window(want: u64) -> u64 {
    if want < READ_AHEAD_BYTES {
        READ_AHEAD_BYTES
    } else if want > MAX_CHUNK_BYTES {
        MAX_CHUNK_BYTES
    } else {
        want
    }
}

/// A parsed `Content-Range` header value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentRange {
    /// `bytes first-last/total` (`total` is `None` for `*`).
    Span {
        first: u64,
        last: u64,
        total: Option<u64>,
    },
    /// `bytes */total`, the answer to an unsatisfiable range.
    Unsatisfied { total: u64 },
}

fn parse_content_range(value: &str) -> Option<ContentRange> {
    let rest = value.trim().strip_prefix("bytes")?.trim_start();
    let (span, total) = rest.split_once('/')?;
    let total = total.trim().parse::<u64>().ok();
    let span = span.trim();
    if span == "*" {
        return total.map(|total| ContentRange::Unsatisfied { total });
    }
    let (first, last) = span.split_once('-')?;
    Some(ContentRange::Span {
        first: first.trim().parse().ok()?,
        last: last.trim().parse().ok()?,
        total,
    })
}

/// A failed fetch. The text never contains the URL.
#[derive(Debug)]
struct FetchError {
    transient: bool,
    status: Option<u16>,
    message: String,
}

impl FetchError {
    fn transient(message: &str) -> Self {
        Self {
            transient: true,
            status: None,
            message: message.to_owned(),
        }
    }

    fn fatal(message: &str) -> Self {
        Self {
            transient: false,
            status: None,
            message: message.to_owned(),
        }
    }

    fn status(code: u16) -> Self {
        Self {
            transient: code == 429 || (500..=599).contains(&code),
            status: Some(code),
            message: format!("HTTP status {code}"),
        }
    }

    fn into_stream_error(self) -> StreamError {
        match self.status {
            Some(404 | 410) => StreamError::NotAvailable,
            _ => StreamError::Network(self.message),
        }
    }
}

impl From<FetchError> for io::Error {
    fn from(error: FetchError) -> Self {
        Self::other(error.message)
    }
}

/// One successful fetch.
struct Fetched {
    bytes: Vec<u8>,
    total: Option<u64>,
    /// The server said the start is at or past the end.
    past_end: bool,
}

/// A seekable HTTP resource read with range requests.
struct HttpStream {
    client: Client,
    url: String,
    pos: u64,
    len: Option<u64>,
    buf: Vec<u8>,
    buf_start: u64,
}

impl fmt::Debug for HttpStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpStream")
            .field("pos", &self.pos)
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

fn build_client(https_only: bool) -> Result<Client, StreamError> {
    Client::builder()
        .connect_timeout(REQUEST_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .redirect(https_redirects())
        .https_only(https_only)
        .build()
        .map_err(|_| StreamError::Network("could not build the HTTP client".to_owned()))
}

impl HttpStream {
    /// Open `url` and fetch the first window, which also tells the total size.
    fn open(url: &str, https_only: bool) -> Result<Self, StreamError> {
        if https_only && !is_https(url) {
            return Err(StreamError::Network("stream URL is not HTTPS".to_owned()));
        }
        let mut stream = Self {
            client: build_client(https_only)?,
            url: url.to_owned(),
            pos: 0,
            len: None,
            buf: Vec::new(),
            buf_start: 0,
        };
        stream
            .fetch(0, READ_AHEAD_BYTES)
            .map_err(FetchError::into_stream_error)?;
        if stream.buf.is_empty() {
            return Err(StreamError::Network("empty stream".to_owned()));
        }
        Ok(stream)
    }

    /// One request for `start..=last`.
    fn try_fetch(&self, start: u64, last: u64) -> Result<Fetched, FetchError> {
        let wanted = last - start + 1;
        let response = self
            .client
            .get(&self.url)
            .header(RANGE, format!("bytes={start}-{last}"))
            .send()
            .map_err(|e| {
                if e.is_timeout() {
                    FetchError::transient("request timed out")
                } else if e.is_redirect() {
                    FetchError::fatal("redirect refused")
                } else if e.is_builder() {
                    FetchError::fatal("invalid stream request")
                } else {
                    FetchError::transient("connection failed")
                }
            })?;
        let code = response.status().as_u16();
        let content_range = response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_content_range);
        let total = match (code, content_range) {
            (206, Some(ContentRange::Span { first, total, .. })) => {
                if first != start {
                    return Err(FetchError::fatal("server answered a different range"));
                }
                total
            }
            (206, _) => return Err(FetchError::fatal("missing Content-Range")),
            (200, _) => {
                if start != 0 {
                    return Err(FetchError::fatal("server ignores range requests"));
                }
                response.content_length()
            }
            (416, range) => {
                let total = match range {
                    Some(ContentRange::Unsatisfied { total }) => Some(total),
                    _ => None,
                };
                return Ok(Fetched {
                    bytes: Vec::new(),
                    total,
                    past_end: true,
                });
            }
            (other, _) => return Err(FetchError::status(other)),
        };
        let capacity = usize::try_from(wanted).unwrap_or(0);
        let mut bytes = Vec::with_capacity(capacity);
        response
            .take(wanted)
            .read_to_end(&mut bytes)
            .map_err(|_| FetchError::transient("connection lost while reading"))?;
        if bytes.is_empty() {
            return Err(FetchError::transient("empty answer"));
        }
        Ok(Fetched {
            bytes,
            total,
            past_end: false,
        })
    }

    /// Fill the buffer with a window that starts at `start` and covers a read of `want` bytes.
    fn fetch(&mut self, start: u64, want: u64) -> Result<(), FetchError> {
        let mut end = start.saturating_add(window(want));
        if let Some(len) = self.len {
            end = end.min(len);
        }
        if end <= start {
            self.buf.clear();
            self.buf_start = start;
            return Ok(());
        }
        let last = end - 1;
        let mut attempt = 0_u32;
        let fetched = loop {
            match self.try_fetch(start, last) {
                Ok(fetched) => break fetched,
                Err(error) if error.transient && attempt < MAX_RETRIES => {
                    attempt += 1;
                    std::thread::sleep(Duration::from_millis(250 * u64::from(attempt)));
                }
                Err(error) => return Err(error),
            }
        };
        if let Some(total) = fetched.total {
            self.len = Some(total);
        } else if fetched.past_end {
            // The server gave no size; the resource ends at or before `start`.
            self.len = Some(start);
        }
        self.buf = fetched.bytes;
        self.buf_start = start;
        Ok(())
    }

    fn buffered(&self, pos: u64) -> bool {
        let buffered = u64::try_from(self.buf.len()).unwrap_or(u64::MAX);
        pos >= self.buf_start && pos - self.buf_start < buffered
    }
}

impl Read for HttpStream {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() || self.len.is_some_and(|len| self.pos >= len) {
            return Ok(0);
        }
        if !self.buffered(self.pos) {
            let want = u64::try_from(out.len()).unwrap_or(MAX_CHUNK_BYTES);
            self.fetch(self.pos, want)?;
            if !self.buffered(self.pos) {
                return Ok(0);
            }
        }
        let offset = usize::try_from(self.pos - self.buf_start)
            .map_err(|_| io::Error::other("offset out of range"))?;
        let count = out.len().min(self.buf.len() - offset);
        out[..count].copy_from_slice(&self.buf[offset..offset + count]);
        self.pos += u64::try_from(count).unwrap_or(0);
        Ok(count)
    }
}

impl Seek for HttpStream {
    fn seek(&mut self, target: SeekFrom) -> io::Result<u64> {
        let next = match target {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::Current(delta) => i128::from(self.pos) + i128::from(delta),
            SeekFrom::End(delta) => {
                let len = self.len.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::Unsupported, "stream length is unknown")
                })?;
                i128::from(len) + i128::from(delta)
            }
        };
        self.pos = u64::try_from(next)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"))?;
        Ok(self.pos)
    }
}

impl ByteSource for HttpStream {
    fn byte_len(&self) -> Option<u64> {
        self.len
    }
}

/// Opens resolved streams as seekable HTTPS readers.
#[derive(Debug, Clone, Copy)]
pub struct HttpStreamOpener {
    https_only: bool,
}

impl HttpStreamOpener {
    /// An opener that accepts HTTPS URLs only.
    #[must_use]
    pub const fn new() -> Self {
        Self { https_only: true }
    }

    /// Test constructor: lets the loopback test server (plain HTTP) through.
    #[cfg(test)]
    const fn allow_http_for_tests() -> Self {
        Self { https_only: false }
    }
}

impl Default for HttpStreamOpener {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamOpener for HttpStreamOpener {
    fn open(&self, stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        let opened = HttpStream::open(stream.url.expose(), self.https_only)?;
        Ok(Box::new(opened))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nemo_core::audio::{Codec, Origin, StreamInfo};
    use nemo_core::secret::Secret;

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
}
