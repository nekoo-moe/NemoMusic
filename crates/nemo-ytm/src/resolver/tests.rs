//! Unit tests for the cached and fallback resolvers.

use std::sync::atomic::AtomicUsize;

use nemo_core::audio::{Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::stream::Transport;

use super::*;

struct Counting {
    calls: AtomicUsize,
    expires_in: Option<Duration>,
    fail: bool,
    delay: Duration,
    failed: AtomicUsize,
}

impl Counting {
    fn new(expires_in: Option<Duration>, fail: bool) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            expires_in,
            fail,
            delay: Duration::from_millis(20),
            failed: AtomicUsize::new(0),
        })
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl StreamResolver for Counting {
    fn resolve<'a>(&'a self, _video: &'a VideoId) -> BoxFuture<'a, Answer> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            if self.fail {
                return Err(StreamError::NotAvailable);
            }
            Ok(ResolvedStream {
                url: Secret::new("https://x.invalid/a".to_owned()),
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
fn repeated_primary_refusals_pause_the_primary() {
    let rt = rt();
    let bad = Counting::new(Some(Duration::from_hours(1)), true);
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
