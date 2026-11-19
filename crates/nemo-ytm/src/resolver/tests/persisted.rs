//! The cache with a file behind it: a song played before a restart is reused after it.

use super::*;
use crate::resolver::{StreamStore, UrlCheck};

/// Says whether an address still answers, and counts the questions.
struct Check {
    alive: bool,
    asked: AtomicUsize,
}

impl UrlCheck for Check {
    fn alive<'a>(&'a self, _stream: &'a ResolvedStream) -> BoxFuture<'a, bool> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        let alive = self.alive;
        Box::pin(async move { alive })
    }
}

fn check(alive: bool) -> Arc<Check> {
    Arc::new(Check {
        alive,
        asked: AtomicUsize::new(0),
    })
}

fn path(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nemo-persisted-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir); // A leftover of an earlier run is fine to drop.
    dir.join("streams.json")
}

/// A run of the app: a new cache over the same file.
fn run(
    rt: &tokio::runtime::Runtime,
    inner: &Arc<Counting>,
    file: &std::path::Path,
    check: &Arc<Check>,
) -> CachedResolver {
    CachedResolver::new(inner.clone(), rt.handle().clone()).with_persisted(
        Arc::new(StreamStore::open(file.to_path_buf())),
        check.clone(),
    )
}

/// Wait until the background write has put the file in place.
fn wait_for_file(file: &std::path::Path) {
    for _ in 0..200 {
        if file.is_file() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_song_from_the_last_run_is_reused_after_one_check_and_no_resolve() {
    let rt = rt();
    let file = path("reuse");
    let v = vid("abcdefghijk");
    let first = Counting::new(Some(Duration::from_hours(6)), false);
    let yes = check(true);
    assert!(
        rt.block_on(run(&rt, &first, &file, &yes).resolve(&v))
            .is_ok()
    );
    assert_eq!(first.calls(), 1);
    assert_eq!(
        yes.asked.load(Ordering::SeqCst),
        0,
        "a fresh resolve needs no check"
    );
    wait_for_file(&file);

    let second = Counting::new(Some(Duration::from_hours(6)), false);
    let cache = run(&rt, &second, &file, &yes);
    for _ in 0..3 {
        assert!(rt.block_on(cache.resolve(&v)).is_ok());
    }
    assert_eq!(second.calls(), 0, "no resolve after the restart");
    assert_eq!(
        yes.asked.load(Ordering::SeqCst),
        1,
        "one check, then the memory cache"
    );
    let _ = std::fs::remove_dir_all(file.parent().expect("folder")); // Cleanup only.
}

#[test]
fn a_dead_address_is_resolved_again_and_replaced() {
    let rt = rt();
    let file = path("dead");
    let v = vid("abcdefghijk");
    let first = Counting::new(Some(Duration::from_hours(6)), false);
    assert!(
        rt.block_on(run(&rt, &first, &file, &check(true)).resolve(&v))
            .is_ok()
    );
    wait_for_file(&file);

    let second = Counting::new(Some(Duration::from_hours(6)), false);
    let no = check(false);
    assert!(
        rt.block_on(run(&rt, &second, &file, &no).resolve(&v))
            .is_ok()
    );
    assert_eq!(no.asked.load(Ordering::SeqCst), 1);
    assert_eq!(second.calls(), 1, "the refused address was resolved again");
    let _ = std::fs::remove_dir_all(file.parent().expect("folder")); // Cleanup only.
}

#[test]
fn a_stream_that_could_not_be_opened_is_forgotten_for_the_next_run() {
    let rt = rt();
    let file = path("failed");
    let v = vid("abcdefghijk");
    let first = Counting::new(Some(Duration::from_hours(6)), false);
    let cache = run(&rt, &first, &file, &check(true));
    assert!(rt.block_on(cache.resolve(&v)).is_ok());
    wait_for_file(&file);
    cache.stream_failed(&v);

    let second = Counting::new(Some(Duration::from_hours(6)), false);
    assert!(
        rt.block_on(run(&rt, &second, &file, &check(true)).resolve(&v))
            .is_ok()
    );
    assert_eq!(second.calls(), 1);
    let _ = std::fs::remove_dir_all(file.parent().expect("folder")); // Cleanup only.
}

#[test]
fn another_language_is_neither_kept_nor_served_from_the_file() {
    let rt = rt();
    let file = path("language");
    let v = vid("abcdefghijk");
    let german = StreamRequest {
        audio_language: Some("de".to_owned()),
        ..StreamRequest::default()
    };
    let first = Counting::new(Some(Duration::from_hours(6)), false);
    assert!(
        rt.block_on(run(&rt, &first, &file, &check(true)).resolve_with(&v, &german))
            .is_ok()
    );
    std::thread::sleep(Duration::from_millis(150));
    assert!(!file.exists(), "nothing was kept");
    let _ = std::fs::remove_dir_all(file.parent().expect("folder")); // Cleanup only.
}

#[test]
fn nothing_the_cache_logs_names_an_address_or_an_id() {
    let rt = rt();
    let file = path("log");
    let v = vid("abcdefghijk");
    let lines = crate::timing::capture::lines_of(|| {
        let first = Counting::new(Some(Duration::from_hours(6)), false);
        let _ = rt.block_on(run(&rt, &first, &file, &check(true)).resolve(&v));
        wait_for_file(&file);
        let second = Counting::new(Some(Duration::from_hours(6)), false);
        let _ = rt.block_on(run(&rt, &second, &file, &check(false)).resolve(&v));
    });
    for line in &lines {
        for secret in [SECRET, "abcdefghijk", "x.invalid"] {
            assert!(!line.contains(secret), "{line}");
        }
    }
    let _ = std::fs::remove_dir_all(file.parent().expect("folder")); // Cleanup only.
}
