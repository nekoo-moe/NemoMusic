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
/// Size of the first request when a stream is opened, in bytes. Smaller than the read-ahead
/// so the container header (an `.m4a` keeps it at the start) and the first seconds of audio
/// arrive sooner; about 16 s of a 128 kbps stream.
pub const FIRST_WINDOW_BYTES: u64 = 256 * 1024;
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
            .fetch_window(0, FIRST_WINDOW_BYTES)
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
        self.fetch_window(start, window(want))
    }

    /// Fill the buffer with `size` bytes from `start` (fewer at the end of the resource).
    fn fetch_window(&mut self, start: u64, size: u64) -> Result<(), FetchError> {
        let mut end = start.saturating_add(size.clamp(1, MAX_CHUNK_BYTES));
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
mod tests;
