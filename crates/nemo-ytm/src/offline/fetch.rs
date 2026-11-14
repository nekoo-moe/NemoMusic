//! Downloading one song into the cache (offline-cache task 7, criteria 11 and 13).
//!
//! One blocking `GET` per call, with one `Range: bytes=<part length>-` header, streamed into
//! the `.part` file in reads of at most [`READ_BYTES`]. The pause flag is checked between
//! reads, so a pause stops within one read. After the last byte the length is compared to what
//! the server announced, the file is synced and renamed into `blobs/`. A `200` answer to a
//! range means the server ignored it: the part file restarts from zero. The bytes are kept as
//! served, with no re-encoding.
//!
//! The URL, the file names and the video id are never logged or put in an error. Call this
//! from the download worker thread only: the blocking client must not run inside an async
//! task.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use nemo_core::secret::Secret;
use reqwest::blocking::{Client, Response};
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, RANGE, RETRY_AFTER};

use crate::http::{https_redirects, is_https, parse_retry_after};

/// Most bytes one read takes from the body.
pub(super) const READ_BYTES: usize = 1024 * 1024;
/// Connect timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Longest wait for one read (the blocking client applies it per read, not to the whole body).
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// A download failed. No variant carries a URL, path or video id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FetchError {
    /// The client could not be built, the URL was refused, or the request failed or timed
    /// out.
    Network,
    /// The server answered with a status other than 200, 206 or 429.
    Status(u16),
    /// The server asked to slow down (429), with its `Retry-After` when given in seconds.
    RateLimited(Option<Duration>),
    /// The body ended before the announced length, no length was announced, or the server
    /// answered a different range.
    Length,
    /// The part file or the saved file could not be written.
    Io,
}

impl From<std::io::Error> for FetchError {
    fn from(_: std::io::Error) -> Self {
        // Dropped on purpose: I/O errors can name the path.
        Self::Io
    }
}

/// How a download call ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Fetched {
    /// Every byte arrived and the file is in `blobs/`.
    Complete {
        /// Size of the saved file.
        bytes: u64,
    },
    /// The pause flag was set; the part file keeps `done` bytes for a later resume.
    Paused {
        /// Bytes in the part file.
        done: u64,
    },
}

/// The files of one download: the `.part` file it writes and where the finished song goes.
#[derive(Debug, Clone, Copy)]
pub(super) struct Target<'a> {
    /// `tmp/<opaque>.part`.
    pub(super) part: &'a Path,
    /// `blobs/<opaque>`.
    pub(super) dest: &'a Path,
}

/// The ranged downloader. One per worker; it holds one HTTP client.
pub(super) struct Fetcher {
    client: Client,
    https_only: bool,
}

impl Fetcher {
    /// A downloader. `https_only` is `false` only for the loopback test server.
    pub(super) fn new(https_only: bool) -> Result<Self, FetchError> {
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(READ_TIMEOUT)
            .redirect(https_redirects())
            .https_only(https_only)
            .build()
            .map_err(|_| FetchError::Network)?;
        Ok(Self { client, https_only })
    }

    /// Download `url` into `target.part`, continuing from its length, and move it to
    /// `target.dest` once complete. `progress` gets the bytes written so far and the total
    /// after every read.
    pub(super) fn fetch(
        &self,
        url: &Secret<String>,
        target: Target<'_>,
        pause: &AtomicBool,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<Fetched, FetchError> {
        if self.https_only && !is_https(url.expose()) {
            return Err(FetchError::Network);
        }
        let have = std::fs::metadata(target.part).map_or(0, |meta| meta.len());
        let response = self
            .client
            .get(url.expose())
            .header(RANGE, format!("bytes={have}-"))
            .send()
            .map_err(|_| FetchError::Network)?;
        let Some(start) = start_of(&response, have)? else {
            // 416 for a part file that already holds every byte.
            return finish(target, have);
        };
        let mut file = open_part(target.part, start)?;
        match copy_body(response, &mut file, start, pause, progress)? {
            Fetched::Paused { done } => {
                file.sync_all()?;
                Ok(Fetched::Paused { done })
            }
            Fetched::Complete { bytes } => {
                file.sync_all()?;
                drop(file);
                finish(target, bytes)
            }
        }
    }
}

/// The offset the body starts at, or `None` when the part file is already complete.
fn start_of(response: &Response, have: u64) -> Result<Option<u64>, FetchError> {
    match response.status().as_u16() {
        206 => match content_range(response) {
            Some((first, _)) if first == have => Ok(Some(have)),
            _ => Err(FetchError::Length),
        },
        // The server ignored the range: the whole body follows, so start again.
        200 => Ok(Some(0)),
        416 => match content_range(response) {
            Some((_, Some(total))) if total == have && have > 0 => Ok(None),
            _ => Err(FetchError::Status(416)),
        },
        429 => Err(FetchError::RateLimited(
            response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(parse_retry_after),
        )),
        code => Err(FetchError::Status(code)),
    }
}

/// `Content-Range: bytes first-last/total` as `(first, total)`, or `bytes */total` as
/// `(0, total)`.
fn content_range(response: &Response) -> Option<(u64, Option<u64>)> {
    let value = response.headers().get(CONTENT_RANGE)?.to_str().ok()?;
    let rest = value.trim().strip_prefix("bytes")?.trim_start();
    let (span, total) = rest.split_once('/')?;
    let total = total.trim().parse().ok();
    let span = span.trim();
    if span == "*" {
        return Some((0, total));
    }
    let (first, _) = span.split_once('-')?;
    Some((first.trim().parse().ok()?, total))
}

/// The announced end of the body: the `Content-Range` total, else `start + Content-Length`.
fn expected_total(response: &Response, start: u64) -> Option<u64> {
    if let Some((_, Some(total))) = content_range(response) {
        return Some(total);
    }
    let len: u64 = response
        .headers()
        .get(CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .parse()
        .ok()?;
    start.checked_add(len)
}

/// Open the part file to continue at `start`: append when resuming, truncate when starting.
fn open_part(path: &Path, start: u64) -> Result<File, FetchError> {
    let file = if start == 0 {
        File::create(path)?
    } else {
        OpenOptions::new().append(true).open(path)?
    };
    Ok(file)
}

/// Copy the body into `file` in reads of at most [`READ_BYTES`], checking `pause` before each.
fn copy_body(
    mut response: Response,
    file: &mut File,
    start: u64,
    pause: &AtomicBool,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<Fetched, FetchError> {
    let total = expected_total(&response, start);
    let mut done = start;
    let mut buf = vec![0_u8; READ_BYTES];
    loop {
        if pause.load(Ordering::SeqCst) {
            return Ok(Fetched::Paused { done });
        }
        let n = response.read(&mut buf).map_err(|_| FetchError::Network)?;
        if n == 0 {
            break;
        }
        let chunk = buf.get(..n).ok_or(FetchError::Io)?;
        file.write_all(chunk)?;
        done += u64::try_from(n).map_err(|_| FetchError::Io)?;
        progress(done, total);
    }
    if total == Some(done) {
        Ok(Fetched::Complete { bytes: done })
    } else {
        // The part file stays, so a later call resumes from `done`.
        Err(FetchError::Length)
    }
}

/// Move the complete part file into `blobs/`.
fn finish(target: Target<'_>, bytes: u64) -> Result<Fetched, FetchError> {
    std::fs::rename(target.part, target.dest)?;
    Ok(Fetched::Complete { bytes })
}

#[cfg(test)]
mod tests;
