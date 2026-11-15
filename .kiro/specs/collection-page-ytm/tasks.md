# Collection page in the YouTube Music layout: tasks

Owner pre-approved 2026-11-15 (UI polish wave). Stacked on `living-backdrop` (the window no longer takes the page colour; the page shows it itself).

## Tasks
- [x] 1. Rebuild the album and playlist pages: page gradient, two columns or stacked, centred hero with round Shuffle and white Play, `Artist • Album` rows, skeleton in the same layout; `design.md` section 4 line (owner override of the accent Play). Crate and files: `nemo-ui`: `src/gui/views/detail.rs`, `src/gui/views/detail/collection.rs` (new), `src/gui/views/detail/song_row.rs` (new), `src/scheme.rs`, `src/scheme/tests.rs`, `src/gui/app.rs` (`page_seeds` visibility), `examples/snapshots/backdrop.rs`. Checks: fmt, clippy `-D warnings` and tests for `nemo-ui` and `nemo-app`; snapshots. _Requirements: 1, 2, 3, 4, 5, 6, 7, 8, 9_
- [ ] 2. Sort control over the list, after the owner decides how Play follows a sorted list. _Requirements: none yet (Open Questions)_

## Definition of Done
See `.kiro/steering/workflow.md` section 4.
