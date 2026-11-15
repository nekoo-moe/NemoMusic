Spec approved by the owner on 2026-11-15.

## Tasks

- [x] 1. Reload the music data after a sign-in, an account switch or a background sign-in. _Requirements: 1, 2, 3, 5, 6, 7, 8, 11, 12_
  - Crate and files: `nemo-ui` (`src/refresh.rs`, `src/lib.rs`, `src/gui/app.rs`, `src/gui/app/{refresh.rs,refresh/tests.rs,dispatch.rs,handle.rs,job.rs,message.rs,update.rs}`).
  - Does: the pure `Refresh` state machine; the account watch after updates and frames; the two sequential reload steps; late start-up answers dropped; a failure keeps the data and shows one notice.
  - Checks: `cargo fmt --all -- --check`; `cargo clippy -p nemo-ui -p nemo-app --all-targets --all-features -- -D warnings`; `cargo test -p nemo-ui -p nemo-app --all-features`.
  - Steering: code.md (errors, tests), design.md section 8 (calm).
- [ ] 2. Add the Refresh control, `F5` and `Ctrl+R`. _Requirements: 4, 5, 7, 9, 10, 12_
  - Crate and files: `nemo-ui` (`src/refresh.rs`, `src/keys.rs`, `src/explore.rs`, `src/state/reduce.rs`, `src/gui/app/{refresh.rs,refresh/tests.rs,handle.rs,message.rs,subscribe.rs}`, `src/gui/assets.rs`, `assets/icons/refresh.svg`, `src/gui/views/shell/topbar.rs`, `src/gui/views/panels.rs`); `docs/ui/snapshots/{home-signed-in,shortcuts}-1366x697.png`.
  - Does: a 40 px icon button in the top bar before Settings, tooltip `Refresh (F5)`, dimmed and inert with `Refreshing your music…` while busy; the keys; the shortcut sheet row (row gap removed so the sheet still fits 697 px).
  - Checks: as task 1, plus the headless snapshots.
  - Steering: design.md sections 5, 8, 13 and 15.

## Definition of Done

See [workflow.md section 4](../../steering/workflow.md#4-definition-of-done).
