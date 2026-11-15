//! Per-line side data of a cached hit (lyrics-plus): singer, background vocals and the
//! provider's romanization, stored as JSON in the `meta` column beside the lyrics body.

use nemo_core::lyrics::{LyricsDoc, ProviderId};
use nemo_core::lyrics_plus::LineMeta;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{CacheError, HIT_TTL, LyricsCache};

/// Private serde mirror of [`LineMeta`], which carries no serde.
#[derive(Serialize, Deserialize)]
struct MetaRec {
    singer: Option<u8>,
    background: bool,
    roman: Option<String>,
}

/// The column value for `meta`: `None` when no line has anything to say.
pub(super) fn encode_meta(meta: &[LineMeta]) -> Result<Option<Vec<u8>>, CacheError> {
    if meta.iter().all(|m| *m == LineMeta::default()) {
        return Ok(None);
    }
    let rec: Vec<MetaRec> = meta
        .iter()
        .map(|m| MetaRec {
            singer: m.singer,
            background: m.background,
            roman: m.roman.clone(),
        })
        .collect();
    serde_json::to_vec(&rec)
        .map(Some)
        .map_err(|e| CacheError::Body(e.to_string()))
}

fn decode_meta(body: &[u8]) -> Result<Vec<LineMeta>, CacheError> {
    let rec: Vec<MetaRec> =
        serde_json::from_slice(body).map_err(|e| CacheError::Body(e.to_string()))?;
    Ok(rec
        .into_iter()
        .map(|m| LineMeta {
            singer: m.singer,
            background: m.background,
            roman: m.roman,
        })
        .collect())
}

impl LyricsCache {
    /// Store lyrics for 30 days with their per-line side data, same index as `doc.lines`.
    ///
    /// # Errors
    /// [`CacheError`] on database or body errors.
    pub fn put_hit_with_meta(
        &self,
        track_key: &str,
        provider: ProviderId,
        doc: &LyricsDoc,
        score: u8,
        meta: &[LineMeta],
    ) -> Result<(), CacheError> {
        self.put(
            track_key,
            provider,
            "hit",
            Some((doc, score, meta)),
            HIT_TTL,
        )
    }

    /// The per-line side data of the fresh hit for a track and provider, same index as its
    /// `LyricsDoc.lines`. Empty when the source gave none, the row is older than this data, or
    /// there is no fresh hit.
    ///
    /// # Errors
    /// [`CacheError`] on database or body errors.
    pub fn line_meta(
        &self,
        track_key: &str,
        provider: ProviderId,
    ) -> Result<Vec<LineMeta>, CacheError> {
        let now = self.clock.now_secs();
        let body = self
            .lock()
            .query_row(
                "SELECT meta FROM lyrics_cache
                 WHERE track_key = ?1 AND provider = ?2 AND status = 'hit'
                   AND (expires_at > ?3 OR pinned = 1)",
                params![track_key, provider.as_str(), now],
                |r| r.get::<_, Option<Vec<u8>>>(0),
            )
            .optional()?
            .flatten();
        body.map_or_else(|| Ok(Vec::new()), |b| decode_meta(&b))
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::super::{DB_FILE, EntryStatus, encode_doc};
    use super::*;
    use crate::ttml::parse_ttml_detailed;

    const P: ProviderId = ProviderId::new("Unison");

    // Synthetic fixture in the Apple layout: a duet, a background line and `x-roman` spans.
    const DUET: &str = r#"<tt><head><metadata><ttm:agent xml:id="v1"/><ttm:agent xml:id="v2"/></metadata></head><body><div>
<p begin="1.0" end="3.0" ttm:agent="v1"><span begin="1.0" end="2.0">君の</span><span begin="2.0" end="3.0">名は</span><span ttm:role="x-roman">kimi no na wa</span><span ttm:role="x-bg"><span begin="2.5" end="3.0">(ah)</span><span ttm:role="x-roman">(a)</span></span></p>
<p begin="4.0" end="5.0" ttm:agent="v2"><span begin="4.0" end="5.0">さよなら</span><span ttm:role="x-roman">sayonara</span></p>
</div></body></tt>"#;

    struct Dir(std::path::PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            Self(std::env::temp_dir().join(format!(
                "nemo-lyrics-meta-{name}-{}-{nanos}",
                std::process::id()
            )))
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn ttml_line_meta_round_trips_through_the_cache() {
        let d = Dir::new("round-trip");
        let t = parse_ttml_detailed(DUET, P);
        let meta = t.line_meta();
        let expected = [
            (Some(0), false, Some("kimi no na wa")),
            (Some(0), true, Some("(a)")),
            (Some(1), false, Some("sayonara")),
        ];
        let got: Vec<_> = meta
            .iter()
            .map(|m| (m.singer, m.background, m.roman.as_deref()))
            .collect();
        assert_eq!(got, expected);

        let cache = LyricsCache::open(&d.0).expect("open");
        cache
            .put_hit_with_meta("k", P, &t.doc, 90, &meta)
            .expect("put");
        assert_eq!(cache.line_meta("k", P).expect("meta"), meta);
        drop(cache);
        let again = LyricsCache::open(&d.0).expect("reopen");
        assert_eq!(again.line_meta("k", P).expect("meta"), meta);
        let row = again.get("k", P).expect("get").map(|e| e.status);
        assert_eq!(
            row,
            Some(EntryStatus::Hit {
                doc: t.doc,
                score: 90
            })
        );
        assert!(
            again
                .line_meta("k", ProviderId::new("LRCLIB"))
                .expect("none")
                .is_empty()
        );
    }

    #[test]
    fn a_hit_without_meta_and_a_refresh_without_meta_give_an_empty_list() {
        let d = Dir::new("plain");
        let t = parse_ttml_detailed(DUET, P);
        let cache = LyricsCache::open(&d.0).expect("open");
        cache.put_hit("k", P, &t.doc, 90).expect("put");
        assert!(cache.line_meta("k", P).expect("meta").is_empty());
        cache
            .put_hit_with_meta("k", P, &t.doc, 90, &t.line_meta())
            .expect("put");
        cache.put_miss("k", P).expect("miss");
        assert!(cache.line_meta("k", P).expect("meta").is_empty());
    }

    #[test]
    fn rows_from_before_the_meta_column_return_an_empty_list() {
        let d = Dir::new("old-row");
        std::fs::create_dir_all(&d.0).expect("dir");
        let doc = parse_ttml_detailed(DUET, P).doc;
        {
            // The layout after change 2 (own lyrics), before line data, with one hit.
            let old = Connection::open(d.0.join(DB_FILE)).expect("old");
            old.execute_batch(
                "CREATE TABLE lyrics_cache (
                     track_key TEXT NOT NULL, provider TEXT NOT NULL, status TEXT NOT NULL,
                     granularity TEXT, body BLOB, score INTEGER,
                     fetched_at INTEGER NOT NULL, expires_at INTEGER NOT NULL,
                     pinned INTEGER NOT NULL DEFAULT 0,
                     PRIMARY KEY (track_key, provider)
                 );
                 PRAGMA user_version = 2;",
            )
            .expect("old layout");
            old.execute(
                "INSERT INTO lyrics_cache
                     (track_key, provider, status, granularity, body, score, fetched_at, expires_at)
                 VALUES ('k', 'Unison', 'hit', 'syllable', ?1, 90, 0, 9000000000)",
                params![encode_doc(&doc).expect("body")],
            )
            .expect("old row");
        }
        let cache = LyricsCache::open(&d.0).expect("open");
        assert!(cache.line_meta("k", P).expect("meta").is_empty());
        let row = cache.get("k", P).expect("get").map(|e| e.status);
        assert_eq!(row, Some(EntryStatus::Hit { doc, score: 90 }));
        let version: i64 = cache
            .lock()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("version");
        assert_eq!(version, 3);
    }
}
