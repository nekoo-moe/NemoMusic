//! A refused video is not a failed request: the chain keeps asking the native path.

use super::*;

/// Refuses every video whose id starts with `x` (a bot check, an unplayable video).
struct Picky {
    calls: AtomicUsize,
}

impl StreamResolver for Picky {
    fn resolve<'a>(&'a self, video: &'a VideoId) -> BoxFuture<'a, Answer> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let refuse = video.as_str().starts_with('x');
        Box::pin(async move {
            Counting::new(Some(Duration::from_hours(1)), refuse)
                .resolve(video)
                .await
        })
    }
}

#[test]
fn refused_videos_do_not_stop_the_native_path_for_the_songs_after_them() {
    let rt = rt();
    let picky = Arc::new(Picky {
        calls: AtomicUsize::new(0),
    });
    let good = Counting::new(Some(Duration::from_hours(1)), false);
    let chain = FallbackResolver::new(picky.clone(), good.clone());
    for id in ["xaaaaaaaaaa", "xbbbbbbbbbb", "xcccccccccc", "xdddddddddd"] {
        assert!(
            rt.block_on(chain.resolve(&vid(id))).is_ok(),
            "yt-dlp serves it"
        );
    }
    assert_eq!((picky.calls.load(Ordering::SeqCst), good.calls()), (4, 4));
    // The next song is playable natively: the native path is still asked, and it answers.
    let ok = vid("okaaaaaaaaa");
    assert!(rt.block_on(chain.resolve(&ok)).is_ok());
    assert_eq!((picky.calls.load(Ordering::SeqCst), good.calls()), (5, 4));
    // That stream came from the native path (a failed open marks it so).
    chain.stream_failed(&ok);
    assert_eq!(good.failed.load(Ordering::SeqCst), 0);
}

#[test]
fn five_refusals_in_a_row_rest_the_native_path() {
    let rt = rt();
    let picky = Arc::new(Picky {
        calls: AtomicUsize::new(0),
    });
    let good = Counting::new(Some(Duration::from_hours(1)), false);
    let chain = FallbackResolver::new(picky.clone(), good.clone());
    for n in 0..PRIMARY_REFUSAL_LIMIT {
        let id = format!("x{n:010}");
        assert!(rt.block_on(chain.resolve(&vid(&id))).is_ok());
    }
    let asked = picky.calls.load(Ordering::SeqCst);
    assert_eq!(asked, PRIMARY_REFUSAL_LIMIT as usize);
    // Resting: the next song goes straight to the fallback, even one the native path could play.
    assert!(rt.block_on(chain.resolve(&vid("okaaaaaaaaa"))).is_ok());
    assert_eq!(picky.calls.load(Ordering::SeqCst), asked);
}
