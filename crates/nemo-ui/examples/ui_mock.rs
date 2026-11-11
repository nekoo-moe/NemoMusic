//! Opens the UI on the mock backend (no network, no audio device).
//!
//! `cargo run -p nemo-ui --example ui_mock --features gui --release`
//! Press F3 for the frame-time readout.
//!
//! Development switches (environment variables):
//! * `NEMO_MOCK_SIZE=1366x697` opens the window at that logical size.
//! * `NEMO_MOCK_MAXIMIZED=1` opens the window maximized (a 1366 x 768 laptop gives about 1366 x 697).
//! * `NEMO_MOCK_NOTICE=1` shows the first-run notice (by default it counts as accepted).
//! * `NEMO_MOCK_SIGNED_IN=1` starts with a signed-in mock account.
//! * `NEMO_MOCK_READOUT=1` starts with the frame-time readout on.

use nemo_ui::{UiOptions, UiSettings};

fn flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == "1")
}

fn size() -> Option<(f32, f32)> {
    let raw = std::env::var("NEMO_MOCK_SIZE").ok()?;
    let (w, h) = raw.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

fn main() -> iced::Result {
    let mut options = UiOptions {
        settings: UiSettings {
            tos_acknowledged: !flag("NEMO_MOCK_NOTICE"),
            ..UiSettings::default()
        },
        show_frame_readout: flag("NEMO_MOCK_READOUT"),
        maximized: flag("NEMO_MOCK_MAXIMIZED"),
        ..UiOptions::default()
    };
    if let Some(s) = size() {
        options.window_size = s;
    }
    let backend = if flag("NEMO_MOCK_SIGNED_IN") {
        nemo_ui::mock::mock_backend_signed_in()
    } else {
        nemo_ui::mock::mock_backend()
    };
    nemo_ui::run(backend, options)
}
