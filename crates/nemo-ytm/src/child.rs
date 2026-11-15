//! Starting a child program (the sign-in window, `yt-dlp`).
//!
//! On Linux, `exec` of a file fails with `ETXTBSY` ("text file busy") while any process still
//! holds that file open for writing. A file the app has just written (the yt-dlp copy it
//! installs, a test helper script) can stay open for a moment in a child that another thread
//! forked in that window: the inherited descriptor closes only when that child calls `exec`.
//! The condition clears within milliseconds, so the start is retried a few times.

use std::io::{self, ErrorKind};
use std::time::Duration;

use tokio::process::{Child, Command};

/// How many times a start is tried in all.
const ATTEMPTS: u32 = 5;
/// Pause between two tries.
const PAUSE: Duration = Duration::from_millis(25);

/// Start `command`, retrying only while the program file is busy.
///
/// # Errors
/// The last start error: at once for any error other than "text file busy", or after
/// [`ATTEMPTS`] busy tries.
pub(crate) async fn spawn(command: &mut Command) -> io::Result<Child> {
    let mut tries = 1;
    loop {
        match command.spawn() {
            Err(e) if e.kind() == ErrorKind::ExecutableFileBusy && tries < ATTEMPTS => {
                tries += 1;
                tokio::time::sleep(PAUSE).await;
            }
            other => return other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_program_fails_at_once_with_not_found() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");
        let started = std::time::Instant::now();
        let err = runtime
            .block_on(spawn(&mut Command::new("nemo-no-such-program-xyz")))
            .expect_err("no such program");
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(started.elapsed() < PAUSE, "no retry for other errors");
    }
}
