# Living backdrop: tasks

Owner pre-approved 2026-11-15 (UI polish wave).

## Tasks
- [x] 1. Window backdrop from the playing cover, slow capped drift, still and flat modes, song wins over the open page, pane alpha 0.66, `design.md` sections 7 and 9 updated (owner override). Crate and files: `nemo-ui`: `src/drift.rs` (new), `src/gui/views/backdrop.rs`, `src/gui/views/backdrop/field.rs` (new), `src/gui/views/backdrop/tests.rs` (new), `src/gui/views/mod.rs`, `src/gui/app/sync.rs`, `src/scheme.rs`, `src/scheme/fade.rs`, `src/scheme/tests.rs`, `src/canvas.rs`, `src/theme.rs`, `src/lib.rs`, `examples/snapshots/backdrop.rs`. Checks: fmt, clippy `-D warnings` and tests for `nemo-ui` and `nemo-app`; snapshots `bg-*`. _Requirements: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10_

## Definition of Done
See `.kiro/steering/workflow.md` section 4.
