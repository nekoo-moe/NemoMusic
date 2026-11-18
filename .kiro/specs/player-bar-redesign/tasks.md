# Player bar redesign: tasks

Owner approved the requirements on 2026-11-18 by sending the request.

## Tasks

- [x] 1. Spec and layout rules (`feat/nemo-ui-player-bar-spec`). Files: this folder, `.kiro/steering/design.md` (sections 4, 7, 13), `crates/nemo-ui/src/layout.rs`, `crates/nemo-ui/src/layout/bar.rs`. Does: capsule width, zones and collapse step as pure functions. Checks: layout unit tests. _Requirements: 1, 6_
- [x] 2. Mute (`feat/nemo-ui-player-bar-mute`, stacked on 1). Files: `gui/app.rs`, `gui/app/{message,handle,mute}.rs`. Does: `Message::ToggleMute` sends `SetVolume(0)` and back. Checks: mute test against the mock player. _Requirements: 7_
- [x] 3. Capsule (`feat/nemo-ui-player-bar-capsule`, stacked on 2). Files: `gui/views/bar.rs`, `bar/*.rs`, `gui/views/mod.rs`, `layout.rs`, `theme/glass.rs`, `examples/snapshots/player_bar.rs`, `docs/ui/snapshots/player-bar-*.png`. Does: floating capsule, glass Low and Off surfaces, three zones, play morph, right zone steps, More button, snapshots. Checks: unit tests, snapshots, idle frame tests. _Requirements: 1, 2, 3, 4, 6, 8, 9, 10, 11, 12_
- [x] 4. Seek slider (`feat/nemo-ui-player-bar-seek`, stacked on 3). Files: `bar/seek.rs`, `bar/centre.rs`, `gui/fx.rs` (one visibility), `gui/headless.rs` (pointer shots), snapshots. Does: custom seek widget with growing track, thumb on hover or focus, drag tooltip, times. Checks: pointer to fraction tests, hover and drag snapshots. _Requirements: 5, 9, 10_

## Definition of Done

See section 4 of `.kiro/steering/workflow.md`.
