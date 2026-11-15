//! Track rows: the scan writes them in batches, the start-up load reads them all, and the
//! prepare thread reads one file's span and gain.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use nemo_core::library::{
    LibraryError, LocalPlayback, ReplayGainValues, TrackSpan, TrackStatus, UnplayableReason,
};
use rusqlite::{OptionalExtension, Row, Transaction, params};

use super::{Store, db, path_text};

/// Everything the library stores about one track.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrackRow {
    /// The audio file.
    pub path: PathBuf,
    /// 0 for the whole file, `n` for track `n` of the file's CUE sheet.
    pub cue_index: u32,
    /// File size in bytes when it was read.
    pub size: u64,
    /// File modified time when it was read, in milliseconds since the Unix epoch.
    pub modified_ms: i64,
    /// Title (the file stem when untagged).
    pub title: String,
    /// Artist names.
    pub artists: Vec<String>,
    /// Album title.
    pub album: Option<String>,
    /// Album artist.
    pub album_artist: Option<String>,
    /// Disc number.
    pub disc: Option<u32>,
    /// Track number.
    pub number: Option<u32>,
    /// Year.
    pub year: Option<u32>,
    /// Genre.
    pub genre: Option<String>,
    /// Duration of the track (of its span for a CUE track).
    pub duration: Option<Duration>,
    /// Whether it can be played, and why not.
    pub status: TrackStatus,
    /// The span of the file a CUE track plays.
    pub span: Option<TrackSpan>,
    /// Stored `ReplayGain` values.
    pub gain: ReplayGainValues,
}

/// A stored `REAL` back to the `f32` it was written from.
#[allow(
    clippy::cast_possible_truncation,
    reason = "every stored gain, peak and loudness was written from an f32"
)]
pub(super) fn narrow(value: f64) -> f32 {
    value as f32
}

fn ms(d: Duration) -> i64 {
    i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
}

fn duration(ms: Option<i64>) -> Option<Duration> {
    ms.and_then(|m| u64::try_from(m).ok())
        .map(Duration::from_millis)
}

fn number(value: Option<i64>) -> Option<u32> {
    value.and_then(|n| u32::try_from(n).ok())
}

fn status_text(status: TrackStatus) -> (&'static str, Option<&'static str>) {
    match status {
        TrackStatus::Playable => ("playable", None),
        TrackStatus::Missing => ("missing", None),
        TrackStatus::Unplayable(reason) => (
            "unplayable",
            Some(match reason {
                UnplayableReason::UnsupportedFormat => "unsupported",
                UnplayableReason::DecodeFailed => "decode",
                UnplayableReason::Unreadable => "unreadable",
            }),
        ),
    }
}

fn status_of(status: &str, reason: Option<&str>) -> TrackStatus {
    match (status, reason) {
        ("missing", _) => TrackStatus::Missing,
        ("unplayable", Some("unsupported")) => {
            TrackStatus::Unplayable(UnplayableReason::UnsupportedFormat)
        }
        ("unplayable", Some("decode")) => TrackStatus::Unplayable(UnplayableReason::DecodeFailed),
        ("unplayable", _) => TrackStatus::Unplayable(UnplayableReason::Unreadable),
        _ => TrackStatus::Playable,
    }
}

/// The album row of `title` and `album_artist`, made when it is new.
fn album_id(tx: &Transaction<'_>, title: &str, album_artist: &str) -> rusqlite::Result<i64> {
    tx.execute(
        "INSERT OR IGNORE INTO album (title, album_artist) VALUES (?1, ?2)",
        params![title, album_artist],
    )?;
    tx.query_row(
        "SELECT id FROM album WHERE title = ?1 AND album_artist = ?2",
        params![title, album_artist],
        |r| r.get(0),
    )
}

/// Insert a track, or replace the row of the same file and CUE index.
const UPSERT_TRACK: &str =
    "INSERT INTO track (folder_id, path, cue_index, size, modified_ms, title, artists,
         album_id, album_artist, disc, number, year, genre, duration_ms, status, reason,
         span_start_ms, span_end_ms, track_gain_db, album_gain_db, track_peak, album_peak)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
         ?18, ?19, ?20, ?21, ?22)
     ON CONFLICT (path, cue_index) DO UPDATE SET
         folder_id = excluded.folder_id, size = excluded.size,
         modified_ms = excluded.modified_ms, title = excluded.title,
         artists = excluded.artists, album_id = excluded.album_id,
         album_artist = excluded.album_artist, disc = excluded.disc,
         number = excluded.number, year = excluded.year, genre = excluded.genre,
         duration_ms = excluded.duration_ms, status = excluded.status,
         reason = excluded.reason, span_start_ms = excluded.span_start_ms,
         span_end_ms = excluded.span_end_ms, track_gain_db = excluded.track_gain_db,
         album_gain_db = excluded.album_gain_db, track_peak = excluded.track_peak,
         album_peak = excluded.album_peak";

fn insert(tx: &Transaction<'_>, folder: i64, row: &TrackRow) -> Result<(), LibraryError> {
    let album = match &row.album {
        Some(title) => {
            Some(album_id(tx, title, row.album_artist.as_deref().unwrap_or("")).map_err(db)?)
        }
        None => None,
    };
    let artists = serde_json::to_string(&row.artists).map_err(db)?;
    let (status, reason) = status_text(row.status);
    let span = row.span;
    tx.execute(
        UPSERT_TRACK,
        params![
            folder,
            path_text(&row.path)?,
            row.cue_index,
            i64::try_from(row.size).unwrap_or(i64::MAX),
            row.modified_ms,
            row.title,
            artists,
            album,
            row.album_artist,
            row.disc,
            row.number,
            row.year,
            row.genre,
            row.duration.map(ms),
            status,
            reason,
            span.map(|s| ms(s.start)),
            span.and_then(|s| s.end).map(ms),
            row.gain.track_gain_db.map(f64::from),
            row.gain.album_gain_db.map(f64::from),
            row.gain.track_peak.map(f64::from),
            row.gain.album_peak.map(f64::from),
        ],
    )
    .map_err(db)?;
    Ok(())
}

const SELECT_TRACK: &str = "SELECT t.path, t.cue_index, t.size, t.modified_ms, t.title, t.artists,
        a.title, t.album_artist, t.disc, t.number, t.year, t.genre, t.duration_ms, t.status,
        t.reason, t.span_start_ms, t.span_end_ms, t.track_gain_db, t.album_gain_db,
        t.track_peak, t.album_peak
    FROM track t LEFT JOIN album a ON a.id = t.album_id";

fn gain_of(r: &Row<'_>, first: usize) -> rusqlite::Result<ReplayGainValues> {
    let value = |i: usize| -> rusqlite::Result<Option<f32>> {
        Ok(r.get::<_, Option<f64>>(first + i)?.map(narrow))
    };
    Ok(ReplayGainValues {
        track_gain_db: value(0)?,
        album_gain_db: value(1)?,
        track_peak: value(2)?,
        album_peak: value(3)?,
    })
}

fn span_of(start: Option<i64>, end: Option<i64>) -> Option<TrackSpan> {
    duration(start).map(|start| TrackSpan {
        start,
        end: duration(end),
    })
}

fn row_of(r: &Row<'_>) -> rusqlite::Result<TrackRow> {
    let artists: String = r.get(5)?;
    let status: String = r.get(13)?;
    let reason: Option<String> = r.get(14)?;
    Ok(TrackRow {
        path: PathBuf::from(r.get::<_, String>(0)?),
        cue_index: r.get(1)?,
        size: u64::try_from(r.get::<_, i64>(2)?).unwrap_or(0),
        modified_ms: r.get(3)?,
        title: r.get(4)?,
        artists: serde_json::from_str(&artists).unwrap_or_default(),
        album: r.get(6)?,
        album_artist: r.get(7)?,
        disc: number(r.get(8)?),
        number: number(r.get(9)?),
        year: number(r.get(10)?),
        genre: r.get(11)?,
        duration: duration(r.get(12)?),
        status: status_of(&status, reason.as_deref()),
        span: span_of(r.get(15)?, r.get(16)?),
        gain: gain_of(r, 17)?,
    })
}

impl Store {
    /// Write a batch of tracks found under the watched `folder` in one transaction. A row
    /// for the same file and CUE index is replaced.
    ///
    /// # Errors
    /// [`LibraryError::Db`] when the folder is not watched or the write fails (nothing of
    /// the batch is kept), or [`LibraryError::Io`] for a path that is not valid Unicode.
    pub fn write_tracks(&mut self, folder: &Path, rows: &[TrackRow]) -> Result<(), LibraryError> {
        let tx = self.conn.transaction().map_err(db)?;
        let folder_id: i64 = tx
            .query_row(
                "SELECT id FROM folder WHERE path = ?1",
                [path_text(folder)?],
                |r| r.get(0),
            )
            .map_err(db)?;
        for row in rows {
            insert(&tx, folder_id, row)?;
        }
        tx.commit().map_err(db)
    }

    /// Size and modified time of every file under the watched `folder`, so a scan re-reads a
    /// file only when one of them changed.
    ///
    /// # Errors
    /// [`LibraryError::Io`] for a path that is not valid Unicode, or [`LibraryError::Db`].
    pub fn stamps(&self, folder: &Path) -> Result<HashMap<PathBuf, (u64, i64)>, LibraryError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT t.path, t.size, t.modified_ms FROM track t
                 JOIN folder f ON f.id = t.folder_id WHERE f.path = ?1",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map([path_text(folder)?], |r| {
                let size = u64::try_from(r.get::<_, i64>(1)?).unwrap_or(0);
                Ok((PathBuf::from(r.get::<_, String>(0)?), (size, r.get(2)?)))
            })
            .map_err(db)?
            .collect::<Result<_, _>>()
            .map_err(db)?;
        Ok(rows)
    }

    /// Remove the rows of files that are gone (every CUE track of each). Returns how many
    /// rows went. No file is touched.
    ///
    /// # Errors
    /// [`LibraryError::Io`] for a path that is not valid Unicode, or [`LibraryError::Db`].
    pub fn remove_files(&mut self, paths: &[PathBuf]) -> Result<usize, LibraryError> {
        let tx = self.conn.transaction().map_err(db)?;
        let mut removed = 0;
        for path in paths {
            removed += tx
                .execute("DELETE FROM track WHERE path = ?1", [path_text(path)?])
                .map_err(db)?;
        }
        tx.commit().map_err(db)?;
        Ok(removed)
    }

    /// Every track, ordered by path and CUE index, for the start-up load.
    ///
    /// # Errors
    /// [`LibraryError::Db`] on a database failure.
    pub fn all_tracks(&self) -> Result<Vec<TrackRow>, LibraryError> {
        let sql = format!("{SELECT_TRACK} ORDER BY t.path, t.cue_index");
        let mut stmt = self.conn.prepare(&sql).map_err(db)?;
        let rows = stmt
            .query_map([], row_of)
            .map_err(db)?
            .collect::<Result<_, _>>()
            .map_err(db)?;
        Ok(rows)
    }

    /// The span and gain of one track, for the prepare thread; `None` when it is not stored.
    ///
    /// # Errors
    /// [`LibraryError::Io`] for a path that is not valid Unicode, or [`LibraryError::Db`].
    pub fn playback(
        &self,
        path: &Path,
        cue_index: u32,
    ) -> Result<Option<LocalPlayback>, LibraryError> {
        let sql = format!("{SELECT_TRACK} WHERE t.path = ?1 AND t.cue_index = ?2");
        self.conn
            .query_row(&sql, params![path_text(path)?, cue_index], row_of)
            .optional()
            .map_err(db)
            .map(|row| {
                row.map(|r| LocalPlayback {
                    span: r.span,
                    gain: r.gain,
                })
            })
    }
}
