//! The player store with a fake source and a fake solver.

use std::sync::atomic::Ordering;
use std::time::Duration;

use super::fakes::{FakeSolver, FakeSource};
use crate::cipher::PlayerStore;

use std::path::PathBuf;
use std::sync::Arc;

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .build()
        .expect("runtime")
}

fn file(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nemo-player-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir); // A leftover of an earlier run is fine to drop.
    dir.join("player.json")
}

fn store(
    rt: &tokio::runtime::Runtime,
    source: &Arc<FakeSource>,
    solver: &Arc<FakeSolver>,
    path: Option<PathBuf>,
) -> Arc<PlayerStore> {
    PlayerStore::new(source.clone(), solver.clone(), rt.handle().clone(), path)
}

#[test]
fn concurrent_gets_prepare_the_player_once() {
    let rt = rt();
    let source = FakeSource::new("aaaaaaaa", Duration::from_millis(40));
    let solver = Arc::new(FakeSolver::default());
    let store = store(&rt, &source, &solver, None);
    let (a, b) = rt.block_on(async { tokio::join!(store.get(), store.get()) });
    let (a, b) = (a.expect("a"), b.expect("b"));
    assert_eq!((a.id.as_str(), a.sts), ("aaaaaaaa", 20522));
    assert_eq!(a, b);
    assert_eq!(*a.preprocessed, "pre-aaaaaaaa");
    assert_eq!(source.scripts.load(Ordering::SeqCst), 1);
    assert_eq!(solver.wholes.load(Ordering::SeqCst), 1);
    assert!(rt.block_on(store.get()).is_ok());
    assert_eq!(
        source.ids.load(Ordering::SeqCst),
        1,
        "a fresh player is not asked about again"
    );
}

#[test]
fn a_later_run_uses_the_file_and_asks_for_nothing() {
    let rt = rt();
    let path = file("reuse");
    let (source, solver) = (
        FakeSource::new("aaaaaaaa", Duration::ZERO),
        Arc::new(FakeSolver::default()),
    );
    assert!(
        rt.block_on(store(&rt, &source, &solver, Some(path.clone())).get())
            .is_ok()
    );

    let (source, solver) = (
        FakeSource::new("bbbbbbbb", Duration::ZERO),
        Arc::new(FakeSolver::default()),
    );
    let second = store(&rt, &source, &solver, Some(path.clone()));
    assert!(second.has_file());
    let info = rt.block_on(second.get()).expect("from the file");
    assert_eq!(info.id, "aaaaaaaa");
    assert_eq!(
        source.ids.load(Ordering::SeqCst) + source.scripts.load(Ordering::SeqCst),
        0
    );
    assert_eq!(solver.wholes.load(Ordering::SeqCst), 0);
    let _ = std::fs::remove_dir_all(path.parent().expect("folder")); // Cleanup only.
}

#[cfg(unix)]
#[test]
fn the_cache_file_is_private_and_wipe_deletes_it() {
    use std::os::unix::fs::PermissionsExt;

    let rt = rt();
    let path = file("mode");
    let (source, solver) = (
        FakeSource::new("aaaaaaaa", Duration::ZERO),
        Arc::new(FakeSolver::default()),
    );
    let store = store(&rt, &source, &solver, Some(path.clone()));
    assert!(rt.block_on(store.get()).is_ok());
    assert_eq!(
        std::fs::metadata(&path).expect("file").permissions().mode() & 0o777,
        0o600
    );
    store.wipe();
    assert!(!path.exists());
    let _ = std::fs::remove_dir_all(path.parent().expect("folder")); // Cleanup only.
}

#[test]
fn an_old_file_is_confirmed_with_one_id_request_and_a_new_id_downloads_again() {
    let rt = rt();
    let path = file("old");
    let (source, solver) = (
        FakeSource::new("aaaaaaaa", Duration::ZERO),
        Arc::new(FakeSolver::default()),
    );
    assert!(
        rt.block_on(store(&rt, &source, &solver, Some(path.clone())).get())
            .is_ok()
    );
    // Make the file seven hours old.
    let text = std::fs::read_to_string(&path).expect("file");
    let old = text.replacen("\"checked\":", "\"checked\":1,\"was\":", 1);
    std::fs::write(&path, old).expect("write");

    let (same, solver2) = (
        FakeSource::new("aaaaaaaa", Duration::ZERO),
        Arc::new(FakeSolver::default()),
    );
    let info = rt
        .block_on(store(&rt, &same, &solver2, Some(path.clone())).get())
        .expect("same");
    assert_eq!(info.id, "aaaaaaaa");
    assert_eq!(
        (
            same.ids.load(Ordering::SeqCst),
            same.scripts.load(Ordering::SeqCst)
        ),
        (1, 0)
    );

    let text = std::fs::read_to_string(&path).expect("file");
    std::fs::write(
        &path,
        text.replacen("\"checked\":", "\"checked\":1,\"was\":", 1),
    )
    .expect("write");
    let (newer, solver3) = (
        FakeSource::new("bbbbbbbb", Duration::ZERO),
        Arc::new(FakeSolver::default()),
    );
    let info = rt
        .block_on(store(&rt, &newer, &solver3, Some(path.clone())).get())
        .expect("new");
    assert_eq!(info.id, "bbbbbbbb");
    assert_eq!(newer.scripts.load(Ordering::SeqCst), 1);
    let _ = std::fs::remove_dir_all(path.parent().expect("folder")); // Cleanup only.
}

#[test]
fn a_player_that_cannot_be_confirmed_is_still_used() {
    let rt = rt();
    let path = file("offline");
    let (source, solver) = (
        FakeSource::new("aaaaaaaa", Duration::ZERO),
        Arc::new(FakeSolver::default()),
    );
    assert!(
        rt.block_on(store(&rt, &source, &solver, Some(path.clone())).get())
            .is_ok()
    );
    let text = std::fs::read_to_string(&path).expect("file");
    std::fs::write(
        &path,
        text.replacen("\"checked\":", "\"checked\":1,\"was\":", 1),
    )
    .expect("write");
    let (down, solver2) = (
        FakeSource::new("zzzzzzzz", Duration::ZERO),
        Arc::new(FakeSolver::default()),
    );
    down.fail_ids.store(true, Ordering::SeqCst);
    let info = rt
        .block_on(store(&rt, &down, &solver2, Some(path.clone())).get())
        .expect("kept");
    assert_eq!(info.id, "aaaaaaaa");
    let _ = std::fs::remove_dir_all(path.parent().expect("folder")); // Cleanup only.
}

#[test]
fn a_refresh_drops_the_player_so_the_next_get_reads_the_id_again_once_in_ten_minutes() {
    let rt = rt();
    let path = file("refresh");
    let (source, solver) = (
        FakeSource::new("aaaaaaaa", Duration::ZERO),
        Arc::new(FakeSolver::default()),
    );
    let store = store(&rt, &source, &solver, Some(path.clone()));
    assert!(rt.block_on(store.get()).is_ok());
    source.set_id("bbbbbbbb");
    store.refresh();
    assert!(!path.exists(), "the file of the stale table is gone");
    let info = rt.block_on(store.get()).expect("new player");
    assert_eq!(info.id, "bbbbbbbb");
    assert_eq!(source.ids.load(Ordering::SeqCst), 2);
    source.set_id("cccccccc");
    store.refresh();
    assert_eq!(
        rt.block_on(store.get()).expect("kept").id,
        "bbbbbbbb",
        "a second refresh within ten minutes changes nothing"
    );
    let _ = std::fs::remove_dir_all(path.parent().expect("folder")); // Cleanup only.
}

#[test]
fn a_caller_that_gives_up_does_not_stop_the_preparation() {
    let rt = rt();
    let source = FakeSource::new("aaaaaaaa", Duration::from_millis(100));
    let solver = Arc::new(FakeSolver::default());
    let store = store(&rt, &source, &solver, None);
    let gave_up =
        rt.block_on(async { tokio::time::timeout(Duration::from_millis(20), store.get()).await });
    assert!(gave_up.is_err(), "the caller was dropped (a lost race)");
    std::thread::sleep(Duration::from_millis(500));
    assert!(rt.block_on(store.get()).is_ok());
    assert_eq!(
        source.scripts.load(Ordering::SeqCst),
        1,
        "the work was done once, for the next caller"
    );
    assert_eq!(solver.wholes.load(Ordering::SeqCst), 1);
}
