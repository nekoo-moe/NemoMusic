//! Log setup. Logs go to a file in the data folder and never hold secrets (`code.md`).
//!
//! The level comes from `NEMO_LOG` (`error`, `warn`, `info`, `debug`, `trace`); default `info`.

use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::Mutex;

use tracing::Level;

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
pub fn init(logs_dir: &Path) {
    let level = std::env::var("NEMO_LOG")
        .ok()
        .and_then(|v| parse_level(&v))
        .unwrap_or(Level::INFO);
    let builder = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_ansi(false)
        .with_target(true);
    let result = match open_log(logs_dir) {
        Ok(file) => builder.with_writer(Mutex::new(file)).try_init(),
        Err(_) => builder.with_writer(std::io::stderr).try_init(),
    };
    // A second init (tests, or a subscriber set by a host) is harmless: keep the first.
    let _ = result;
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
