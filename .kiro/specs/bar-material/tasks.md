# Material player bar, Restart now, Windows blur: tasks

Owner approved 2026-11-19 (request in `requirements.md`).

## Tasks
- [x] 1. The bar: a flat Material surface with no outline, rim or glint; opaque under Reduce transparency and over a see-through window; the `Player bar` card without chips; preview on the same fill; `design.md` sections 4, 6 and 7 (owner, 2026-11-19; bar-material); snapshots `bar-material-*`. Files: `nemo-ui`: `src/gui/views/bar/surface.rs`, `src/gui/views/bar.rs`, `src/theme/glass.rs`, `src/settings.rs`, `src/copy/appearance.rs`, `src/gui/views/settings/appearance.rs`, `src/gui/views/settings/appearance/preview.rs`, `examples/snapshots/player_bar.rs`. Checks: fmt, clippy `-D warnings`, tests for `nemo-ui` and `nemo-app`. _Requirements: 1, 2, 3, 4, 5, 6, 7_
- [x] 2. Restart now: the pill beside the Window glass switch, its entrance, `UiOptions::on_restart`, the `nemo-app` implementation (save, spawn, exit), the failure sentence, tests and a snapshot. Files: `nemo-ui`: `src/gui/views/settings/appearance.rs`, `src/gui/app.rs`, `src/state.rs`, `src/lib.rs`, `src/copy/appearance.rs`; `nemo-app`: `src/restart.rs` (new), `src/ui_bridge.rs`, `src/main.rs`. Checks: as task 1. _Requirements: 8, 9, 10, 11, 12, 13, 14_
- [x] 3. Windows blur: `window-vibrancy` `=0.8.1` for `cfg(windows)` in `nemo-app`, the `on_window_ready` hook, Mica or acrylic with the tint, one warning on failure, the helper sentence. Files: `nemo-app`: `Cargo.toml`, `Cargo.lock`, `src/blur.rs` (new, `cfg(windows)`), `src/main.rs`, `src/ui_bridge.rs`; `nemo-ui`: `src/window_glass.rs` (`WindowLook`), `src/gui/shell.rs`, `src/gui/app/option.rs`, `src/lib.rs`, `src/copy/appearance.rs`. Checks: as task 1, `cargo deny check` if available, CI's Windows job. _Requirements: 15, 16, 17, 18, 19_

## Definition of Done
See `.kiro/steering/workflow.md` section 4.
