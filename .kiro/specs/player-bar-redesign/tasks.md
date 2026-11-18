# Player bar redesign: tasks

Owner approved the requirements on 2026-11-18 by sending the request.

## Tasks

- [ ] 1. Spec and layout rules (`feat/nemo-ui-player-bar-spec`). Files: this folder, `.kiro/steering/design.md` (sections 4, 13), `crates/nemo-ui/src/layout.rs`, `crates/nemo-ui/src/layout/bar.rs`. Does: capsule width, zones and collapse step as pure functions. Checks: layout unit tests. _Requirements: 1, 6_
- [ ] 2. Capsule (`feat/nemo-ui-player-bar-capsule`, stacked on 1). Files: `gui/views/bar.rs`, `bar/*.rs`, `gui/views/shell.rs`, `gui/app/{message,handle}.rs`, `gui/app.rs`, `layout.rs` constants, `examples/snapshots/player_bar.rs`. Does: floating capsule, glass Low and Off surfaces, three zones, play morph, right zone steps, mute, more button, snapshots. Checks: unit tests, snapshots, idle frame tests. _Requirements: 1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12_
- [ ] 3. Seek slider (`feat/nemo-ui-player-bar-seek`, stacked on 2). Files: `bar/seek.rs`, `bar.rs`, snapshots. Does: custom seek widget with growing track, thumb on hover or focus, drag tooltip, times. Checks: pointer to fraction tests, hover and drag snapshots. _Requirements: 5, 9, 10_

## Definition of Done

See section 4 of `.kiro/steering/workflow.md`.
