//! Mica or acrylic blur behind the see-through window (bar-material spec). Windows only; the
//! crate `window-vibrancy` does the calls and `nemo-ui` never names it. The UI hands the window
//! over once, when the window was created see-through and Reduce transparency is off.
//!
//! Windows 11 (build 22000 or newer) gets Mica. Older Windows 10 (1809 or newer) gets acrylic,
//! tinted from the theme base and the opacity setting. If neither works, the plain transparent
//! window stays and one warning goes to the log.

use std::fmt::Display;
use std::sync::Arc;

use nemo_ui::WindowHandler;

/// The hook for `UiOptions::on_window_ready`.
pub fn handler() -> WindowHandler {
    Arc::new(|window, look| {
        let (red, green, blue, alpha) = look.tint();
        blur_with(
            || window_vibrancy::apply_mica(window, Some(look.dark)),
            || window_vibrancy::apply_acrylic(window, Some((red, green, blue, alpha))),
        );
    })
}

/// Try Mica, then acrylic. Mica failing is normal on Windows 10, so only both failing is logged,
/// once, with the reason of the last call. No window handle is ever printed.
fn blur_with<E: Display>(
    mica: impl FnOnce() -> Result<(), E>,
    acrylic: impl FnOnce() -> Result<(), E>,
) {
    if mica().is_ok() {
        return;
    }
    if let Err(err) = acrylic() {
        tracing::warn!(error = %err, "no blur behind the window; the plain see-through window stays");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn mica_that_works_is_the_only_call() {
        let acrylic = Cell::new(false);
        blur_with(
            || Ok::<(), String>(()),
            || {
                acrylic.set(true);
                Ok(())
            },
        );
        assert!(!acrylic.get());
    }

    #[test]
    fn acrylic_is_tried_when_mica_is_not_there() {
        let acrylic = Cell::new(false);
        blur_with(
            || Err("Windows 10".to_owned()),
            || {
                acrylic.set(true);
                Ok(())
            },
        );
        assert!(acrylic.get());
    }

    #[test]
    fn both_failing_does_not_panic() {
        blur_with(|| Err("a".to_owned()), || Err("b".to_owned()));
    }
}
