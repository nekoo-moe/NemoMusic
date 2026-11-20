## Tasks

- [x] 1. Spec, model without `All`, persistence, nav per area, the control in the sheet and in Settings (PR 1)
  - Crate and files: `nemo-ui` (`workspace.rs`, `settings.rs`, `gui/app/workspace.rs`, sidebar, Library, Search, topbar, headless), `nemo-app` (`config.rs`, `ui_bridge.rs`).
  - Does: criteria 1 to 14.
  - _Requirements: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14_

- [x] 2. `Your accounts` sheet with the SoundCloud card (PR 2)
  - Crate and files: `nemo-ui` (`views/account.rs`, shared SoundCloud account pieces moved out of `views/settings/soundcloud.rs`).
  - Does: criteria 15 to 17.
  - _Requirements: 15, 16, 17_
  - Depends on: 1

- [ ] 3. Caption, transitions, Reduce motion, keyboard, steering lines (PR 3)
  - Crate and files: `nemo-ui` (sidebar brand, `anim.rs`, the control), `design.md` sections 4 and 18, `product.md` Scope line, README if behaviour text changes.
  - Does: criteria 18 to 21.
  - _Requirements: 18, 19, 20, 21_
  - Depends on: 2

## Definition of Done

See section 4 of `.kiro/steering/workflow.md`.
