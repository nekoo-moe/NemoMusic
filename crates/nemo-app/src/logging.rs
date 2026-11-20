//! Log setup. Logs go to a file in the data folder and never hold secrets (`code.md`).
//!
//! The level comes from `NEMO_LOG` (`error`, `warn`, `info`, `debug`, `trace`); default `info`.

use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::Mutex;

use tracing::Level;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::prelude::*;

use crate::dev_capture::{CaptureLayer, DevRing, RING_CAPACITY, capture_filter};

/// Name of the log file inside the logs folder.
pub const LOG_FILE: &str = "nemo.log";

/// Largest log file kept at startup. A bigger file is moved to `nemo.old.log`.
pub const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;

/// Read a level name. Unknown or empty text gives `None`.
#[must_use]
pub fn parse_level(text: &str) -> Option<Level> {
    match text.trim().to_ascii_lowercase().as_str() {
        "error" => Some(Level::ERROR),
        "warn" | "warning" => Some(Level::WARN),
        "info" => Some(Level::INFO),
        "debug" => Some(Level::DEBUG),
        "trace" => Some(Level::TRACE),
        _ => None,
    }
}

/// Start logging into `logs_dir`. Falls back to stderr when the file cannot be opened.
///
/// Returns the developer ring that the capture layer fills (dev-tools spec). The ring lives in
/// memory only; the log file and stderr are unchanged by it.
pub fn init(logs_dir: &Path) -> DevRing {
    let level = std::env::var("NEMO_LOG")
        .ok()
        .and_then(|v| parse_level(&v))
        .unwrap_or(Level::INFO);
    let writer = match open_log(logs_dir) {
        Ok(file) => BoxMakeWriter::new(Mutex::new(file)),
        Err(_) => BoxMakeWriter::new(std::io::stderr),
    };
    let ring = DevRing::new(RING_CAPACITY);
    // A second init (tests, or a subscriber set by a host) is harmless: keep the first.
    let _ = build(writer, level, CaptureLayer::new(ring.clone())).try_init();
    ring
}

/// The subscriber: the text layer at the `NEMO_LOG` level and the capture layer with its own
/// filter. Each layer has its own filter, so the ring also receives provider trace events when
/// the text layer is at `info`.
pub(crate) fn build(
    writer: BoxMakeWriter,
    level: Level,
    capture: CaptureLayer,
) -> impl tracing::Subscriber + Send + Sync + 'static {
    let text = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_writer(writer)
        .with_filter(LevelFilter::from_level(level));
    tracing_subscriber::registry()
        .with(text)
        .with(capture.with_filter(capture_filter(level)))
}

/// Open the log file for appending, rotating it once when it is too big.
fn open_log(logs_dir: &Path) -> std::io::Result<File> {
    let path = logs_dir.join(LOG_FILE);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
        std::fs::rename(&path, logs_dir.join("nemo.old.log"))?;
    }
    OpenOptions::new().create(true).append(true).open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_parse_case_insensitively() {
        assert_eq!(parse_level("DEBUG"), Some(Level::DEBUG));
        assert_eq!(parse_level(" warning "), Some(Level::WARN));
        assert_eq!(parse_level("loud"), None);
        assert_eq!(parse_level(""), None);
    }

    #[test]
    fn a_large_log_is_rotated_once() {
        let dir = std::env::temp_dir().join(format!("nemo-app-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let big = vec![b'x'; usize::try_from(MAX_LOG_BYTES + 1).expect("fits")];
        std::fs::write(dir.join(LOG_FILE), big).expect("write");
        let file = open_log(&dir).expect("open");
        assert_eq!(file.metadata().expect("meta").len(), 0);
        assert!(dir.join("nemo.old.log").is_file());
        std::fs::remove_dir_all(dir).expect("cleanup");
    }
}
