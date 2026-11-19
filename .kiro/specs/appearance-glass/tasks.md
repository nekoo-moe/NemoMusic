# Appearance and window glass: tasks

Owner approved 2026-11-19 (request in `requirements.md`).

## Tasks
- [x] 1. Model: `window_glass`, `window_glass_opacity`, the pure `window_glass` module, config and bridge, `design.md` sections 4, 6 and 7 wording (owner, 2026-11-19; appearance-glass). Crates and files: `nemo-ui`: `src/window_glass.rs` (new), `src/settings.rs`, `src/lib.rs`; `nemo-app`: `src/config.rs`, `src/ui_bridge.rs`. Checks: fmt, clippy `-D warnings` and tests for `nemo-ui` and `nemo-app`. _Requirements: 9, 13, 15_
- [x] 2. See-through window: transparent window, application style, backdrop and pane alpha hooks, alpha floors and contrast tests, snapshot proof over a mock wallpaper. Files: `nemo-ui`: `src/gui/shell.rs`, `src/gui/mod.rs`, `src/gui/app.rs`, `src/gui/views/backdrop.rs`, `src/gui/views/kit.rs`, `src/gui/views/shell.rs`, `src/gui/views/shell/nav.rs`, `src/scheme.rs`, `src/gui/headless/glass.rs` (new), `examples/snapshots/glass.rs` (new). Checks: as task 1; snapshots `glass-*`. _Requirements: 10, 11, 12, 13, 15, 17_
- [ ] 3. Appearance page: live preview, grouped cards, aligned controls, Window glass row with the restart note and the state of the running window, snapshots in dark, AMOLED and light. Files: `nemo-ui`: `src/gui/views/settings/appearance.rs` and new files beside it, `src/copy.rs`. Checks: as task 1; snapshots `appearance-*`. _Requirements: 1, 2, 3, 4, 5, 6, 7, 8, 14, 16_

## Definition of Done
See `.kiro/steering/workflow.md` section 4.
