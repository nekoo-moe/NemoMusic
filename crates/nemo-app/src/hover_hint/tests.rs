//! The hint with a fake resolver behind a real cache: no network.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

use nemo_core::BoxFuture;
use nemo_core::audio::{Codec, Origin, StreamInfo};
use nemo_core::secret::Secret;
use nemo_core::stream::{ResolvedStream, StreamError, StreamResolver, Transport};
use tokio::runtime::Runtime;

use super::*;

/// Answers after `delay`; counts the resolves that began and the ones dropped before the end.
struct Slow {
    began: AtomicUsize,
    cancelled: AtomicUsize,
    delay: Duration,
}

struct Unfinished<'a>(&'a AtomicUsize, bool);

impl Drop for Unfinished<'_> {
    fn drop(&mut self) {
        if self.1 {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
}

impl StreamResolver for Slow {
    fn resolve<'a>(
        &'a self,
        _video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        Box::pin(async move {
            self.began.fetch_add(1, Ordering::SeqCst);
            let mut guard = Unfinished(&self.cancelled, true);
            tokio::time::sleep(self.delay).await;
            guard.1 = false;
            Ok(ResolvedStream {
                url: Secret::new("https://x.invalid/a".to_owned()),
                info: StreamInfo {
                    origin: Origin::YoutubeMusic,
                    codec: Codec::Opus,
                    bitrate_kbps: None,
                    format: None,
                },
                expires_at: Some(SystemTime::now() + Duration::from_hours(6)),
                transport: Transport::File,
                preview: false,
                audio_language: None,
                audio_tracks: Vec::new(),
            })
        })
    }
}

fn setup(delay: Duration) -> (Runtime, Arc<Slow>, Arc<CachedResolver>, Arc<HoverHint>) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .build()
        .expect("runtime");
    let slow = Arc::new(Slow {
        began: AtomicUsize::new(0),
        cancelled: AtomicUsize::new(0),
        delay,
    });
    let cache = Arc::new(CachedResolver::new(slow.clone(), rt.handle().clone()));
    let hint = HoverHint::new(Arc::clone(&cache));
    (rt, slow, cache, hint)
}

fn vid(text: &str) -> VideoId {
    VideoId::parse(text).expect("valid")
}

fn settle() {
    std::thread::sleep(Duration::from_millis(80));
}

#[test]
fn a_hinted_song_is_resolved_before_the_click_and_the_click_costs_nothing() {
    let (rt, slow, cache, hint) = setup(Duration::from_millis(40));
    hint.tell(Some(vid("aaaaaaaaaaa")));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(slow.began.load(Ordering::SeqCst), 1);
    let began = std::time::Instant::now();
    assert!(rt.block_on(cache.resolve(&vid("aaaaaaaaaaa"))).is_ok());
    assert!(
        began.elapsed() < Duration::from_millis(30),
        "served from memory"
    );
    assert_eq!(slow.began.load(Ordering::SeqCst), 1);
}

#[test]
fn leaving_cancels_the_running_resolve() {
    let (_rt, slow, _cache, hint) = setup(Duration::from_secs(5));
    hint.tell(Some(vid("aaaaaaaaaaa")));
    settle();
    assert_eq!(slow.began.load(Ordering::SeqCst), 1);
    hint.tell(None);
    settle();
    assert_eq!(
        slow.cancelled.load(Ordering::SeqCst),
        1,
        "the resolve was dropped"
    );
}

#[test]
fn a_new_song_replaces_the_hint_so_only_one_is_in_flight() {
    let (_rt, slow, _cache, hint) = setup(Duration::from_secs(5));
    hint.tell(Some(vid("aaaaaaaaaaa")));
    settle();
    hint.tell(Some(vid("bbbbbbbbbbb")));
    settle();
    assert_eq!(slow.began.load(Ordering::SeqCst), 2);
    assert_eq!(
        slow.cancelled.load(Ordering::SeqCst),
        1,
        "the first was cancelled"
    );
    hint.tell(Some(vid("bbbbbbbbbbb")));
    settle();
    assert_eq!(
        slow.began.load(Ordering::SeqCst),
        2,
        "the same song again starts nothing"
    );
}

#[test]
fn a_hint_that_a_play_has_taken_over_is_not_cancelled_when_the_pointer_leaves() {
    let (rt, slow, cache, hint) = setup(Duration::from_millis(300));
    let video = vid("aaaaaaaaaaa");
    hint.tell(Some(video.clone()));
    settle();
    let playing = {
        let (cache, video) = (Arc::clone(&cache), video.clone());
        rt.spawn(async move { cache.resolve(&video).await })
    };
    settle();
    hint.tell(None);
    assert!(rt.block_on(playing).expect("joined").is_ok());
    assert_eq!(
        slow.cancelled.load(Ordering::SeqCst),
        0,
        "the play kept the resolve"
    );
    assert_eq!(slow.began.load(Ordering::SeqCst), 1);
}

#[test]
fn the_hook_does_nothing_before_the_services_exist_and_tells_the_hint_after() {
    let (_rt, slow, _cache, hint) = setup(Duration::from_secs(5));
    let cell = HintSlot::default();
    let ui_hook = hook(Arc::clone(&cell));
    ui_hook(Some(vid("aaaaaaaaaaa")));
    settle();
    assert_eq!(slow.began.load(Ordering::SeqCst), 0);
    assert!(cell.set(hint).is_ok());
    ui_hook(Some(vid("aaaaaaaaaaa")));
    settle();
    assert_eq!(slow.began.load(Ordering::SeqCst), 1);
}
