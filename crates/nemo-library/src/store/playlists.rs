//! Local playlists: each change is saved at once, in one transaction (criterion 18).
//!
//! An entry is a local file (path and CUE index) or a streamed song saved by playlist-import
//! or a stats recap, kept as JSON. Entries are numbered from 0 with no gaps.

use std::path::PathBuf;
use std::time::Duration;

use nemo_core::audio::Origin;
use nemo_core::library::{LibraryError, LocalPlaylist, LocalPlaylistId, PlaylistEntry};
use nemo_core::track::{Artwork, Thumbnail, Track, TrackId, VideoId};
use rusqlite::{Transaction, params};
use serde_json::{Value, json};

use super::{Store, db, now_secs, path_text};

/// An entry to add to a playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewEntry {
    /// A local file; `cue_index` 0 is the whole file, `n` track `n` of its CUE sheet.
    File {
        /// The audio file.
        path: PathBuf,
        /// The CUE track, or 0.
        cue_index: u32,
    },
    /// A streamed song.
    Stream(Track),
}

/// One stored entry: `(path, cue_index, stream)`.
type Stored = (Option<String>, u32, Option<String>);

fn origin_text(origin: Origin) -> &'static str {
    match origin {
        Origin::Local => "local",
        Origin::YoutubeMusic => "youtube",
        Origin::SoundCloud => "soundcloud",
    }
}

fn stream_json(track: &Track) -> String {
    let thumbnails: Vec<Value> = match &track.artwork {
        Artwork::Remote(all) => all
            .iter()
            .map(|t| json!([t.url, t.width, t.height]))
            .collect(),
        Artwork::None | Artwork::LocalFile(_) => Vec::new(),
    };
    json!({
        "id": track.id.0,
        "title": track.title,
        "artists": track.artists,
        "album": track.album,
        "duration_ms": track.duration.and_then(|d| u64::try_from(d.as_millis()).ok()),
        "origin": origin_text(track.origin),
        "video_id": track.video_id.as_ref().map(VideoId::as_str),
        "thumbnails": thumbnails,
    })
    .to_string()
}

fn text_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn thumbnail(t: &Value) -> Option<Thumbnail> {
    let size = |i: usize| {
        t.get(i)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    };
    Some(Thumbnail {
        url: t.get(0)?.as_str()?.to_owned(),
        width: size(1)?,
        height: size(2)?,
    })
}

/// A stored streamed song back as a track; `None` when the JSON does not parse.
fn stream_track(text: &str) -> Option<Track> {
    let v: Value = serde_json::from_str(text).ok()?;
    let origin = match v.get("origin")?.as_str()? {
        "soundcloud" => Origin::SoundCloud,
        "local" => Origin::Local,
        _ => Origin::YoutubeMusic,
    };
    let thumbs: Vec<Thumbnail> = v
        .get("thumbnails")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(thumbnail)
        .collect();
    Some(Track {
        id: TrackId(text_of(&v, "id")?),
        title: text_of(&v, "title")?,
        artists: v
            .get("artists")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|a| a.as_str().map(str::to_owned))
            .collect(),
        album: text_of(&v, "album"),
        duration: v
            .get("duration_ms")
            .and_then(Value::as_u64)
            .map(Duration::from_millis),
        origin,
        video_id: text_of(&v, "video_id").and_then(|id| VideoId::parse(&id).ok()),
        local_path: None,
        artwork: if thumbs.is_empty() {
            Artwork::None
        } else {
            Artwork::Remote(thumbs)
        },
    })
}

fn entries_of(tx: &Transaction<'_>, id: LocalPlaylistId) -> rusqlite::Result<Vec<Stored>> {
    let mut stmt = tx.prepare(
        "SELECT path, cue_index, stream FROM playlist_entry
         WHERE playlist_id = ?1 ORDER BY position",
    )?;
    stmt.query_map([id.0], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect()
}

/// Replace the entries of a playlist with `entries`, numbered from 0.
fn write_entries(
    tx: &Transaction<'_>,
    id: LocalPlaylistId,
    entries: &[Stored],
) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM playlist_entry WHERE playlist_id = ?1", [id.0])?;
    for (position, (path, cue, stream)) in (0_i64..).zip(entries) {
        tx.execute(
            "INSERT INTO playlist_entry (playlist_id, position, path, cue_index, stream)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id.0, position, path, cue, stream],
        )?;
    }
    Ok(())
}

impl Store {
    /// Create an empty playlist.
    ///
    /// # Errors
    /// [`LibraryError::Db`] on a database failure.
    pub fn create_playlist(&mut self, name: &str) -> Result<LocalPlaylistId, LibraryError> {
        self.conn
            .execute(
                "INSERT INTO playlist (name, created_at) VALUES (?1, ?2)",
                params![name, now_secs()],
            )
            .map_err(db)?;
        Ok(LocalPlaylistId(self.conn.last_insert_rowid()))
    }

    /// Rename a playlist.
    ///
    /// # Errors
    /// [`LibraryError::Db`] when the playlist does not exist or the write fails.
    pub fn rename_playlist(&mut self, id: LocalPlaylistId, name: &str) -> Result<(), LibraryError> {
        let changed = self
            .conn
            .execute(
                "UPDATE playlist SET name = ?2 WHERE id = ?1",
                params![id.0, name],
            )
            .map_err(db)?;
        if changed == 0 {
            return Err(LibraryError::Db);
        }
        Ok(())
    }

    /// Delete a playlist and its entries. The files of its entries are not touched.
    ///
    /// # Errors
    /// [`LibraryError::Db`] on a database failure.
    pub fn delete_playlist(&mut self, id: LocalPlaylistId) -> Result<(), LibraryError> {
        self.conn
            .execute("DELETE FROM playlist WHERE id = ?1", [id.0])
            .map_err(db)?;
        Ok(())
    }

    /// Add entries to the end of a playlist. Returns how many were added.
    ///
    /// # Errors
    /// [`LibraryError::Db`] when the playlist does not exist or the write fails (nothing is
    /// added), or [`LibraryError::Io`] for a path that is not valid Unicode.
    pub fn add_entries(
        &mut self,
        id: LocalPlaylistId,
        new: &[NewEntry],
    ) -> Result<usize, LibraryError> {
        let tx = self.conn.transaction().map_err(db)?;
        let mut entries = entries_of(&tx, id).map_err(db)?;
        for entry in new {
            entries.push(match entry {
                NewEntry::File { path, cue_index } => {
                    (Some(path_text(path)?.to_owned()), *cue_index, None)
                }
                NewEntry::Stream(track) => (None, 0, Some(stream_json(track))),
            });
        }
        write_entries(&tx, id, &entries).map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(new.len())
    }

    /// Remove the entry at `index`.
    ///
    /// # Errors
    /// [`LibraryError::Db`] when there is no such entry or the write fails.
    pub fn remove_entry(&mut self, id: LocalPlaylistId, index: usize) -> Result<(), LibraryError> {
        self.reorder(id, |entries| {
            (index < entries.len()).then(|| {
                entries.remove(index);
            })
        })
    }

    /// Move the entry at `from` to `to`.
    ///
    /// # Errors
    /// [`LibraryError::Db`] when either index is out of range or the write fails.
    pub fn move_entry(
        &mut self,
        id: LocalPlaylistId,
        from: usize,
        to: usize,
    ) -> Result<(), LibraryError> {
        self.reorder(id, |entries| {
            (from < entries.len() && to < entries.len()).then(|| {
                let entry = entries.remove(from);
                entries.insert(to, entry);
            })
        })
    }

    /// Change the entries with `change` (which says `None` for a bad index) in one
    /// transaction.
    fn reorder(
        &mut self,
        id: LocalPlaylistId,
        change: impl FnOnce(&mut Vec<Stored>) -> Option<()>,
    ) -> Result<(), LibraryError> {
        let tx = self.conn.transaction().map_err(db)?;
        let mut entries = entries_of(&tx, id).map_err(db)?;
        change(&mut entries).ok_or(LibraryError::Db)?;
        write_entries(&tx, id, &entries).map_err(db)?;
        tx.commit().map_err(db)
    }

    /// Every playlist with its entries, in the order they were made. A file entry is
    /// `missing` when the library holds no playable track for it.
    ///
    /// # Errors
    /// [`LibraryError::Db`] on a database failure.
    pub fn playlists(&self) -> Result<Vec<LocalPlaylist>, LibraryError> {
        let mut heads = self
            .conn
            .prepare("SELECT id, name FROM playlist ORDER BY id")
            .map_err(db)?;
        let heads: Vec<(i64, String)> = heads
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(db)?
            .collect::<Result<_, _>>()
            .map_err(db)?;
        let mut stmt = self
            .conn
            .prepare(
                "SELECT e.path, e.stream, t.status IS NULL OR t.status = 'missing'
                 FROM playlist_entry e
                 LEFT JOIN track t ON t.path = e.path AND t.cue_index = e.cue_index
                 WHERE e.playlist_id = ?1 ORDER BY e.position",
            )
            .map_err(db)?;
        let mut out = Vec::with_capacity(heads.len());
        for (id, name) in heads {
            let entries = stmt
                .query_map([id], |r| {
                    let path: Option<String> = r.get(0)?;
                    let stream: Option<String> = r.get(1)?;
                    let gone: bool = r.get(2)?;
                    Ok(PlaylistEntry {
                        missing: path.is_some() && gone,
                        path: path.map(PathBuf::from),
                        track: stream.as_deref().and_then(stream_track),
                    })
                })
                .map_err(db)?
                .collect::<Result<_, _>>()
                .map_err(db)?;
            out.push(LocalPlaylist {
                id: LocalPlaylistId(id),
                name,
                entries,
            });
        }
        Ok(out)
    }
}
