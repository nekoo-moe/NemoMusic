//! Restart on request of the UI (bar-material spec). The UI never starts a process: this module
//! makes sure the settings are on disk, then starts a new copy of this program with the same
//! arguments. The UI closes its own window afterwards, through iced, so the audio device is
//! released by the normal shutdown path.

use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nemo_ui::gui::RestartHandler;

use crate::ui_bridge::SettingsMsg;

/// How long to wait for the settings to reach the disk.
const SAVE_WAIT: Duration = Duration::from_secs(2);

/// The hook for `UiOptions::on_restart`: flush the settings through `saver`, then start a new
/// copy of the program.
pub fn handler(saver: Sender<SettingsMsg>) -> RestartHandler {
    let saver = Mutex::new(saver);
    Arc::new(move || {
        restart_with(
            || flush(&saver),
            || {
                let exe = std::env::current_exe()?;
                std::process::Command::new(exe)
                    .args(std::env::args_os().skip(1))
                    .spawn()
                    .map(drop)
            },
        )
    })
}

/// Ask the saver thread to answer once everything sent before has been written. `false` when it
/// does not answer in time or is gone.
fn flush(saver: &Mutex<Sender<SettingsMsg>>) -> bool {
    let (ack, done) = channel();
    let sent = saver
        .lock()
        .is_ok_and(|tx| tx.send(SettingsMsg::Flush(ack)).is_ok());
    sent && done.recv_timeout(SAVE_WAIT).is_ok()
}

/// Flush, then spawn. Without the flush the new copy could start with the old settings and the
/// restart would change nothing, so a failed flush stops here. `true` when the new copy started.
fn restart_with(flush: impl FnOnce() -> bool, spawn: impl FnOnce() -> std::io::Result<()>) -> bool {
    if !flush() {
        tracing::warn!("the settings could not be saved in time, so the app did not restart");
        return false;
    }
    match spawn() {
        Ok(()) => true,
        Err(err) => {
            tracing::warn!(error = %err, "the app could not start a new copy of itself");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn the_settings_are_saved_before_the_new_copy_starts() {
        let log = RefCell::new(Vec::new());
        let started = restart_with(
            || {
                log.borrow_mut().push("flush");
                true
            },
            || {
                log.borrow_mut().push("spawn");
                Ok(())
            },
        );
        assert!(started);
        assert_eq!(*log.borrow(), ["flush", "spawn"]);
    }

    #[test]
    fn a_failed_save_starts_nothing() {
        let spawned = RefCell::new(false);
        let started = restart_with(
            || false,
            || {
                *spawned.borrow_mut() = true;
                Ok(())
            },
        );
        assert!(!started && !*spawned.borrow());
    }

    #[test]
    fn a_failed_spawn_is_reported_as_not_started() {
        let started = restart_with(|| true, || Err(std::io::Error::other("no such program")));
        assert!(!started);
    }

    #[test]
    fn a_gone_saver_means_no_restart() {
        let (tx, rx) = channel();
        drop(rx);
        assert!(!flush(&Mutex::new(tx)));
    }
}
