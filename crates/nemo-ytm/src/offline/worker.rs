//! The download worker (offline-cache task 9, criteria 10, 11, 12 and 14).
//!
//! One `std` thread, `nemo-downloads`, takes one queued song at a time: it resolves the song
//! with [`StreamResolver::resolve_with`] and the [`StreamRequest`] it was given (`nemo-app` sets
//! `prefer_premium`), downloads it, and then waits a [`Jitter`] of [`GAP_MIN`] to [`GAP_MAX`]
//! before the next song. A failure marks the song failed with a plain reason, the worker moves
//! on, and the wait grows from [`FIRST_BACKOFF`], doubling up to [`MAX_BACKOFF`]; a `429`
//! waits at least as long as its `Retry-After`. After [`FAILURES_BEFORE_PAUSE`] failures in a
//! row the queue pauses. There are no parallel connections, and nothing is retried without a
//! wait: the user retries one song by hand, and it runs once.
//!
//! Nothing here logs a URL, a path or a video id.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use nemo_core::audio::StreamInfo;
use nemo_core::downloads::{DownloadEvent, FailReason, PauseReason, SaveState};
use nemo_core::secret::Secret;
use nemo_core::stream::{StreamError, StreamRequest, StreamResolver};
use nemo_core::track::VideoId;
use tokio::runtime::Handle;

use super::OfflineError;
use super::fetch::{FetchError, Fetched};

mod queue;

#[cfg(test)]
mod tests;

/// Shortest wait between two songs.
pub const GAP_MIN: Duration = Duration::from_secs(10);
/// Longest wait between two songs.
pub const GAP_MAX: Duration = Duration::from_secs(30);
/// Wait after the first failure; each failure in a row doubles it.
pub const FIRST_BACKOFF: Duration = Duration::from_mins(1);
/// Longest wait after failures.
pub const MAX_BACKOFF: Duration = Duration::from_mins(30);
/// Failures in a row after which the queue pauses.
pub const FAILURES_BEFORE_PAUSE: u32 = 5;
/// Name of the worker thread.
pub const THREAD_NAME: &str = "nemo-downloads";

/// The wait between two songs. Implementations answer [`GAP_MIN`] to [`GAP_MAX`]; the worker
/// clamps anything else into that range.
pub trait Jitter: Send + Sync {
    /// How long to wait before the next song.
    fn pause(&self) -> Duration;
}

/// A uniform random wait of [`GAP_MIN`] to [`GAP_MAX`], in whole milliseconds.
#[derive(Debug, Clone, Copy, Default)]
pub struct RandomJitter;

impl Jitter for RandomJitter {
    fn pause(&self) -> Duration {
        let mut bytes = [0_u8; 8];
        // Without random bytes the longest wait is the safe answer.
        if getrandom::fill(&mut bytes).is_err() {
            return GAP_MAX;
        }
        let span = u64::try_from(GAP_MAX.saturating_sub(GAP_MIN).as_millis()).unwrap_or(0);
        GAP_MIN + Duration::from_millis(u64::from_le_bytes(bytes) % (span + 1))
    }
}

/// How the worker waits, so tests can run on a fake clock.
pub(super) trait Pacer: Send + Sync {
    /// Wait `wait`, or less when the worker is told to stop.
    fn sleep(&self, wait: Duration);
}

#[derive(Debug, Default)]
struct Flags {
    stop: bool,
    /// The queue may have new work.
    wake: bool,
}

/// What the rest of the cache tells the worker: pause, resume, new work and stop.
#[derive(Debug, Default)]
pub(super) struct Control {
    flags: Mutex<Flags>,
    signal: Condvar,
    /// Read between download reads, so a pause stops within one read.
    pause: AtomicBool,
}

impl Control {
    /// A control that lets the worker look at the queue as soon as it starts.
    pub(super) fn new() -> Self {
        Self {
            flags: Mutex::new(Flags {
                stop: false,
                wake: true,
            }),
            ..Self::default()
        }
    }

    fn flags(&self) -> std::sync::MutexGuard<'_, Flags> {
        // A poisoned lock only means a thread panicked while setting a flag; the flags stay valid.
        self.flags.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Stop after the current read; the part file stays for a resume.
    pub(super) fn pause(&self) {
        self.pause.store(true, Ordering::SeqCst);
    }

    /// Go on with the queue.
    pub(super) fn resume(&self) {
        self.pause.store(false, Ordering::SeqCst);
        self.wake();
    }

    /// Whether the queue is paused.
    pub(super) fn is_paused(&self) -> bool {
        self.pause.load(Ordering::SeqCst)
    }

    /// The queue changed (a save or a retry): look at it again.
    pub(super) fn wake(&self) {
        self.flags().wake = true;
        self.signal.notify_all();
    }

    /// End the worker thread after the current read.
    pub(super) fn stop(&self) {
        self.pause.store(true, Ordering::SeqCst);
        self.flags().stop = true;
        self.signal.notify_all();
    }

    /// Block until there is work and the queue is not paused. `false` once stopped.
    fn wait_for_work(&self) -> bool {
        let mut flags = self.flags();
        while !flags.stop && (!flags.wake || self.is_paused()) {
            flags = self
                .signal
                .wait(flags)
                .unwrap_or_else(PoisonError::into_inner);
        }
        flags.wake = false;
        !flags.stop
    }
}

impl Pacer for Control {
    fn sleep(&self, wait: Duration) {
        let flags = self.flags();
        // Only `stop` ends the wait early: a save or a resume never shortens the gap.
        drop(
            self.signal
                .wait_timeout_while(flags, wait, |f| !f.stop)
                .unwrap_or_else(PoisonError::into_inner),
        );
    }
}

/// One queued song as the worker sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Job {
    /// The song.
    pub(super) video: VideoId,
    /// Opaque name of its part file in `tmp/` and, once complete, of its file in `blobs/`.
    pub(super) file: String,
}

/// The download queue the worker takes songs from.
pub(super) trait Queue: Send + Sync {
    /// The first queued song, now marked as saving; `None` when nothing is queued.
    fn next(&self) -> Result<Option<Job>, OfflineError>;
    /// The song is complete: list it as saved and drop it from the queue.
    fn saved(&self, job: &Job, bytes: u64, info: &StreamInfo) -> Result<(), OfflineError>;
    /// The download was paused with `done` bytes in its part file: queue it again.
    fn requeue(&self, job: &Job, done: u64) -> Result<(), OfflineError>;
    /// The song failed: keep it in the queue, failed, with `reason`.
    fn failed(&self, job: &Job, reason: FailReason) -> Result<(), OfflineError>;
}

/// Downloads one song's bytes into the cache.
pub(super) trait Download: Send + Sync {
    /// Fetch `url` into the part file of `job` and move it to `blobs/` once complete.
    fn fetch(
        &self,
        url: &Secret<String>,
        job: &Job,
        pause: &AtomicBool,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<Fetched, FetchError>;
}

/// Where the worker reports state changes.
pub(super) type EventSink = Arc<dyn Fn(DownloadEvent) + Send + Sync>;

/// Everything the worker uses. A struct, so the constructor stays small.
pub(super) struct WorkerDeps {
    pub(super) queue: Arc<dyn Queue>,
    pub(super) resolver: Arc<dyn StreamResolver>,
    /// The request every song is resolved with.
    pub(super) request: StreamRequest,
    /// Runs the resolver futures.
    pub(super) handle: Handle,
    pub(super) download: Arc<dyn Download>,
    pub(super) jitter: Arc<dyn Jitter>,
    pub(super) pacer: Arc<dyn Pacer>,
    pub(super) control: Arc<Control>,
    pub(super) events: EventSink,
}

/// How one song ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Saved,
    Paused,
    Failed {
        reason: FailReason,
        retry_after: Option<Duration>,
    },
}

/// The download worker: one song at a time, paced.
pub(super) struct Worker {
    deps: WorkerDeps,
    /// Failures in a row.
    failures: u32,
}

impl Worker {
    pub(super) fn new(deps: WorkerDeps) -> Self {
        Self { deps, failures: 0 }
    }

    /// Start the `nemo-downloads` thread. It ends after [`Control::stop`].
    pub(super) fn spawn(self) -> std::io::Result<JoinHandle<()>> {
        std::thread::Builder::new()
            .name(THREAD_NAME.to_owned())
            .spawn(move || self.run())
    }

    fn run(mut self) {
        while self.deps.control.wait_for_work() {
            while !self.deps.control.is_paused() && self.run_one() {}
        }
    }

    /// Download the next queued song and wait before the one after it. `false` when the queue
    /// is empty or cannot be read.
    pub(super) fn run_one(&mut self) -> bool {
        let job = match self.deps.queue.next() {
            Ok(Some(job)) => job,
            Ok(None) => return false,
            Err(e) => {
                tracing::warn!(error = %e, "the download queue could not be read");
                return false;
            }
        };
        let outcome = self.download(&job);
        self.record(&job, outcome);
        let wait = self.after(outcome);
        self.deps.pacer.sleep(wait);
        true
    }

    fn emit(&self, video: &VideoId, state: SaveState) {
        (self.deps.events)(DownloadEvent::State {
            video: video.clone(),
            state,
        });
    }

    fn download(&self, job: &Job) -> Outcome {
        self.emit(
            &job.video,
            SaveState::Saving {
                done: 0,
                total: None,
            },
        );
        let resolved = self.deps.handle.block_on(
            self.deps
                .resolver
                .resolve_with(&job.video, &self.deps.request),
        );
        let stream = match resolved {
            Ok(stream) => stream,
            Err(e) => return failed(resolve_reason(&e), None),
        };
        let mut progress = |done, total| self.emit(&job.video, SaveState::Saving { done, total });
        let fetched =
            self.deps
                .download
                .fetch(&stream.url, job, &self.deps.control.pause, &mut progress);
        match fetched {
            Ok(Fetched::Complete { bytes }) => {
                match self.deps.queue.saved(job, bytes, &stream.info) {
                    Ok(()) => Outcome::Saved,
                    Err(e) => {
                        tracing::warn!(error = %e, "a downloaded song could not be listed");
                        failed(FailReason::Damaged, None)
                    }
                }
            }
            Ok(Fetched::Paused { done }) => {
                if let Err(e) = self.deps.queue.requeue(job, done) {
                    tracing::warn!(error = %e, "a paused download could not be queued again");
                }
                Outcome::Paused
            }
            Err(e) => {
                let (reason, retry_after) = fetch_reason(e);
                failed(reason, retry_after)
            }
        }
    }

    fn record(&self, job: &Job, outcome: Outcome) {
        match outcome {
            Outcome::Saved => self.emit(&job.video, SaveState::Saved),
            Outcome::Paused => self.emit(&job.video, SaveState::Queued),
            Outcome::Failed { reason, .. } => {
                tracing::info!(reason = ?reason, "a download failed");
                if let Err(e) = self.deps.queue.failed(job, reason) {
                    tracing::warn!(error = %e, "a failed download could not be marked");
                }
                self.emit(&job.video, SaveState::Failed(reason));
            }
        }
    }

    /// Count the outcome and say how long to wait before the next song.
    fn after(&mut self, outcome: Outcome) -> Duration {
        let Outcome::Failed { retry_after, .. } = outcome else {
            self.failures = 0;
            return self.deps.jitter.pause().clamp(GAP_MIN, GAP_MAX);
        };
        self.failures += 1;
        let wait = backoff(self.failures).max(retry_after.unwrap_or(Duration::ZERO));
        if self.failures >= FAILURES_BEFORE_PAUSE {
            self.failures = 0;
            self.deps.control.pause();
            tracing::info!("downloads paused after repeated failures");
            (self.deps.events)(DownloadEvent::QueuePaused(PauseReason::RepeatedFailures));
        }
        wait
    }
}

const fn failed(reason: FailReason, retry_after: Option<Duration>) -> Outcome {
    Outcome::Failed {
        reason,
        retry_after,
    }
}

/// The wait after `failures` failures in a row: 60 s, doubling, at most 30 min.
pub(super) fn backoff(failures: u32) -> Duration {
    let doublings = failures.saturating_sub(1).min(16);
    FIRST_BACKOFF
        .saturating_mul(1 << doublings)
        .min(MAX_BACKOFF)
}

fn resolve_reason(error: &StreamError) -> FailReason {
    match error {
        StreamError::NotAvailable => FailReason::NotAvailable,
        StreamError::ResolverMissing | StreamError::Network(_) | StreamError::Parse(_) => {
            FailReason::Network
        }
    }
}

fn fetch_reason(error: FetchError) -> (FailReason, Option<Duration>) {
    match error {
        FetchError::RateLimited(after) => (FailReason::RateLimited, after),
        FetchError::Network | FetchError::Status(_) | FetchError::Length => {
            (FailReason::Network, None)
        }
        FetchError::Io => (FailReason::Damaged, None),
    }
}
