//! The worker over the real index and downloader, against the loopback test server.

use std::path::PathBuf;

use super::super::queue::CacheDownload;
use super::*;
use crate::offline::{Clock, OfflineCache};
use crate::test_server::{TestServer, range_response};

struct FixedClock;

impl Clock for FixedClock {
    fn now_secs(&self) -> i64 {
        7
    }
}

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn queue_row(cache: &OfflineCache, id: &str, position: i64) {
    cache
        .store
        .with_conn(|conn| {
            conn.execute(
                "INSERT INTO queue (video_id, position, reason, state, meta, added_at)
                 VALUES (?1, ?2, 'user', 'queued', '{}', 1)",
                rusqlite::params![id, position],
            )?;
            Ok(())
        })
        .expect("queue row");
}

#[test]
fn the_index_queue_saves_songs_and_a_manual_retry_runs_once() {
    let dir = Dir(std::env::temp_dir().join(format!("nemo-offline-worker-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&dir.0);
    let cache = Arc::new(OfflineCache::open(&dir.0, Arc::new(FixedClock)).expect("open"));
    queue_row(&cache, IDS[0], 1);
    queue_row(&cache, IDS[1], 2);
    let data: Vec<u8> = (0..5000_u32).map(|i| (i % 251) as u8).collect();
    let body = data.clone();
    let server = TestServer::start(move |request, _| range_response(&body, request));
    let rig = Rig::new();
    let resolver = FakeResolver::new(&server.url("/song"), vec![IDS[1]]);
    let download = Arc::new(CacheDownload::new(&dir.0, false).expect("download"));
    let mut worker = rig.worker(cache.clone(), resolver.clone(), download, Vec::new());
    drain(&mut worker, &rig.control);

    // The first song is saved with its bytes as served; the second failed.
    let (file, codec, saved_at): (String, String, i64) = cache
        .store
        .with_conn(|conn| {
            Ok(conn.query_row(
                "SELECT file, codec, saved_at FROM song WHERE video_id = ?1",
                [IDS[0]],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?)
        })
        .expect("song row");
    assert_eq!((codec.as_str(), saved_at), ("opus", 7));
    assert_eq!(
        std::fs::read(dir.0.join("blobs").join(file)).expect("blob"),
        data
    );
    let state: (String, String) = cache
        .store
        .with_conn(|conn| {
            Ok(conn.query_row(
                "SELECT state, fail_reason FROM queue WHERE video_id = ?1",
                [IDS[1]],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?)
        })
        .expect("queue row");
    assert_eq!(state, ("failed".to_owned(), "not_available".to_owned()));
    assert_eq!(resolver.calls(), 2);

    // A manual retry queues the song once; it fails again and stays failed.
    assert!(
        cache
            .retry_download(&VideoId::parse(IDS[1]).expect("id"))
            .expect("retry")
    );
    drain(&mut worker, &rig.control);
    assert_eq!(resolver.calls(), 3);
    assert!(!worker.run_one());
    assert_eq!(resolver.calls(), 3);
    assert!(
        !cache
            .retry_download(&VideoId::parse(IDS[0]).expect("id"))
            .expect("retry")
    );
}
