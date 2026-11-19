# Brand intro, rebuilt setup and folder picker: tasks

Owner approved the request 2026-11-19.

## Tasks

- [x] 1. Brand intro (`feat/nemo-ui-brand-intro`). Files: `splash.rs`, `splash/intro.rs`, `motion/response.rs`, `gui/views/splash.rs`, `gui/shell.rs`, `examples/snapshots/startup.rs`, design.md sections 8 and 16. Does: the mark, glyph, bloom, wordmark, tagline, exit, floor of 1.2 s, reduced motion. _Requirements: 1, 2, 3, 4, 5, 6, 7, 8_
- [x] 2. Setup stage and layout (`feat/nemo-ui-setup-stage`, stacked on 1). Files: `gui/views/setup.rs`, `setup/*`, `copy/setup.rs`, `examples/snapshots/setup.rs`. Does: brand panel, stepper, content card, narrow dots, two-column Look and Sources, fit at 960 x 600. _Requirements: 9, 10, 11, 15_
- [ ] 3. Setup motion (`feat/nemo-ui-setup-motion`, stacked on 2). Files: `setup/motion.rs`, `gui/views/setup*`, `gui/app/setup.rs`. Does: slide, row stagger, marker morph and check, button states, Done settle, reduced motion. _Requirements: 12, 13, 14, 15_
- [ ] 4. Folder picker (`feat/nemo-app-folder-picker`, on main). Files: `nemo-app` `Cargo.toml`, `ui_bridge.rs`, a small picker module; `startup-flow` task 6 ticked. Does: `Add a folder` through `rfd`, cancelled case, count-only logs. _Requirements: 16, 17, 18_

## Definition of Done

See section 4 of `.kiro/steering/workflow.md`.
