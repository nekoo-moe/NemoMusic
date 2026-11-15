//! Download worker tests with a fake clock, a fake `Jitter`, a fake queue and downloader; the
//! run over the real index is in `index.rs`. No request leaves 127.0.0.1.

use std::sync::atomic::Ordering;

use nemo_core::BoxFuture;
use nemo_core::audio::{Codec, Origin};
use nemo_core::stream::{ResolvedStream, Transport};

use super::*;

mod fakes;
mod index;

use fakes::{FakeDownload, FakeJitter, FakePacer, FakeQueue};

fn stream(url: &str) -> ResolvedStream {
    ResolvedStream {
        url: Secret::new(url.to_owned()),
        info: StreamInfo {
            origin: Origin::YoutubeMusic,
            codec: Codec::Opus,
            bitrate_kbps: Some(128),
            format: None,
        },
        expires_at: None,
        transport: Transport::File,
        preview: false,
        audio_language: None,
        audio_tracks: Vec::new(),
    }
}

const AUTOMATIC: StreamRequest = StreamRequest {
    audio_language: None,
    prefer_premium: false,
    prefer_codec: None,
};

/// Records every request; fails with `NotAvailable` for the ids in `missing`.
struct FakeResolver {
    url: String,
    missing: Vec<&'static str>,
    seen: Mutex<Vec<(String, StreamRequest)>>,
}

impl FakeResolver {
    fn new(url: &str, missing: Vec<&'static str>) -> Arc<Self> {
        Arc::new(Self {
            url: url.to_owned(),
            missing,
            seen: Mutex::default(),
        })
    }
    fn calls(&self) -> usize {
        self.seen.lock().expect("lock").len()
    }
}

impl StreamResolver for FakeResolver {
    fn resolve<'a>(
        &'a self,
        video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        self.resolve_with(video, &AUTOMATIC)
    }
    fn resolve_with<'a>(
        &'a self,
        video: &'a VideoId,
        request: &'a StreamRequest,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        self.seen
            .lock()
            .expect("lock")
            .push((video.as_str().to_owned(), request.clone()));
        let answer = if self.missing.contains(&video.as_str()) {
            Err(StreamError::NotAvailable)
        } else {
            Ok(stream(&self.url))
        };
        Box::pin(async move { answer })
    }
}

fn premium() -> StreamRequest {
    StreamRequest {
        prefer_premium: true,
        ..StreamRequest::default()
    }
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime")
}

struct Rig {
    rt: tokio::runtime::Runtime,
    pacer: Arc<FakePacer>,
    control: Arc<Control>,
    events: Arc<Mutex<Vec<DownloadEvent>>>,
}

impl Rig {
    fn new() -> Self {
        Self {
            rt: rt(),
            pacer: Arc::default(),
            control: Arc::new(Control::new()),
            events: Arc::default(),
        }
    }

    fn worker(
        &self,
        queue: Arc<dyn Queue>,
        resolver: Arc<dyn StreamResolver>,
        download: Arc<dyn Download>,
        jitter: Vec<u64>,
    ) -> Worker {
        let events = Arc::clone(&self.events);
        Worker::new(WorkerDeps {
            queue,
            resolver,
            request: premium(),
            handle: self.rt.handle().clone(),
            download,
            jitter: Arc::new(FakeJitter(Mutex::new(
                jitter.into_iter().map(Duration::from_secs).collect(),
            ))),
            pacer: self.pacer.clone(),
            control: Arc::clone(&self.control),
            events: Arc::new(move |e| events.lock().expect("lock").push(e)),
        })
    }

    fn pauses(&self) -> usize {
        self.events
            .lock()
            .expect("lock")
            .iter()
            .filter(|e| **e == DownloadEvent::QueuePaused(PauseReason::RepeatedFailures))
            .count()
    }
}

/// Run songs the way the thread does until the queue is empty or paused.
fn drain(worker: &mut Worker, control: &Control) {
    while !control.is_paused() && worker.run_one() {}
}

const IDS: [&str; 6] = [
    "aaaaaaaaaa1",
    "aaaaaaaaaa2",
    "aaaaaaaaaa3",
    "aaaaaaaaaa4",
    "aaaaaaaaaa5",
    "aaaaaaaaaa6",
];

fn secs(list: &[u64]) -> Vec<Duration> {
    list.iter().copied().map(Duration::from_secs).collect()
}

#[test]
fn songs_run_one_at_a_time_with_10_to_30_s_between_them() {
    let rig = Rig::new();
    let queue = FakeQueue::with(&IDS[..4]);
    let resolver = FakeResolver::new("https://x.invalid/a", Vec::new());
    let download = FakeDownload::new(&rig.pacer, Vec::new());
    let mut worker = rig.worker(
        queue.clone(),
        resolver.clone(),
        download.clone(),
        vec![5, 45, 20],
    );
    drain(&mut worker, &rig.control);

    assert_eq!(download.most_active.load(Ordering::SeqCst), 1);
    let starts = download.starts.lock().expect("lock").clone();
    assert_eq!(starts, secs(&[0, 10, 40, 60]));
    for pair in starts.windows(2) {
        let gap = pair[1].saturating_sub(pair[0]);
        assert!((GAP_MIN..=GAP_MAX).contains(&gap), "gap {gap:?}");
    }
    assert_eq!(queue.saved.lock().expect("lock").len(), 4);
    // Every song is resolved once, with the request the worker was given.
    let seen = resolver.seen.lock().expect("lock").clone();
    assert_eq!(seen.len(), 4);
    assert!(seen.iter().all(|(_, request)| *request == premium()));
}

#[test]
fn the_random_jitter_stays_between_10_and_30_s() {
    for _ in 0..200 {
        let wait = RandomJitter.pause();
        assert!((GAP_MIN..=GAP_MAX).contains(&wait), "wait {wait:?}");
    }
}

#[test]
fn failures_back_off_doubling_and_five_in_a_row_pause_the_queue() {
    let rig = Rig::new();
    let queue = FakeQueue::with(&IDS);
    let resolver = FakeResolver::new("https://x.invalid/a", Vec::new());
    let download = FakeDownload::new(&rig.pacer, vec![Err(FetchError::Network); 6]);
    let mut worker = rig.worker(queue.clone(), resolver.clone(), download, Vec::new());
    drain(&mut worker, &rig.control);

    assert_eq!(rig.pacer.sleeps(), secs(&[60, 120, 240, 480, 960]));
    assert!(rig.control.is_paused());
    assert_eq!(rig.pauses(), 1);
    // Each failed song has a plain reason; the sixth was not started.
    let failed = queue.failed();
    assert_eq!(failed.len(), 5);
    assert!(failed.iter().all(|(_, r)| *r == FailReason::Network));
    assert_eq!(resolver.calls(), 5);
    assert_eq!(queue.jobs.lock().expect("lock").len(), 1);

    // After the user resumes, the count starts again from the first backoff.
    rig.control.resume();
    drain(&mut worker, &rig.control);
    assert_eq!(resolver.calls(), 6);
    assert_eq!(rig.pacer.sleeps().last(), Some(&FIRST_BACKOFF));
    assert!(!rig.control.is_paused());
}

#[test]
fn the_backoff_stops_growing_at_30_minutes() {
    assert_eq!(backoff(1), FIRST_BACKOFF);
    assert_eq!(backoff(2), Duration::from_mins(2));
    assert_eq!(backoff(5), Duration::from_mins(16));
    assert_eq!(backoff(6), MAX_BACKOFF);
    assert_eq!(backoff(u32::MAX), MAX_BACKOFF);
}

#[test]
fn a_429_waits_its_retry_after_and_never_less_than_the_backoff() {
    let rig = Rig::new();
    let queue = FakeQueue::with(&IDS[..3]);
    let resolver = FakeResolver::new("https://x.invalid/a", Vec::new());
    let download = FakeDownload::new(
        &rig.pacer,
        vec![
            Err(FetchError::RateLimited(Some(Duration::from_secs(600)))),
            Err(FetchError::RateLimited(Some(Duration::from_secs(5)))),
        ],
    );
    let mut worker = rig.worker(queue.clone(), resolver, download, vec![15]);
    drain(&mut worker, &rig.control);

    assert_eq!(rig.pacer.sleeps(), secs(&[600, 120, 15]));
    assert!(
        queue
            .failed()
            .iter()
            .all(|(_, r)| *r == FailReason::RateLimited)
    );
}

#[test]
fn a_failed_song_is_marked_and_a_success_resets_the_count() {
    let rig = Rig::new();
    let queue = FakeQueue::with(&IDS[..3]);
    let resolver = FakeResolver::new("https://x.invalid/a", vec![IDS[0]]);
    let download = FakeDownload::new(&rig.pacer, vec![Ok(Fetched::Complete { bytes: 1 })]);
    let mut worker = rig.worker(queue.clone(), resolver, download, vec![12, 12]);
    drain(&mut worker, &rig.control);

    assert_eq!(
        queue.failed(),
        vec![(
            VideoId::parse(IDS[0]).expect("id"),
            FailReason::NotAvailable
        )]
    );
    assert_eq!(queue.saved.lock().expect("lock").len(), 2);
    assert_eq!(rig.pacer.sleeps(), secs(&[60, 12, 12]));
    assert!(
        rig.events
            .lock()
            .expect("lock")
            .contains(&DownloadEvent::State {
                video: VideoId::parse(IDS[0]).expect("id"),
                state: SaveState::Failed(FailReason::NotAvailable),
            })
    );
}

#[test]
fn a_pause_keeps_the_part_and_queues_the_song_again() {
    let rig = Rig::new();
    let queue = FakeQueue::with(&IDS[..2]);
    let resolver = FakeResolver::new("https://x.invalid/a", Vec::new());
    let download = FakeDownload::new(&rig.pacer, Vec::new());
    let mut worker = rig.worker(queue.clone(), resolver, download, Vec::new());
    rig.control.pause();
    assert!(worker.run_one());
    assert_eq!(
        *queue.requeued.lock().expect("lock"),
        vec![(VideoId::parse(IDS[0]).expect("id"), 10)]
    );
    assert!(queue.failed().is_empty());
    assert_eq!(rig.pauses(), 0);
}

#[test]
fn the_thread_is_named_waits_for_work_and_stops() {
    let rt = rt();
    let control = Arc::new(Control::new());
    let queue = FakeQueue::with(&[]);
    let events: EventSink = Arc::new(|_| {});
    let worker = Worker::new(WorkerDeps {
        queue: queue.clone(),
        resolver: FakeResolver::new("https://x.invalid/a", Vec::new()),
        request: StreamRequest::default(),
        handle: rt.handle().clone(),
        download: FakeDownload::new(&Arc::default(), Vec::new()),
        jitter: Arc::new(RandomJitter),
        pacer: control.clone(),
        control: Arc::clone(&control),
        events,
    });
    let thread = worker.spawn().expect("spawn");
    control.wake();
    std::thread::sleep(Duration::from_millis(100));
    control.stop();
    thread.join().expect("join");
    let threads = queue.threads.lock().expect("lock").clone();
    assert!(!threads.is_empty());
    assert!(threads.iter().all(|t| t.as_deref() == Some(THREAD_NAME)));
}
