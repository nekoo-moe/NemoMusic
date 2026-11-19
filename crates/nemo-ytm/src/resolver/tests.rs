//! Unit tests for the cached and fallback resolvers.

use std::sync::atomic::AtomicUsize;

use nemo_core::audio::{Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::stream::Transport;

use super::*;

/// Part of every fake stream URL and video id: it must never reach a log line.
const SECRET: &str = "SECRETURL";

/// Counts a resolve that is dropped before it ends.
struct Unfinished<'a>(&'a AtomicUsize, bool);

impl Drop for Unfinished<'_> {
    fn drop(&mut self) {
        if self.1 {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
}

struct Counting {
    calls: AtomicUsize,
    expires_in: Option<Duration>,
    fail: bool,
    /// A failing resolve fails as a request (an HTTP error), not as a refused video.
    request_error: bool,
    delay: Duration,
    failed: AtomicUsize,
    /// Resolves that were dropped before they answered.
    cancelled: AtomicUsize,
    /// Every request this resolver was given, in order.
    seen: Mutex<Vec<StreamRequest>>,
}

impl Counting {
    fn new(expires_in: Option<Duration>, fail: bool) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            expires_in,
            fail,
            request_error: false,
            delay: Duration::from_millis(20),
            failed: AtomicUsize::new(0),
            cancelled: AtomicUsize::new(0),
            seen: Mutex::default(),
        })
    }
    /// A resolver that answers after `delay` (a stalled network, a slow `yt-dlp` start).
    fn after(delay: Duration, fail: bool) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            expires_in: Some(Duration::from_hours(1)),
            fail,
            request_error: false,
            delay,
            failed: AtomicUsize::new(0),
            cancelled: AtomicUsize::new(0),
            seen: Mutex::default(),
        })
    }
    /// A resolver whose every answer is a failed request.
    fn failing_requests() -> Arc<Self> {
        let mut me = Arc::try_unwrap(Self::new(Some(Duration::from_hours(1)), true))
            .unwrap_or_else(|_| unreachable!("just built"));
        me.request_error = true;
        Arc::new(me)
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    fn seen(&self) -> Vec<StreamRequest> {
        self.seen.lock().expect("lock").clone()
    }
}

impl StreamResolver for Counting {
    fn resolve<'a>(&'a self, video: &'a VideoId) -> BoxFuture<'a, Answer> {
        self.resolve_with(video, &DEFAULT_REQUEST)
    }
    fn resolve_with<'a>(
        &'a self,
        _video: &'a VideoId,
        request: &'a StreamRequest,
    ) -> BoxFuture<'a, Answer> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.seen.lock().expect("lock").push(request.clone());
            let mut unfinished = Unfinished(&self.cancelled, true);
            tokio::time::sleep(self.delay).await;
            unfinished.1 = false;
            if self.fail {
                return Err(if self.request_error {
                    StreamError::Network("HTTP 429".to_owned())
                } else {
                    StreamError::NotAvailable
                });
            }
            Ok(ResolvedStream {
                url: Secret::new(format!("https://x.invalid/{SECRET}")),
                info: StreamInfo {
                    origin: Origin::YoutubeMusic,
                    codec: Codec::Aac,
                    bitrate_kbps: Some(128),
                    format: None,
                },
                expires_at: self.expires_in.map(|d| SystemTime::now() + d),
                transport: Transport::File,
                preview: false,
                audio_language: None,
                audio_tracks: Vec::new(),
            })
        })
    }
    fn stream_failed(&self, _video: &VideoId) {
        self.failed.fetch_add(1, Ordering::SeqCst);
    }
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .build()
        .expect("runtime")
}

fn vid(s: &str) -> VideoId {
    VideoId::parse(s).expect("valid")
}

#[test]
fn a_fresh_stream_is_reused_and_concurrent_asks_share_one_request() {
    let rt = rt();
    let inner = Counting::new(Some(Duration::from_hours(6)), false);
    let cache = CachedResolver::new(inner.clone(), rt.handle().clone());
    let v = vid("abcdefghijk");
    rt.block_on(async {
        let (a, b) = tokio::join!(cache.resolve(&v), cache.resolve(&v));
        assert!(a.is_ok() && b.is_ok());
        assert!(cache.resolve(&v).await.is_ok());
    });
    assert_eq!(inner.calls(), 1);
    assert_eq!(cache.len(), 1);
    cache.stream_failed(&v);
    assert!(cache.is_empty());
    assert_eq!(inner.failed.load(Ordering::SeqCst), 1);
    assert!(rt.block_on(cache.resolve(&v)).is_ok());
    assert_eq!(inner.calls(), 2);
}

#[test]
fn near_expiry_unknown_expiry_and_errors_are_not_kept() {
    let rt = rt();
    let v = vid("abcdefghijk");
    for inner in [
        Counting::new(Some(Duration::from_mins(5)), false),
        Counting::new(None, false),
        Counting::new(Some(Duration::from_secs(3600)), true),
    ] {
        let cache = CachedResolver::new(inner.clone(), rt.handle().clone());
        let _ = rt.block_on(cache.resolve(&v));
        let _ = rt.block_on(cache.resolve(&v));
        assert_eq!(inner.calls(), 2);
        assert!(cache.is_empty());
    }
}

#[test]
fn warm_fills_the_cache_in_the_background() {
    let rt = rt();
    let inner = Counting::new(Some(Duration::from_secs(3600)), false);
    let cache = CachedResolver::new(inner.clone(), rt.handle().clone());
    let v = vid("abcdefghijk");
    cache.warm(&v);
    cache.warm(&v);
    std::thread::sleep(Duration::from_millis(200));
    assert!(rt.block_on(cache.resolve(&v)).is_ok());
    assert_eq!(inner.calls(), 1);
}

#[test]
fn the_cache_stays_bounded() {
    let rt = rt();
    let inner = Counting::new(Some(Duration::from_secs(3600)), false);
    let cache = CachedResolver::new(inner, rt.handle().clone());
    for i in 0..(MAX_CACHED + 10) {
        let id = format!("abcdefg{i:04}");
        let _ = rt.block_on(cache.resolve(&vid(&id)));
    }
    assert!(cache.len() <= MAX_CACHED);
}

#[test]
fn repeated_failed_requests_rest_the_primary() {
    let rt = rt();
    let bad = Counting::failing_requests();
    let good = Counting::new(Some(Duration::from_hours(1)), false);
    let chain = FallbackResolver::new(bad.clone(), good.clone());
    for id in ["aaaaaaaaaaa", "bbbbbbbbbbb", "ccccccccccc"] {
        assert!(rt.block_on(chain.resolve(&vid(id))).is_ok());
    }
    assert_eq!((bad.calls(), good.calls()), (2, 3));
}

#[test]
fn fallback_takes_over_on_error_and_after_failed_opens() {
    let rt = rt();
    let v = vid("abcdefghijk");
    let bad = Counting::new(Some(Duration::from_secs(3600)), true);
    let good = Counting::new(Some(Duration::from_secs(3600)), false);
    let chain = FallbackResolver::new(bad.clone(), good.clone());
    assert!(rt.block_on(chain.resolve(&v)).is_ok());
    assert_eq!((bad.calls(), good.calls()), (1, 1));

    let primary = Counting::new(Some(Duration::from_secs(3600)), false);
    let fallback = Counting::new(Some(Duration::from_secs(3600)), false);
    let chain = FallbackResolver::new(primary.clone(), fallback.clone());
    assert!(rt.block_on(chain.resolve(&v)).is_ok());
    assert_eq!((primary.calls(), fallback.calls()), (1, 0));
    chain.stream_failed(&v);
    assert!(rt.block_on(chain.resolve(&v)).is_ok());
    assert_eq!((primary.calls(), fallback.calls()), (1, 1));
    // Failed opens of other videos switch the primary off.
    for id in ["bbbbbbbbbbb", "ccccccccccc"] {
        let other = vid(id);
        assert!(rt.block_on(chain.resolve(&other)).is_ok());
        chain.stream_failed(&other);
    }
    let calls = primary.calls();
    assert!(rt.block_on(chain.resolve(&vid("ddddddddddd"))).is_ok());
    assert_eq!(primary.calls(), calls);
}

fn dubbed() -> StreamRequest {
    StreamRequest {
        audio_language: Some("ja".to_owned()),
        prefer_premium: true,
        prefer_codec: Some(Codec::Aac),
    }
}

#[test]
fn the_request_reaches_the_inner_resolver_through_both_wrappers() {
    let rt = rt();
    let primary = Counting::new(Some(Duration::from_hours(1)), false);
    let fallback = Counting::new(Some(Duration::from_hours(1)), false);
    let chain: Arc<dyn StreamResolver> =
        Arc::new(FallbackResolver::new(primary.clone(), fallback.clone()));
    let cache = CachedResolver::new(chain, rt.handle().clone());
    let request = dubbed();
    assert!(
        rt.block_on(cache.resolve_with(&vid("abcdefghijk"), &request))
            .is_ok()
    );
    // The native path cannot serve a language or Premium, so yt-dlp gets the request.
    assert_eq!(primary.seen(), Vec::<StreamRequest>::new());
    assert_eq!(fallback.seen(), vec![request]);
}

#[test]
fn a_codec_hint_alone_still_asks_the_primary_with_the_request() {
    let rt = rt();
    let primary = Counting::new(Some(Duration::from_hours(1)), false);
    let fallback = Counting::new(Some(Duration::from_hours(1)), false);
    let chain = FallbackResolver::new(primary.clone(), fallback.clone());
    let request = StreamRequest {
        prefer_codec: Some(Codec::Aac),
        ..StreamRequest::default()
    };
    assert!(
        rt.block_on(chain.resolve_with(&vid("abcdefghijk"), &request))
            .is_ok()
    );
    assert_eq!(primary.seen(), vec![request]);
    assert!(fallback.seen().is_empty());
}

#[test]
fn a_failed_primary_hands_the_same_request_to_the_fallback() {
    let rt = rt();
    let bad = Counting::new(Some(Duration::from_hours(1)), true);
    let good = Counting::new(Some(Duration::from_hours(1)), false);
    let chain = FallbackResolver::new(bad.clone(), good.clone());
    let request = StreamRequest {
        prefer_codec: Some(Codec::Opus),
        ..StreamRequest::default()
    };
    assert!(
        rt.block_on(chain.resolve_with(&vid("abcdefghijk"), &request))
            .is_ok()
    );
    assert_eq!(bad.seen(), vec![request.clone()]);
    assert_eq!(good.seen(), vec![request]);
}

#[test]
fn two_requests_for_one_video_make_two_cache_entries() {
    let rt = rt();
    let inner = Counting::new(Some(Duration::from_hours(1)), false);
    let cache = CachedResolver::new(inner.clone(), rt.handle().clone());
    let v = vid("abcdefghijk");
    let request = dubbed();
    rt.block_on(async {
        assert!(cache.resolve(&v).await.is_ok());
        assert!(cache.resolve_with(&v, &request).await.is_ok());
        assert!(cache.resolve_with(&v, &request).await.is_ok());
    });
    assert_eq!(inner.calls(), 2);
    assert_eq!(cache.len(), 2);
    assert_eq!(inner.seen(), vec![StreamRequest::default(), request]);
    // A failed open drops the video for every request.
    cache.stream_failed(&v);
    assert!(cache.is_empty());
}

#[test]
fn the_default_request_hits_the_same_entry_as_resolve() {
    let rt = rt();
    let inner = Counting::new(Some(Duration::from_hours(1)), false);
    let cache = CachedResolver::new(inner.clone(), rt.handle().clone());
    let v = vid("abcdefghijk");
    cache.warm(&v);
    std::thread::sleep(Duration::from_millis(200));
    rt.block_on(async {
        assert!(cache.resolve(&v).await.is_ok());
        assert!(
            cache
                .resolve_with(&v, &StreamRequest::default())
                .await
                .is_ok()
        );
    });
    assert_eq!(inner.calls(), 1);
    assert_eq!(cache.len(), 1);
}

#[test]
fn no_url_and_no_id_reach_a_log_line_on_any_path() {
    // A current-thread runtime runs the spawned resolves on this thread, where the capture is.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime");
    let lines = crate::timing::capture::lines_of(|| {
        let bad = Counting::new(Some(Duration::from_hours(1)), true);
        let good = Counting::new(Some(Duration::from_hours(1)), false);
        let chain: Arc<dyn StreamResolver> = Arc::new(FallbackResolver::new(bad, good));
        let cache = CachedResolver::new(chain, rt.handle().clone());
        let v = vid("abcdefghijk");
        rt.block_on(async {
            assert!(
                cache.resolve(&v).await.is_ok(),
                "a miss: native, then yt-dlp"
            );
            assert!(cache.resolve(&v).await.is_ok(), "a hit");
            cache.warm(&vid("bbbbbbbbbbb"));
            tokio::time::sleep(Duration::from_millis(100)).await;
        });
        cache.stream_failed(&v);
    });
    assert!(
        lines.iter().any(|l| l.contains("resolve native")),
        "the stage lines are there: {lines:?}"
    );
    for line in &lines {
        for secret in [SECRET, "abcdefghijk", "bbbbbbbbbbb", "x.invalid"] {
            assert!(!line.contains(secret), "{line}");
        }
    }
}

/// Runs one resolve of `chain` and returns how long it took.
fn timed(
    rt: &tokio::runtime::Runtime,
    chain: &FallbackResolver,
    v: &VideoId,
) -> (Answer, Duration) {
    let began = Instant::now();
    let answer = rt.block_on(chain.resolve(v));
    (answer, began.elapsed())
}

const RACE: Duration = Duration::from_millis(50);

#[test]
fn a_slow_primary_gets_the_fallback_beside_it_and_the_first_good_answer_wins() {
    let rt = rt();
    let v = vid("abcdefghijk");
    let slow = Counting::after(Duration::from_millis(600), false);
    let quick = Counting::after(Duration::from_millis(20), false);
    let chain = FallbackResolver::new(slow.clone(), quick.clone()).with_race_after(RACE);
    let (answer, took) = timed(&rt, &chain, &v);
    assert!(answer.is_ok());
    assert!(took < Duration::from_millis(400), "{took:?}");
    assert_eq!((slow.calls(), quick.calls()), (1, 1));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        slow.cancelled.load(Ordering::SeqCst),
        1,
        "the loser is cancelled"
    );
    assert_eq!(quick.cancelled.load(Ordering::SeqCst), 0);
    chain.stream_failed(&v);
    assert_eq!(
        quick.failed.load(Ordering::SeqCst),
        1,
        "the stream came from the fallback"
    );
}

#[test]
fn a_slow_primary_that_answers_before_the_fallback_wins_and_the_fallback_is_cancelled() {
    let rt = rt();
    let v = vid("abcdefghijk");
    let slow = Counting::after(Duration::from_millis(150), false);
    let slower = Counting::after(Duration::from_secs(5), false);
    let chain = FallbackResolver::new(slow.clone(), slower.clone()).with_race_after(RACE);
    let (answer, took) = timed(&rt, &chain, &v);
    assert!(answer.is_ok());
    assert!(took < Duration::from_secs(1), "{took:?}");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(slower.cancelled.load(Ordering::SeqCst), 1);
    chain.stream_failed(&v);
    assert_eq!(
        slow.failed.load(Ordering::SeqCst),
        1,
        "the stream came from the primary"
    );
}

#[test]
fn a_slow_primary_that_fails_leaves_the_answer_to_the_running_fallback() {
    let rt = rt();
    let v = vid("abcdefghijk");
    let bad = Counting::after(Duration::from_millis(150), true);
    let good = Counting::after(Duration::from_millis(300), false);
    let chain = FallbackResolver::new(bad.clone(), good.clone()).with_race_after(RACE);
    let (answer, took) = timed(&rt, &chain, &v);
    assert!(answer.is_ok());
    assert!(took < Duration::from_millis(600), "{took:?}");
    assert_eq!(
        (bad.calls(), good.calls()),
        (1, 1),
        "the fallback is not started twice"
    );
}

#[test]
fn a_failing_fallback_waits_for_the_slow_primary() {
    let rt = rt();
    let v = vid("abcdefghijk");
    let slow = Counting::after(Duration::from_millis(300), false);
    let bad = Counting::after(Duration::from_millis(80), true);
    let chain = FallbackResolver::new(slow.clone(), bad).with_race_after(RACE);
    assert!(timed(&rt, &chain, &v).0.is_ok());
    chain.stream_failed(&v);
    assert_eq!(slow.failed.load(Ordering::SeqCst), 1);
}

#[test]
fn when_both_fail_the_error_is_the_fallbacks_and_a_fast_primary_starts_no_race() {
    let rt = rt();
    let v = vid("abcdefghijk");
    let chain = FallbackResolver::new(
        Counting::after(Duration::from_millis(150), true),
        Counting::after(Duration::from_millis(100), true),
    )
    .with_race_after(RACE);
    assert!(timed(&rt, &chain, &v).0.is_err());

    let fast = Counting::after(Duration::from_millis(10), false);
    let idle = Counting::after(Duration::from_millis(10), false);
    let chain = FallbackResolver::new(fast, idle.clone()).with_race_after(RACE);
    assert!(timed(&rt, &chain, &v).0.is_ok());
    assert_eq!(idle.calls(), 0);
}

mod persisted;
mod refusals;
