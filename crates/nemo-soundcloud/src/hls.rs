//! Reading an on-demand HLS stream as one forward byte stream for the decoder.
//!
//! SoundCloud serves most tracks as fragmented MP4 AAC in ten-second segments: an init segment
//! (`#EXT-X-MAP`) and the `.m4s` segments. [`HlsSource`] joins the init segment and the segments
//! in order, so the decoder reads one stream. One fetch thread fills a bounded channel
//! ([`READ_AHEAD_SEGMENTS`] segments), so at most that many segments wait in memory and nothing
//! is written to disk. The init segment and the first segment are fetched at the same moment, so
//! the first bytes arrive after one round trip.
//!
//! The source is forward only (`is_seekable()` is false): seeking by segment is a later wave
//! (soundcloud-full task 11). Reads block; the decoder thread calls them, never the audio
//! callback. A playlist that names a key is refused before this reader starts.

use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use nemo_core::services::ByteSource;
use reqwest::blocking::Client;

pub mod fetch;
pub mod playlist;

pub use fetch::{Failure, Rules};
pub use playlist::{MediaPlaylist, PlaylistError};

/// Segments kept ahead of the decoder.
pub const READ_AHEAD_SEGMENTS: usize = 3;

/// What the fetch thread sends the reader.
enum Msg {
    Data(Vec<u8>),
    End,
    Failed(Failure),
}

/// The init segment and the first segment, when they were fetched ahead of time.
#[derive(Debug, Clone, Default)]
pub struct Head {
    /// The init segment, if the playlist has one.
    pub init: Option<Vec<u8>>,
    /// The first media segment.
    pub first: Vec<u8>,
}

/// Fetch the playlist's init segment and first segment at the same moment.
///
/// # Errors
/// The [`Failure`] of the first request that failed.
pub fn fetch_head(client: &Client, list: &MediaPlaylist, rules: &Rules) -> Result<Head, Failure> {
    let first_url = &list.segments.first().ok_or(Failure::Network)?.url;
    thread::scope(|scope| {
        let init = list
            .init
            .as_ref()
            .map(|url| scope.spawn(|| fetch::get(client, url, rules)));
        let first = fetch::get(client, first_url, rules)?;
        let init = match init {
            Some(handle) => Some(handle.join().map_err(|_| Failure::Network)??),
            None => None,
        };
        Ok(Head { init, first })
    })
}

/// Everything the fetch thread needs.
struct Job {
    client: Client,
    rules: Rules,
    list: MediaPlaylist,
    head: Option<Head>,
}

/// The decoder's view of the stream.
pub struct HlsSource {
    /// `Mutex` only so the source is `Sync`; the reader is the one user.
    rx: Mutex<Receiver<Msg>>,
    chunk: Vec<u8>,
    at: usize,
    pos: u64,
    failure: Option<Failure>,
    ended: bool,
    stop: Arc<AtomicBool>,
    /// The sum of the playlist's segment lengths. The init segment of a fragmented stream states
    /// none, so the decoder would report the first fragment's length.
    length: Duration,
}

impl std::fmt::Debug for HlsSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HlsSource")
            .field("pos", &self.pos)
            .finish_non_exhaustive()
    }
}

impl HlsSource {
    /// Start reading `list`. `head` is what [`fetch_head`] already got, if anything. Returns at
    /// once; the first read waits for the first bytes.
    ///
    /// # Errors
    /// An [`io::Error`] when the fetch thread cannot start.
    pub fn start(
        client: Client,
        rules: Rules,
        list: MediaPlaylist,
        head: Option<Head>,
    ) -> io::Result<Self> {
        let (tx, rx) = sync_channel(READ_AHEAD_SEGMENTS);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let length = list.duration();
        let job = Job {
            client,
            rules,
            list,
            head,
        };
        thread::Builder::new()
            .name("nemo-sc-hls".into())
            .spawn(move || {
                let outcome = run(job, &tx, &flag);
                let _ = tx.send(match outcome {
                    Ok(()) => Msg::End,
                    Err(failure) => Msg::Failed(failure),
                });
            })?;
        Ok(Self {
            rx: Mutex::new(rx),
            chunk: Vec::new(),
            at: 0,
            pos: 0,
            failure: None,
            ended: false,
            stop,
            length,
        })
    }

    /// The next chunk of bytes, waiting for the fetch thread. `false` at the end or after a
    /// failure.
    fn refill(&mut self) -> bool {
        if self.ended || self.failure.is_some() {
            return false;
        }
        let rx = self.rx.get_mut().unwrap_or_else(PoisonError::into_inner);
        match rx.recv() {
            Ok(Msg::Data(bytes)) => {
                self.chunk = bytes;
                self.at = 0;
                true
            }
            Ok(Msg::End) => {
                self.ended = true;
                false
            }
            Ok(Msg::Failed(failure)) => {
                self.failure = Some(failure);
                false
            }
            Err(_) => {
                self.failure = Some(Failure::Network);
                false
            }
        }
    }
}

impl Drop for HlsSource {
    fn drop(&mut self) {
        // The fetch thread ends after the request it is in; dropping the receiver unblocks a
        // full channel. It is not joined: the control thread must not wait for the network.
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Send `bytes`; `false` when the reader is gone.
fn push(tx: &SyncSender<Msg>, stop: &AtomicBool, bytes: Vec<u8>) -> bool {
    !stop.load(Ordering::SeqCst) && tx.send(Msg::Data(bytes)).is_ok()
}

/// The fetch thread: init segment, first segment, then the rest one by one.
fn run(job: Job, tx: &SyncSender<Msg>, stop: &AtomicBool) -> Result<(), Failure> {
    let Job {
        client,
        rules,
        list,
        head,
    } = job;
    let head = match head {
        Some(head) => head,
        None => fetch_head(&client, &list, &rules)?,
    };
    if let Some(init) = head.init
        && !push(tx, stop, init)
    {
        return Ok(());
    }
    if !push(tx, stop, head.first) {
        return Ok(());
    }
    for segment in list.segments.iter().skip(1) {
        if stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        let bytes = fetch::get(&client, &segment.url, &rules)?;
        if !push(tx, stop, bytes) {
            return Ok(());
        }
    }
    Ok(())
}

impl Read for HlsSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        while self.at >= self.chunk.len() {
            if !self.refill() {
                return match self.failure {
                    Some(_) => Err(io::Error::other("the SoundCloud stream failed")),
                    None => Ok(0),
                };
            }
        }
        let n = out.len().min(self.chunk.len() - self.at);
        out[..n].copy_from_slice(&self.chunk[self.at..self.at + n]);
        self.at += n;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for HlsSource {
    /// Forward only: staying put or skipping ahead works, going back does not.
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(p) => p,
            SeekFrom::Current(d) => self.pos.checked_add_signed(d).ok_or_else(unseekable)?,
            SeekFrom::End(_) => return Err(unseekable()),
        };
        if target < self.pos {
            return Err(unseekable());
        }
        let mut scratch = [0_u8; 8192];
        while self.pos < target {
            let want =
                usize::try_from(target - self.pos).map_or(scratch.len(), |n| n.min(scratch.len()));
            if self.read(&mut scratch[..want])? == 0 {
                break;
            }
        }
        Ok(self.pos)
    }
}

fn unseekable() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "this stream cannot be seeked yet",
    )
}

impl ByteSource for HlsSource {
    fn byte_len(&self) -> Option<u64> {
        None
    }

    fn is_seekable(&self) -> bool {
        false
    }

    fn duration_hint(&self) -> Option<Duration> {
        Some(self.length)
    }
}

#[cfg(test)]
mod tests;
