//! The OS "reduce motion" preference, read once at start (app-motion spec). The UI combines it
//! with the user's own switch in Settings: either one turns motion off.
//!
//! Linux: GNOME's `org.gnome.desktop.interface enable-animations` through `gsettings` (a local
//! process, no network). A missing `gsettings`, another desktop or a slow answer reads as "motion
//! on". Windows: not read yet (the user switch still works).

/// Longest wait for the OS answer, so start-up never stalls on it.
#[cfg(target_os = "linux")]
const WAIT: std::time::Duration = std::time::Duration::from_millis(300);

/// `true` when the OS asks apps to reduce motion.
#[must_use]
pub fn reduce_motion() -> bool {
    #[cfg(target_os = "linux")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("os-motion".into())
            .spawn(move || {
                let out = std::process::Command::new("gsettings")
                    .args(["get", "org.gnome.desktop.interface", "enable-animations"])
                    .stderr(std::process::Stdio::null())
                    .output();
                let reduce = out
                    .ok()
                    .filter(|o| o.status.success())
                    .is_some_and(|o| animations_off(&String::from_utf8_lossy(&o.stdout)));
                // The receiver may have stopped waiting; then the answer is not needed.
                let _ = tx.send(reduce);
            });
        if spawned.is_err() {
            return false;
        }
        let reduce = rx.recv_timeout(WAIT).unwrap_or(false);
        tracing::info!(reduce, "OS reduce-motion preference");
        reduce
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// `true` when a `gsettings` boolean answer says animations are off.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn animations_off(answer: &str) -> bool {
    answer.trim() == "false"
}

#[cfg(test)]
mod tests {
    use super::animations_off;

    #[test]
    fn gsettings_false_means_reduce_motion() {
        assert!(animations_off("false\n"));
        assert!(!animations_off("true\n"));
        assert!(!animations_off(""));
        assert!(!animations_off("No such schema"));
    }
}
