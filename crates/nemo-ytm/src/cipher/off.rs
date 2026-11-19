//! The in-process tier switches itself off for a day when a network refuses its addresses.
//!
//! Nobody has seen the tier work on a real network yet: googlevideo may answer 403 for the
//! addresses of the `tv` clients. The resolver chain already stops using the tier for the rest
//! of a session after [`PRIMARY_FAILURE_LIMIT`](crate::resolver::PRIMARY_FAILURE_LIMIT) refused
//! streams. This marker keeps that decision across restarts for [`OFF_FOR`], so such a network
//! pays the extra time of a refused address at most three times a day. The file holds one
//! number, the Unix time the rest ends. No address, id or account is in it.

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// How long the tier stays off after it switched itself off.
pub const OFF_FOR: Duration = Duration::from_hours(24);

#[derive(Debug, Serialize, Deserialize)]
struct Rest {
    /// Seconds since the Unix epoch.
    until: u64,
}

/// The "off" marker file.
#[derive(Debug, Clone)]
pub struct OffMarker {
    file: Option<PathBuf>,
}

impl OffMarker {
    /// A marker kept in `file` (`None`: nothing is remembered).
    #[must_use]
    pub fn new(file: Option<PathBuf>) -> Self {
        Self { file }
    }

    /// `true` while the tier is off: the marker names a time later than `now`. A missing,
    /// damaged or expired marker is "on"; an expired file is deleted.
    #[must_use]
    pub fn resting(&self, now: SystemTime) -> bool {
        let Some(path) = &self.file else {
            return false;
        };
        let until = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<Rest>(&text).ok())
            .and_then(|rest| UNIX_EPOCH.checked_add(Duration::from_secs(rest.until)));
        match until {
            Some(until) if until > now => true,
            Some(_) => {
                self.clear();
                false
            }
            None => false,
        }
    }

    /// Switch the tier off until `now` plus [`OFF_FOR`], and log it once at info level.
    pub fn switch_off(&self, now: SystemTime) {
        tracing::info!(
            hours = OFF_FOR.as_secs() / 3600,
            "the in-process resolver switched itself off: its streams were refused"
        );
        let Some(path) = &self.file else { return };
        let until = now
            .checked_add(OFF_FOR)
            .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        if write_private(path, &Rest { until }).is_err() {
            tracing::warn!("the in-process resolver marker could not be written");
        }
    }

    /// Delete the marker.
    pub fn clear(&self) {
        if let Some(path) = &self.file {
            // Already gone is the same as deleted.
            let _ = std::fs::remove_file(path);
        }
    }

    /// The function the resolver chain calls when it stops using the tier: it writes the marker.
    #[must_use]
    pub fn hook(self: &Arc<Self>) -> Arc<dyn Fn() + Send + Sync> {
        let marker = Arc::clone(self);
        Arc::new(move || marker.switch_off(SystemTime::now()))
    }
}

fn write_private(path: &std::path::Path, rest: &Rest) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut out = options.open(&tmp)?;
    out.write_all(&serde_json::to_vec(rest).map_err(std::io::Error::other)?)?;
    out.sync_data()?;
    drop(out);
    std::fs::rename(&tmp, path)
}
