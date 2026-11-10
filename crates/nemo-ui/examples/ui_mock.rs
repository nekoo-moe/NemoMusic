//! Opens the UI on the mock backend (no network, no audio device).
//!
//! `cargo run -p nemo-ui --example ui_mock --features gui --release`
//! Press F3 for the frame-time readout.

fn main() -> iced::Result {
    nemo_ui::run(nemo_ui::mock::mock_backend(), nemo_ui::UiOptions::default())
}
