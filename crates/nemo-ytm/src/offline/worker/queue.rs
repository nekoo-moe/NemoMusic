//! The download queue in `offline.db` and the downloader into the cache folders.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use nemo_core::audio::StreamInfo;
use nemo_core::downloads::FailReason;
use nemo_core::secret::Secret;
use nemo_core::track::VideoId;
use rusqlite::{OptionalExtension, Transaction, params};

use super::{Download, Job, Queue};
use crate::offline::fetch::{FetchError, Fetched, Fetcher, Target};
use crate::offline::store::{BLOBS, TMP};
use crate::offline::{OfflineCache, OfflineError, is_opaque_name, opaque_name};

/// The plain reason stored in `queue.fail_reason`.
pub(in crate::offline) const fn fail_text(reason: FailReason) -> &'static str {
    match reason {
        FailReason::LimitReached => "limit_reached",
        FailReason::DiskFull => "disk_full",
        FailReason::NotAvailable => "not_available",
        FailReason::Network => "network",
        FailReason::RateLimited => "rate_limited",
        FailReason::Damaged => "damaged",
    }
}

/// The first row to download: a row left `saving` by an earlier run (it resumes), else the
/// first queued one. Rows with an id that does not parse are marked damaged and skipped.
fn take_next(tx: &Transaction<'_>) -> Result<Option<Job>, OfflineError> {
    loop {
        let row: Option<(String, Option<String>)> = tx
            .query_row(
                "SELECT video_id, part_file FROM queue WHERE state IN ('queued', 'saving')
                 ORDER BY state = 'saving' DESC, position, added_at LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((id, part)) = row else {
            return Ok(None);
        };
        let Ok(video) = VideoId::parse(&id) else {
            tx.execute(
                "UPDATE queue SET state = 'failed', fail_reason = ?1 WHERE video_id = ?2",
                params![fail_text(FailReason::Damaged), id],
            )?;
            continue;
        };
        let file = match part.filter(|name| is_opaque_name(name)) {
            Some(name) => name,
            None => opaque_name()?,
        };
        tx.execute(
            "UPDATE queue SET state = 'saving', part_file = ?1 WHERE video_id = ?2",
            params![file, id],
        )?;
        return Ok(Some(Job { video, file }));
    }
}

impl Queue for OfflineCache {
    fn next(&self) -> Result<Option<Job>, OfflineError> {
        self.store.with_conn(|conn| {
            let tx = conn.transaction()?;
            let job = take_next(&tx)?;
            tx.commit()?;
            Ok(job)
        })
    }

    fn saved(&self, job: &Job, bytes: u64, info: &StreamInfo) -> Result<(), OfflineError> {
        let saved_at = self.clock.now_secs();
        let bytes = i64::try_from(bytes).map_err(|_| OfflineError::Db)?;
        let codec = info.codec.name().to_ascii_lowercase();
        self.store.with_conn(|conn| {
            let tx = conn.transaction()?;
            let (reason, meta, single): (String, String, i64) = tx.query_row(
                "SELECT reason, meta, single FROM queue WHERE video_id = ?1",
                [job.video.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            tx.execute(
                "INSERT INTO song (video_id, file, bytes, codec, bitrate_kbps, reason, meta,
                                   saved_at, single)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    job.video.as_str(),
                    job.file,
                    bytes,
                    codec,
                    info.bitrate_kbps,
                    reason,
                    meta,
                    saved_at,
                    single
                ],
            )?;
            tx.execute(
                "DELETE FROM queue WHERE video_id = ?1",
                [job.video.as_str()],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn requeue(&self, job: &Job, done: u64) -> Result<(), OfflineError> {
        let done = i64::try_from(done).map_err(|_| OfflineError::Db)?;
        self.store.with_conn(|conn| {
            conn.execute(
                "UPDATE queue SET state = 'queued', done_bytes = ?1 WHERE video_id = ?2",
                params![done, job.video.as_str()],
            )?;
            Ok(())
        })
    }

    fn failed(&self, job: &Job, reason: FailReason) -> Result<(), OfflineError> {
        self.store.with_conn(|conn| {
            conn.execute(
                "UPDATE queue SET state = 'failed', fail_reason = ?1 WHERE video_id = ?2",
                params![fail_text(reason), job.video.as_str()],
            )?;
            Ok(())
        })
    }
}

impl OfflineCache {
    /// The user retries one failed song by hand: it is queued once more. `false` when the
    /// song is not a failed queue entry. The worker must be woken afterwards.
    ///
    /// # Errors
    /// Returns [`OfflineError::Db`] when the index cannot be changed.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "`DownloadService::retry` (offline-cache task 12) is the caller"
        )
    )]
    pub(in crate::offline) fn retry_download(&self, video: &VideoId) -> Result<bool, OfflineError> {
        self.store.with_conn(|conn| {
            let changed = conn.execute(
                "UPDATE queue SET state = 'queued', fail_reason = NULL
                 WHERE video_id = ?1 AND state = 'failed'",
                [video.as_str()],
            )?;
            Ok(changed > 0)
        })
    }
}

/// Downloads into `tmp/<name>.part` and moves the complete file to `blobs/<name>`.
pub(in crate::offline) struct CacheDownload {
    fetcher: Fetcher,
    root: PathBuf,
}

impl CacheDownload {
    /// A downloader into the cache folder `root`. `https_only` is `false` only for the
    /// loopback test server.
    pub(in crate::offline) fn new(root: &Path, https_only: bool) -> Result<Self, FetchError> {
        Ok(Self {
            fetcher: Fetcher::new(https_only)?,
            root: root.to_path_buf(),
        })
    }
}

impl Download for CacheDownload {
    fn fetch(
        &self,
        url: &Secret<String>,
        job: &Job,
        pause: &AtomicBool,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<Fetched, FetchError> {
        // Paths are built only from opaque names, so no path can hold an id, a title or `..`.
        if !is_opaque_name(&job.file) {
            return Err(FetchError::Io);
        }
        let part = self.root.join(TMP).join(format!("{}.part", job.file));
        let dest = self.root.join(BLOBS).join(&job.file);
        self.fetcher.fetch(
            url,
            Target {
                part: &part,
                dest: &dest,
            },
            pause,
            progress,
        )
    }
}
