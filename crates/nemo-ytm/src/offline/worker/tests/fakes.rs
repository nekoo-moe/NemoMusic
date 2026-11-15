//! Fakes for the worker tests: a clock that jumps, scripted waits, a queue and a downloader.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::super::*;

/// The fake clock: every sleep moves it on at once.
#[derive(Default)]
pub(super) struct FakePacer {
    pub(super) now: Mutex<Duration>,
    pub(super) sleeps: Mutex<Vec<Duration>>,
}

impl FakePacer {
    pub(super) fn now(&self) -> Duration {
        *self.now.lock().expect("lock")
    }
    pub(super) fn sleeps(&self) -> Vec<Duration> {
        self.sleeps.lock().expect("lock").clone()
    }
}

impl Pacer for FakePacer {
    fn sleep(&self, wait: Duration) {
        *self.now.lock().expect("lock") += wait;
        self.sleeps.lock().expect("lock").push(wait);
    }
}

/// Answers the scripted waits in turn, then 20 s.
pub(super) struct FakeJitter(pub(super) Mutex<VecDeque<Duration>>);

impl Jitter for FakeJitter {
    fn pause(&self) -> Duration {
        let next = self.0.lock().expect("lock").pop_front();
        next.unwrap_or(Duration::from_secs(20))
    }
}

#[derive(Default)]
pub(super) struct FakeQueue {
    pub(super) jobs: Mutex<VecDeque<Job>>,
    pub(super) saved: Mutex<Vec<VideoId>>,
    pub(super) failed: Mutex<Vec<(VideoId, FailReason)>>,
    pub(super) requeued: Mutex<Vec<(VideoId, u64)>>,
    pub(super) threads: Mutex<Vec<Option<String>>>,
}

impl FakeQueue {
    pub(super) fn with(ids: &[&str]) -> Arc<Self> {
        let queue = Self::default();
        for (i, id) in ids.iter().enumerate() {
            queue.jobs.lock().expect("lock").push_back(Job {
                video: VideoId::parse(id).expect("id"),
                file: format!("{i:032x}"),
            });
        }
        Arc::new(queue)
    }
    pub(super) fn failed(&self) -> Vec<(VideoId, FailReason)> {
        self.failed.lock().expect("lock").clone()
    }
}

impl Queue for FakeQueue {
    fn next(&self) -> Result<Option<Job>, OfflineError> {
        let name = std::thread::current().name().map(str::to_owned);
        self.threads.lock().expect("lock").push(name);
        Ok(self.jobs.lock().expect("lock").pop_front())
    }
    fn saved(&self, job: &Job, _: u64, _: &StreamInfo) -> Result<(), OfflineError> {
        self.saved.lock().expect("lock").push(job.video.clone());
        Ok(())
    }
    fn requeue(&self, job: &Job, done: u64) -> Result<(), OfflineError> {
        self.requeued
            .lock()
            .expect("lock")
            .push((job.video.clone(), done));
        Ok(())
    }
    fn failed(&self, job: &Job, reason: FailReason) -> Result<(), OfflineError> {
        self.failed
            .lock()
            .expect("lock")
            .push((job.video.clone(), reason));
        Ok(())
    }
}

/// Answers the scripted results in turn (then success), and records when each fetch ran.
pub(super) struct FakeDownload {
    pub(super) pacer: Arc<FakePacer>,
    pub(super) script: Mutex<VecDeque<Result<Fetched, FetchError>>>,
    pub(super) starts: Mutex<Vec<Duration>>,
    pub(super) active: AtomicUsize,
    pub(super) most_active: AtomicUsize,
}

impl FakeDownload {
    pub(super) fn new(
        pacer: &Arc<FakePacer>,
        script: Vec<Result<Fetched, FetchError>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            pacer: Arc::clone(pacer),
            script: Mutex::new(script.into()),
            starts: Mutex::default(),
            active: AtomicUsize::new(0),
            most_active: AtomicUsize::new(0),
        })
    }
}

impl Download for FakeDownload {
    fn fetch(
        &self,
        _: &Secret<String>,
        _: &Job,
        pause: &AtomicBool,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<Fetched, FetchError> {
        let now = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.most_active.fetch_max(now, Ordering::SeqCst);
        self.starts.lock().expect("lock").push(self.pacer.now());
        let answer = if pause.load(Ordering::SeqCst) {
            Ok(Fetched::Paused { done: 10 })
        } else {
            progress(100, Some(100));
            let next = self.script.lock().expect("lock").pop_front();
            next.unwrap_or(Ok(Fetched::Complete { bytes: 100 }))
        };
        self.active.fetch_sub(1, Ordering::SeqCst);
        answer
    }
}
