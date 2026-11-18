# Startup flow: tasks

Owner approved the request 2026-11-18.

## Tasks

- [x] 1. Splash (`feat/nemo-ui-startup-splash`). Files: `nemo-ui` `splash.rs`, `gui/shell.rs`, `gui/views/splash.rs`, `gui/mod.rs`; `nemo-app` `main.rs`, `backend.rs`, `ui_bridge.rs`; snapshots `startup.rs`. Does: window and splash at once, services built on the `startup` thread with real steps, 500 ms floor, fade over `motion.base`, failure sentence. _Requirements: 1, 2, 3, 4, 5, 6, 7_
- [x] 2. Setup state and setting (`feat/nemo-ui-startup-setup-state`, stacked on 1). Files: `settings.rs`, `state.rs`, `setup.rs`, `nemo-app` `config.rs`, `ui_bridge.rs`. Does: `setup_completed`, first-run versus existing user, wizard state machine (steps, Back, Next, Skip, notice gating). _Requirements: 8, 9, 14, 16, 17_
- [ ] 3. Setup screens (`feat/nemo-ui-startup-setup-screens`, stacked on 2). Files: `gui/views/setup*`, `gui/app/*`, `copy.rs`, Settings Info. Does: the six steps, live look, sources, playback, notice, Done, `Run setup again`; snapshots of every step. _Requirements: 9 to 16, 18_
- [ ] 4. TODO (needs `song-colour-theme` on main): the two song-colour switches in the Look step.

## Definition of Done

See section 4 of `.kiro/steering/workflow.md`.
