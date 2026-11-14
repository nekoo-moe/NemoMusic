//! Tests for removing songs, collections and the whole cache, and for the sign-out wipe.

use super::*;

/// Save a synthetic song with a blob of `bytes` bytes; returns the blob's opaque name.
fn save(d: &Dir, video_id: &str, reason: &str, single: bool, bytes: u32) -> String {
    let file = opaque_name().expect("name");
    std::fs::write(d.0.join("blobs").join(&file), vec![7_u8; bytes as usize]).expect("blob");
    raw(d)
        .execute(
            "INSERT INTO song (video_id, file, bytes, codec, reason, meta, saved_at, single)
             VALUES (?1, ?2, ?3, 'opus', ?4, '{}', 1, ?5)",
            rusqlite::params![video_id, file, bytes, reason, single],
        )
        .expect("insert song");
    file
}

/// Save a collection listing `videos`; returns its id.
fn collection(d: &Dir, kind: &str, browse_id: &str, videos: &[&str]) -> i64 {
    let conn = raw(d);
    conn.execute(
        "INSERT INTO collection (kind, browse_id, title, saved_at) VALUES (?1, ?2, 't', 1)",
        [kind, browse_id],
    )
    .expect("insert collection");
    let id = conn.last_insert_rowid();
    for (position, video_id) in (0_i64..).zip(videos) {
        conn.execute(
            "INSERT INTO collection_song (collection_id, video_id, position) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, video_id, position],
        )
        .expect("insert collection song");
    }
    id
}

/// Queue a synthetic song with a `.part` file of `bytes` bytes; returns the part's name.
fn queue(d: &Dir, video_id: &str, bytes: usize) -> String {
    let part = opaque_name().expect("name");
    std::fs::write(
        d.0.join("tmp").join(format!("{part}.part")),
        vec![1_u8; bytes],
    )
    .expect("part");
    raw(d)
        .execute(
            "INSERT INTO queue (video_id, position, reason, state, part_file, meta, added_at)
             VALUES (?1, 0, 'user', 'saving', ?2, '{}', 1)",
            [video_id, &part],
        )
        .expect("insert queue");
    part
}

fn count(d: &Dir, sql: &str) -> i64 {
    raw(d).query_row(sql, [], |row| row.get(0)).expect("count")
}

fn files_in(d: &Dir, folder: &str) -> usize {
    std::fs::read_dir(d.0.join(folder)).expect("folder").count()
}

fn id(text: &str) -> VideoId {
    VideoId::parse(text).expect("id")
}

#[test]
fn removing_a_song_deletes_its_file_rows_and_unused_cover() {
    let d = dir("rm-song");
    let cache = OfflineCache::open(&d.0, clock(1)).expect("open");
    let kept = save(&d, "aaaaaaaaaa1", "user", true, 10);
    save(&d, "bbbbbbbbbb2", "user", true, 30);
    let art = opaque_name().expect("name");
    std::fs::write(d.0.join("art").join(&art), [0_u8; 5]).expect("art");
    let conn = raw(&d);
    conn.execute(
        "INSERT INTO art (id, source, file, bytes) VALUES (1, 'cover', ?1, 5)",
        [&art],
    )
    .expect("art row");
    conn.execute(
        "UPDATE song SET art_id = 1 WHERE video_id = 'bbbbbbbbbb2'",
        [],
    )
    .expect("link art");
    conn.execute(
        "INSERT INTO segment (video_id, start_ms, end_ms, category, segment_id)
         VALUES ('bbbbbbbbbb2', 0, 10, 'intro', 's1')",
        [],
    )
    .expect("segment");
    drop(conn);
    collection(&d, "album", "MPREb_synthetic", &["bbbbbbbbbb2"]);

    assert_eq!(cache.remove_song(&id("bbbbbbbbbb2")), Ok(35));
    assert_eq!(cache.saved_count(), Ok(1));
    assert_eq!(count(&d, "SELECT COUNT(*) FROM segment"), 0);
    assert_eq!(count(&d, "SELECT COUNT(*) FROM art"), 0);
    assert_eq!(count(&d, "SELECT COUNT(*) FROM collection"), 1);
    assert_eq!(files_in(&d, "art"), 0);
    let blobs: Vec<_> = std::fs::read_dir(d.0.join("blobs"))
        .expect("blobs")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(blobs, vec![std::ffi::OsString::from(kept)]);
    assert_eq!(cache.remove_song(&id("bbbbbbbbbb2")), Ok(0));
}

#[test]
fn removing_a_queued_song_deletes_its_part_file() {
    let d = dir("rm-queued");
    let cache = OfflineCache::open(&d.0, clock(1)).expect("open");
    queue(&d, "aaaaaaaaaa1", 12);
    assert_eq!(cache.remove_song(&id("aaaaaaaaaa1")), Ok(12));
    assert_eq!(count(&d, "SELECT COUNT(*) FROM queue"), 0);
    assert_eq!(files_in(&d, "tmp"), 0);
}

#[test]
fn removing_a_collection_keeps_songs_something_else_holds() {
    let d = dir("rm-collection");
    let cache = OfflineCache::open(&d.0, clock(1)).expect("open");
    save(&d, "only1111111", "user", false, 10);
    save(&d, "shared22222", "user", false, 20);
    save(&d, "single33333", "user", true, 40);
    save(&d, "smart444444", "smart", false, 80);
    queue(&d, "queued55555", 3);
    let album = [
        "only1111111",
        "shared22222",
        "single33333",
        "smart444444",
        "queued55555",
    ];
    collection(&d, "album", "MPREb_album", &album);
    collection(&d, "playlist", "VLPL_list", &["shared22222"]);

    let removed = cache.remove_collection(&CollectionRef::Album(nemo_core::catalog::BrowseId(
        "MPREb_album".to_owned(),
    )));
    assert_eq!(removed, Ok(13));
    let left: Vec<String> = {
        let conn = raw(&d);
        let mut stmt = conn
            .prepare("SELECT video_id FROM song ORDER BY video_id")
            .expect("prepare");
        stmt.query_map([], |row| row.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows")
    };
    assert_eq!(left, ["shared22222", "single33333", "smart444444"]);
    assert_eq!(files_in(&d, "blobs"), 3);
    assert_eq!(files_in(&d, "tmp"), 0);
    assert_eq!(count(&d, "SELECT COUNT(*) FROM queue"), 0);
    assert_eq!(count(&d, "SELECT COUNT(*) FROM collection"), 1);
    assert_eq!(count(&d, "SELECT COUNT(*) FROM collection_song"), 1);
}

#[test]
fn removing_everything_keeps_play_counts_only() {
    let d = dir("rm-all");
    let cache = OfflineCache::open(&d.0, clock(1)).expect("open");
    save(&d, "aaaaaaaaaa1", "user", true, 10);
    save(&d, "bbbbbbbbbb2", "smart", false, 20);
    queue(&d, "ccccccccccc", 4);
    collection(&d, "album", "MPREb_album", &["aaaaaaaaaa1"]);
    raw(&d)
        .execute(
            "INSERT INTO play_count (video_id, plays, last_played) VALUES ('bbbbbbbbbb2', 3, 1)",
            [],
        )
        .expect("plays");
    assert_eq!(cache.remove_all(), Ok(34));
    for folder in ["blobs", "tmp", "art"] {
        assert_eq!(files_in(&d, folder), 0, "{folder}");
    }
    for table in ["song", "queue", "collection", "collection_song", "art"] {
        assert_eq!(
            count(&d, &format!("SELECT COUNT(*) FROM {table}")),
            0,
            "{table}"
        );
    }
    assert_eq!(count(&d, "SELECT COUNT(*) FROM play_count"), 1);
}

#[test]
fn wipe_leaves_no_file_under_the_cache_folder() {
    let d = dir("wipe");
    let cache = OfflineCache::open(&d.0, clock(1)).expect("open");
    save(&d, "aaaaaaaaaa1", "user", true, 10);
    queue(&d, "bbbbbbbbbb2", 4);
    std::fs::write(d.0.join("art").join(opaque_name().expect("name")), b"a").expect("art");
    raw(&d)
        .execute(
            "INSERT INTO play_count (video_id, plays, last_played) VALUES ('aaaaaaaaaa1', 3, 1)",
            [],
        )
        .expect("plays");
    let files = cache.wipe().expect("wipe");
    // A blob, a part, a cover and at least `offline.db`.
    assert!(files >= 4, "{files}");
    assert!(!d.0.exists());
    assert_eq!(cache.saved_count(), Err(OfflineError::Closed));
    assert_eq!(
        cache.remove_song(&id("aaaaaaaaaa1")),
        Err(OfflineError::Closed)
    );
    assert_eq!(cache.wipe(), Ok(0));
}
