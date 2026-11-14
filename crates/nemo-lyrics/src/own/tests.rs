//! Tests for own lyrics in the lyrics database.

use std::path::PathBuf;

use nemo_core::lyrics::{LyricWord, ProviderId};
use nemo_core::lyrics_plus::DraftLine;
use rusqlite::Connection;

use super::*;
use crate::cache::{EntryStatus, SystemClock};

const LRCLIB: ProviderId = ProviderId::new("LRCLIB");

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn dir(name: &str) -> Dir {
    use crate::cache::Clock as _;
    Dir(std::env::temp_dir().join(format!(
        "nemo-lyrics-own-{name}-{}-{}",
        std::process::id(),
        SystemClock.now_secs()
    )))
}

fn open(d: &Dir) -> (Arc<LyricsCache>, OwnLyricsStore) {
    let cache = Arc::new(LyricsCache::open(&d.0).expect("open"));
    (Arc::clone(&cache), OwnLyricsStore::new(cache))
}

fn word(start_ms: u64, end_ms: Option<u64>, text: &str) -> LyricWord {
    LyricWord {
        start_ms,
        end_ms,
        text: text.to_owned(),
    }
}

fn line(text: &str, start_ms: Option<u64>, words: Vec<LyricWord>) -> DraftLine {
    DraftLine {
        text: text.to_owned(),
        start_ms,
        words,
    }
}

fn draft(lines: Vec<DraftLine>) -> OwnLyricsDraft {
    OwnLyricsDraft { lines }
}

fn provider_doc() -> LyricsDoc {
    LyricsDoc {
        lines: vec![LyricLine {
            start_ms: Some(500),
            end_ms: Some(1500),
            text: "provider line".to_owned(),
            words: Vec::new(),
        }],
        granularity: Granularity::Line,
        provider: LRCLIB,
        matched: None,
        instrumental: false,
    }
}

#[test]
fn a_line_synced_draft_is_saved_as_line_and_read_back_as_your_lyrics() {
    let d = dir("line");
    let (_, own) = open(&d);
    let saved = own.save(
        "key",
        &draft(vec![
            line("first", Some(1000), vec![word(1000, Some(1400), "first")]),
            line("second", Some(2000), Vec::new()),
        ]),
    );
    assert_eq!(saved, Ok(Granularity::Line));
    let doc = own.get("key").expect("own lyrics");
    assert_eq!(doc.provider, OWN_LYRICS);
    assert_eq!(doc.granularity, Granularity::Line);
    assert_eq!(doc.lines.len(), 2);
    assert_eq!(doc.lines[1].start_ms, Some(2000));
    assert_eq!(doc.lines[1].end_ms, None, "no time is invented");
    assert_eq!(own.get("other"), None);
}

#[test]
fn a_draft_where_every_line_has_words_is_saved_as_word() {
    let d = dir("word");
    let (_, own) = open(&d);
    let saved = own.save(
        "key",
        &draft(vec![
            line(
                "a b",
                Some(0),
                vec![word(0, None, "a "), word(300, None, "b")],
            ),
            line("c", Some(900), vec![word(900, Some(1200), "c")]),
        ]),
    );
    assert_eq!(saved, Ok(Granularity::Word));
    assert_eq!(own.get("key").expect("doc").lines[0].words.len(), 2);
}

#[test]
fn save_refuses_a_line_with_no_start_and_names_it() {
    let d = dir("missing");
    let (_, own) = open(&d);
    let lines = vec![
        line("one", Some(0), Vec::new()),
        line("two", None, Vec::new()),
        line("three", None, Vec::new()),
    ];
    assert_eq!(
        own.save("key", &draft(lines)),
        Err(OwnLyricsError::MissingStart { line: 2 })
    );
    assert_eq!(own.get("key"), None, "nothing was stored");
    assert_eq!(
        own.save("key", &OwnLyricsDraft::default()),
        Err(OwnLyricsError::Empty)
    );
}

#[test]
fn save_refuses_words_out_of_time_order() {
    let d = dir("order");
    let (_, own) = open(&d);
    let back = vec![word(500, None, "late"), word(100, None, "early")];
    assert_eq!(
        own.save(
            "key",
            &draft(vec![
                line("x", Some(0), Vec::new()),
                line("y", Some(1), back)
            ])
        ),
        Err(OwnLyricsError::WordOrder { line: 2 })
    );
    let ends_early = vec![word(500, Some(400), "w")];
    assert_eq!(
        own.save("key", &draft(vec![line("w", Some(500), ends_early)])),
        Err(OwnLyricsError::WordOrder { line: 1 })
    );
}

#[test]
fn clear_and_refetch_keep_own_lyrics_and_delete_removes_them() {
    let d = dir("clear");
    let (cache, own) = open(&d);
    own.save("key", &draft(vec![line("mine", Some(0), Vec::new())]))
        .expect("save");
    cache
        .put_hit("key", LRCLIB, &provider_doc(), 90)
        .expect("hit");
    cache.clear().expect("clear");
    cache.remove_track("key").expect("refetch");
    assert!(own.get("key").is_some(), "clear keeps own lyrics");
    assert_eq!(
        own.get("key").map(|d| d.lines[0].text.clone()),
        Some("mine".to_owned())
    );
    own.delete("key").expect("delete");
    assert_eq!(own.get("key"), None);
    own.delete("key").expect("deleting nothing is fine");
}

#[test]
fn save_copy_keeps_the_provider_row_unchanged() {
    let d = dir("copy");
    let (cache, own) = open(&d);
    let original = provider_doc();
    cache.put_hit("key", LRCLIB, &original, 80).expect("hit");
    let mut edited = original.clone();
    edited.lines[0].text = "my fix".to_owned();
    own.save_copy("key", &edited).expect("copy");

    let copy = own.get("key").expect("own");
    assert_eq!(copy.provider, OWN_LYRICS);
    assert_eq!(copy.lines[0].text, "my fix");
    assert_eq!(copy.lines[0].end_ms, Some(1500));
    let cached = cached(&cache, "key");
    assert_eq!(
        cached,
        Some(EntryStatus::Hit {
            doc: original,
            score: 80
        })
    );
}

/// The provider row of `track_key`, read through the cache.
fn cached(cache: &LyricsCache, track_key: &str) -> Option<EntryStatus> {
    cache.get(track_key, LRCLIB).expect("get").map(|e| e.status)
}
#[test]
fn an_existing_database_gets_the_own_lyrics_table_and_keeps_its_rows() {
    let d = dir("migrate");
    std::fs::create_dir_all(&d.0).expect("dir");
    {
        // The layout after change 1 (pinned flag), before own lyrics, with one row.
        let old = Connection::open(d.0.join("lyrics.sqlite3")).expect("old");
        old.execute_batch(
            "CREATE TABLE lyrics_cache (
                 track_key TEXT NOT NULL, provider TEXT NOT NULL, status TEXT NOT NULL,
                 granularity TEXT, body BLOB, score INTEGER,
                 fetched_at INTEGER NOT NULL, expires_at INTEGER NOT NULL,
                 pinned INTEGER NOT NULL DEFAULT 0,
                 PRIMARY KEY (track_key, provider)
             );
             INSERT INTO lyrics_cache (track_key, provider, status, fetched_at, expires_at)
             VALUES ('k', 'LRCLIB', 'miss', 0, 9000000000);
             PRAGMA user_version = 1;",
        )
        .expect("old layout");
    }
    let (cache, own) = open(&d);
    assert_eq!(cached(&cache, "k"), Some(EntryStatus::Miss));
    own.save("k", &draft(vec![line("new", Some(10), Vec::new())]))
        .expect("save after migration");
    let version: i64 = cache
        .lock()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .expect("version");
    assert_eq!(version, 2);
    drop((cache, own));
    let (_, again) = open(&d);
    assert!(again.get("k").is_some(), "own lyrics survive reopening");
}
