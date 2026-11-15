//! The Last.fm scrobble queue, kept in the `scrobbles` table of the integrations database
//! (criterion 16).
//!
//! A finished play is appended with [`Store::push_scrobble`]. The sender reads the oldest rows
//! with [`Store::oldest_scrobbles`] (in order, at most [`BATCH`]) and deletes the ones Last.fm took
//! with [`Store::remove_scrobbles`]. The table holds at most [`CAP`] rows: when a push goes over,
//! the oldest rows are dropped and one warning is logged (the warning is not repeated until the
//! queue has been below the cap again). Rows carry no key, secret or session key, and nothing
//! here is ever logged except counts.

use std::sync::atomic::Ordering;

use rusqlite::params;

use crate::store::{Store, StoreError};

/// Most rows the queue keeps.
pub const CAP: usize = 5_000;
/// Most scrobbles one `track.scrobble` call carries.
pub const BATCH: usize = 50;

/// A finished play to send later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewScrobble {
    /// When the play started, Unix seconds.
    pub ts: i64,
    /// Track artist.
    pub artist: String,
    /// Track title.
    pub track: String,
    /// Album title, when known.
    pub album: Option<String>,
    /// Track length in seconds, when known.
    pub duration_s: Option<u32>,
}

/// A queued scrobble.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedScrobble {
    /// Row id. Rows are sent in id order.
    pub id: i64,
    /// The play.
    pub scrobble: NewScrobble,
    /// How many times sending this row failed.
    pub attempts: u32,
}

fn to_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

impl Store {
    /// Append a scrobble. Over [`CAP`] rows, the oldest are dropped.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] when the statement fails.
    pub fn push_scrobble(&self, s: &NewScrobble) -> Result<(), StoreError> {
        let dropped = self.with_conn(|c| {
            let tx = c.transaction()?;
            tx.execute(
                "INSERT INTO scrobbles (ts, artist, track, album, duration_s)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![s.ts, s.artist, s.track, s.album, s.duration_s],
            )?;
            let excess = tx.execute(
                "DELETE FROM scrobbles WHERE id IN
                   (SELECT id FROM scrobbles ORDER BY id DESC LIMIT -1 OFFSET ?1)",
                params![to_i64(CAP)],
            )?;
            tx.commit()?;
            Ok::<usize, rusqlite::Error>(excess)
        })?;
        if dropped > 0 {
            if !self.cap_warned.swap(true, Ordering::Relaxed) {
                tracing::warn!(
                    cap = CAP,
                    dropped,
                    "scrobble queue is full, the oldest scrobbles were dropped"
                );
            }
        } else {
            self.cap_warned.store(false, Ordering::Relaxed);
        }
        Ok(())
    }

    /// The oldest `limit` rows, oldest first.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] when the query fails.
    pub fn oldest_scrobbles(&self, limit: usize) -> Result<Vec<QueuedScrobble>, StoreError> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id, ts, artist, track, album, duration_s, attempts
                 FROM scrobbles ORDER BY id ASC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![to_i64(limit)], |r| {
                Ok(QueuedScrobble {
                    id: r.get(0)?,
                    scrobble: NewScrobble {
                        ts: r.get(1)?,
                        artist: r.get(2)?,
                        track: r.get(3)?,
                        album: r.get(4)?,
                        duration_s: r.get(5)?,
                    },
                    attempts: r.get(6)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::from)
        })
    }

    /// Delete the rows with these ids (Last.fm took them, or they are known bad).
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] when a statement fails.
    pub fn remove_scrobbles(&self, ids: &[i64]) -> Result<(), StoreError> {
        self.with_conn(|c| {
            let tx = c.transaction()?;
            {
                let mut stmt = tx.prepare("DELETE FROM scrobbles WHERE id = ?1")?;
                for id in ids {
                    stmt.execute(params![id])?;
                }
            }
            tx.commit().map_err(StoreError::from)
        })
    }

    /// Add one to `attempts` of these rows.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] when a statement fails.
    pub fn bump_scrobble_attempts(&self, ids: &[i64]) -> Result<(), StoreError> {
        self.with_conn(|c| {
            let tx = c.transaction()?;
            {
                let mut stmt =
                    tx.prepare("UPDATE scrobbles SET attempts = attempts + 1 WHERE id = ?1")?;
                for id in ids {
                    stmt.execute(params![id])?;
                }
            }
            tx.commit().map_err(StoreError::from)
        })
    }

    /// How many scrobbles wait.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] when the query fails.
    pub fn scrobble_count(&self) -> Result<usize, StoreError> {
        self.with_conn(|c| {
            c.query_row("SELECT COUNT(*) FROM scrobbles", [], |r| r.get::<_, i64>(0))
                .map(|n| usize::try_from(n).unwrap_or(0))
                .map_err(StoreError::from)
        })
    }

    /// Delete every queued scrobble (Last.fm disconnect).
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] when the statement fails.
    pub fn clear_scrobbles(&self) -> Result<(), StoreError> {
        self.cap_warned.store(false, Ordering::Relaxed);
        self.with_conn(|c| {
            c.execute("DELETE FROM scrobbles", [])
                .map(|_| ())
                .map_err(StoreError::from)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::TempDir;

    fn scrobble(n: i64) -> NewScrobble {
        NewScrobble {
            ts: 1_700_000_000 + n,
            artist: format!("Artist {n}"),
            track: format!("Track {n}"),
            album: (n % 2 == 0).then(|| format!("Album {n}")),
            duration_s: (n % 3 != 0).then_some(200),
        }
    }

    #[test]
    fn queue_survives_reopen_and_keeps_order() {
        let dir = TempDir::new("queue-reopen");
        let store = Store::open(&dir.0).expect("open");
        for n in 0..10 {
            store.push_scrobble(&scrobble(n)).expect("push");
        }
        drop(store);
        let store = Store::open(&dir.0).expect("reopen");
        assert_eq!(store.scrobble_count().expect("count"), 10);
        let rows = store.oldest_scrobbles(BATCH).expect("read");
        let got: Vec<_> = rows.iter().map(|r| r.scrobble.clone()).collect();
        let want: Vec<_> = (0..10).map(scrobble).collect();
        assert_eq!(got, want);
        assert!(rows.windows(2).all(|w| w[0].id < w[1].id));
    }

    #[test]
    fn oldest_respects_the_limit() {
        let store = Store::open_in_memory().expect("open");
        for n in 0..(i64::try_from(BATCH).expect("small") + 7) {
            store.push_scrobble(&scrobble(n)).expect("push");
        }
        let rows = store.oldest_scrobbles(BATCH).expect("read");
        assert_eq!(rows.len(), BATCH);
        assert_eq!(rows[0].scrobble, scrobble(0));
    }

    #[test]
    fn cap_drops_the_oldest_rows() {
        let store = Store::open_in_memory().expect("open");
        let total = i64::try_from(CAP).expect("small") + 3;
        for n in 0..total {
            store.push_scrobble(&scrobble(n)).expect("push");
        }
        assert_eq!(store.scrobble_count().expect("count"), CAP);
        let first = store.oldest_scrobbles(1).expect("read");
        assert_eq!(first[0].scrobble, scrobble(3));
    }

    #[test]
    fn cap_warning_rearms_after_the_queue_shrinks() {
        let store = Store::open_in_memory().expect("open");
        let total = i64::try_from(CAP).expect("small");
        for n in 0..=total {
            store.push_scrobble(&scrobble(n)).expect("push");
        }
        assert!(store.cap_warned.load(Ordering::Relaxed));
        let ids: Vec<i64> = store
            .oldest_scrobbles(2)
            .expect("read")
            .iter()
            .map(|r| r.id)
            .collect();
        store.remove_scrobbles(&ids).expect("remove");
        store.push_scrobble(&scrobble(1)).expect("push");
        assert!(!store.cap_warned.load(Ordering::Relaxed));
    }

    #[test]
    fn remove_and_clear_delete_rows() {
        let store = Store::open_in_memory().expect("open");
        for n in 0..5 {
            store.push_scrobble(&scrobble(n)).expect("push");
        }
        let rows = store.oldest_scrobbles(2).expect("read");
        let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
        store.remove_scrobbles(&ids).expect("remove");
        assert_eq!(store.scrobble_count().expect("count"), 3);
        assert_eq!(
            store.oldest_scrobbles(1).expect("read")[0].scrobble,
            scrobble(2)
        );
        store.clear_scrobbles().expect("clear");
        assert_eq!(store.scrobble_count().expect("count"), 0);
    }

    #[test]
    fn attempts_count_up() {
        let store = Store::open_in_memory().expect("open");
        store.push_scrobble(&scrobble(1)).expect("push");
        let id = store.oldest_scrobbles(1).expect("read")[0].id;
        store.bump_scrobble_attempts(&[id]).expect("bump");
        store.bump_scrobble_attempts(&[id]).expect("bump");
        assert_eq!(store.oldest_scrobbles(1).expect("read")[0].attempts, 2);
    }

    #[test]
    fn queue_and_switches_share_a_file_without_touching_each_other() {
        let store = Store::open_in_memory().expect("open");
        store.push_scrobble(&scrobble(1)).expect("push");
        store
            .set_switch(crate::store::META_LASTFM, true)
            .expect("on");
        store.clear_scrobbles().expect("clear");
        assert!(
            store
                .switch(crate::store::META_LASTFM, false)
                .expect("read")
        );
    }
}
