//! Removing songs, saved albums and playlists, and the whole cache (criteria 9 and 23).
//!
//! Files go first and their rows after, so a failed delete leaves the song listed (the user
//! can try again), never a file that no row names. Freed bytes are the sizes of the files
//! actually deleted.

use std::io::ErrorKind;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use super::{ART, BLOBS, Store, TMP};
use crate::offline::{OfflineError, is_opaque_name};

/// Songs a collection holds that nothing else holds: no other collection, no save of the song
/// by itself (`single`), and no smart download.
const ONLY_THIS_COLLECTION: &str = "
SELECT cs.video_id FROM collection_song cs
WHERE cs.collection_id = ?1
  AND NOT EXISTS (SELECT 1 FROM collection_song o
                  WHERE o.video_id = cs.video_id AND o.collection_id <> ?1)
  AND NOT EXISTS (SELECT 1 FROM song s
                  WHERE s.video_id = cs.video_id AND (s.single = 1 OR s.reason = 'smart'))
  AND NOT EXISTS (SELECT 1 FROM queue q
                  WHERE q.video_id = cs.video_id AND (q.single = 1 OR q.reason = 'smart'))";

/// A song or queue entry to remove, with the opaque names of its files.
struct Doomed {
    video_id: String,
    file: Option<String>,
    part: Option<String>,
}

impl Store {
    /// Remove one song: its file, its `.part` file, its rows and a cover no other song uses.
    pub(in crate::offline) fn remove_song(
        &self,
        root: &Path,
        video_id: &str,
    ) -> Result<u64, OfflineError> {
        self.with_conn(|conn| {
            let doomed = doomed(conn, &[video_id.to_owned()])?;
            let freed = remove(conn, root, &doomed)?;
            Ok(freed + remove_orphan_art(conn, root)?)
        })
    }

    /// Remove a saved album or playlist and the songs only it holds.
    pub(in crate::offline) fn remove_collection(
        &self,
        root: &Path,
        kind: &str,
        browse_id: &str,
    ) -> Result<u64, OfflineError> {
        self.with_conn(|conn| {
            let id: Option<i64> = conn
                .query_row(
                    "SELECT id FROM collection WHERE kind = ?1 AND browse_id = ?2",
                    params![kind, browse_id],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(id) = id else {
                return Ok(0);
            };
            let videos = video_ids(conn, ONLY_THIS_COLLECTION, params![id])?;
            let freed = remove(conn, root, &doomed(conn, &videos)?)?;
            // Its `collection_song` rows go with it (`ON DELETE CASCADE`).
            conn.execute("DELETE FROM collection WHERE id = ?1", [id])?;
            Ok(freed + remove_orphan_art(conn, root)?)
        })
    }

    /// Remove every saved and queued song, every collection and every cover. Play counts stay.
    pub(in crate::offline) fn remove_all(&self, root: &Path) -> Result<u64, OfflineError> {
        self.with_conn(|conn| {
            let videos = video_ids(
                conn,
                "SELECT video_id FROM song UNION SELECT video_id FROM queue",
                [],
            )?;
            let freed = remove(conn, root, &doomed(conn, &videos)?)?;
            conn.execute("DELETE FROM collection", [])?;
            Ok(freed + remove_orphan_art(conn, root)?)
        })
    }

    /// Close the index and delete the cache folder `root` with everything in it. Returns how
    /// many files were removed. Every later call fails with [`OfflineError::Closed`].
    pub(in crate::offline) fn wipe(&self, root: &Path) -> Result<u64, OfflineError> {
        self.close();
        let files = count_files(root)?;
        match std::fs::remove_dir_all(root) {
            Err(err) if err.kind() != ErrorKind::NotFound => Err(err.into()),
            _ => Ok(files),
        }
    }
}

fn video_ids(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<Vec<String>, OfflineError> {
    let mut stmt = conn.prepare(sql)?;
    let ids = stmt
        .query_map(params, |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

fn doomed(conn: &Connection, videos: &[String]) -> Result<Vec<Doomed>, OfflineError> {
    let mut song = conn.prepare("SELECT file FROM song WHERE video_id = ?1")?;
    let mut queue = conn.prepare("SELECT part_file FROM queue WHERE video_id = ?1")?;
    videos
        .iter()
        .map(|video_id| {
            let file = song.query_row([video_id], |row| row.get(0)).optional()?;
            let part: Option<Option<String>> =
                queue.query_row([video_id], |row| row.get(0)).optional()?;
            Ok(Doomed {
                video_id: video_id.clone(),
                file,
                part: part.flatten(),
            })
        })
        .collect()
}

/// Delete the files of `doomed`, then the rows of every song whose files are gone. Returns
/// the freed bytes, or [`OfflineError::Io`] when a file could not be deleted (its song stays).
fn remove(conn: &mut Connection, root: &Path, doomed: &[Doomed]) -> Result<u64, OfflineError> {
    let mut freed = 0;
    let mut gone = Vec::with_capacity(doomed.len());
    let mut failed = false;
    for entry in doomed {
        let song = entry
            .file
            .as_deref()
            .map_or(Ok(0), |name| delete_file(&root.join(BLOBS), name, ""));
        let part = entry
            .part
            .as_deref()
            .map_or(Ok(0), |name| delete_file(&root.join(TMP), name, ".part"));
        match (song, part) {
            (Ok(a), Ok(b)) => {
                freed += a + b;
                gone.push(entry.video_id.as_str());
            }
            _ => failed = true,
        }
    }
    let tx = conn.transaction()?;
    for video_id in gone {
        // Segments go with the song row (`ON DELETE CASCADE`).
        tx.execute("DELETE FROM song WHERE video_id = ?1", [video_id])?;
        tx.execute("DELETE FROM queue WHERE video_id = ?1", [video_id])?;
    }
    tx.commit()?;
    if failed {
        Err(OfflineError::Io)
    } else {
        Ok(freed)
    }
}

/// Delete the covers no song uses any more; returns the freed bytes.
fn remove_orphan_art(conn: &Connection, root: &Path) -> Result<u64, OfflineError> {
    let orphans: Vec<(i64, String)> = {
        let mut stmt = conn.prepare(
            "SELECT id, file FROM art
             WHERE id NOT IN (SELECT art_id FROM song WHERE art_id IS NOT NULL)",
        )?;
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    let mut freed = 0;
    for (id, file) in orphans {
        freed += delete_file(&root.join(ART), &file, "")?;
        conn.execute("DELETE FROM art WHERE id = ?1", [id])?;
    }
    Ok(freed)
}

/// Delete the cache file `<dir>/<name><suffix>`; returns its size, 0 when it was not there.
/// A name that is not opaque is never built into a path.
fn delete_file(dir: &Path, name: &str, suffix: &str) -> Result<u64, OfflineError> {
    if !is_opaque_name(name) {
        return Ok(0);
    }
    let path = dir.join(format!("{name}{suffix}"));
    let bytes = match std::fs::metadata(&path) {
        Ok(meta) => meta.len(),
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err.into()),
    };
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(bytes),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(0),
        Err(err) => Err(err.into()),
    }
}

/// How many files are under `dir`, in every subfolder.
fn count_files(dir: &Path) -> Result<u64, OfflineError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err.into()),
    };
    let mut count = 0;
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            count += count_files(&entry.path())?;
        } else {
            count += 1;
        }
    }
    Ok(count)
}
