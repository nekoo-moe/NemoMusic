# App motion: tasks

Owner pre-approved 2026-11-15.

## Tasks

- [x] 1. Motion core (`feat/nemo-ui-motion-core`). Crate and files: `nemo-ui` `motion.rs`,
  `motion/presence.rs`, `gui/app/sync.rs`, `gui/app/frame_tests.rs`; `nemo-app`
  `os_motion.rs`, `ui_bridge.rs`. Does: `Presence` tween, `emphasized_accelerate`, spring rest
  rule, reduce-motion targets reached in the same update, OS reduce-motion read on Linux.
  Checks: presence and spring tests, app frame-idle tests, parser test. Steering: design.md 8.
  _Requirements: 1, 9, 10, 12_
- [x] 2. Surfaces (`feat/nemo-ui-motion-surfaces`, stacked on 1). Files: `gui/app/surfaces.rs`,
  `gui/views/{mod.rs,overlays.rs,panels.rs,shell.rs,now_playing*}`, `gui/style.rs`. Does:
  `Presence` for toast, track menu, signal popover, Timing popover, tools panel, create and
  shortcut sheets, first-run notice; full-window Now Playing cover grow; capsule rise 16 px;
  overlay pane close; tooltip fade. Checks: tests that each surface enters, leaves and settles;
  snapshots of a mid-enter frame. _Requirements: 2, 3, 9, 10, 11_
- [x] 3. Details (`feat/nemo-ui-motion-details`, stacked on 2). Files: `gui/views/{pane.rs,
  shell/nav.rs,search*,bar*,kit*}`, `gui/app/*`. Does: sliding pills (tabs, nav, search chips),
  150 ms picture fade, still skeleton, title cross-fade, heart morph, menu hold. Checks: tests of
  pill targets, fade timing and that the skeleton asks for no frames once faded in.
  Done in `gui/app/micro.rs`. Not done: the compact tabs of the classic Now Playing style
  (`np_classic.rs`) still swap their fill; the skeleton fades in with the page entrance (its
  block colours live in `detail.rs`, so a skeleton shown on an already settled page appears
  at once). The title fades in on a track change; the old title does not fade out.
  _Requirements: 4, 5, 6, 7, 8, 9, 10, 11_

Follow-up (not in this spec): the collection page (`gui/views/detail.rs`, its sort control
and header) belongs to another agent; it gets the shared row, card and picture motion without
edits. If it later needs its own hook (for example a sort-change fade), add it there.

## Definition of Done

See `.kiro/steering/workflow.md` section 4.
