# Sharp covers: tasks

Spec approved by the owner on 2026-11-15.

## Tasks

- [x] 1. Fetch and bake cover art at slot size times the window scale. _Requirements: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10_
  - Crate and files: `nemo-ytm` (`src/images.rs`, `src/images/variant.rs`, `src/images/tests.rs`); `nemo-ui` (`src/gui/app/{art,query,job,dispatch,message,handle,update}.rs`, `src/gui/app.rs`, `src/gui/mod.rs`, `src/gui/headless.rs`, `src/cover.rs`, `src/cover/letterbox.rs`, `src/mock/cover.rs`).
  - Does: read the window scale factor, size steps, Google size-suffix rewrite, `i.ytimg.com` variants with 404 fallback and miss markers, letterbox cut, Lanczos3 shrink on the worker.
  - Checks: fmt; `cargo clippy -p nemo-ytm -p nemo-ui -p nemo-app --all-targets --all-features -- -D warnings`; `cargo test -p nemo-ytm -p nemo-ui -p nemo-app --all-features`.
  - Steering: design.md sections 9 and 15.

- [x] 2. Owner follow-ups (2026-11-15): native-size centred drawing for a source smaller than its slot, and a one-time prune of old cache files. _Requirements: 11, 12_
  - Crate and files: `nemo-ytm` (`src/images.rs`, `src/images/prune.rs`); `nemo-ui` (`src/gui/app/art.rs`, `src/gui/app.rs`, `src/gui/views/kit.rs`, `src/gui/views/bar/cover.rs`).
  - Does: `art_fit` and `fitted_art`; `v2-` cache names, `format-v2` marker, `prune_old_files` on the worker.
  - Checks: as task 1.
  - Steering: design.md sections 9 and 15.

## Definition of Done

See [workflow.md section 4](../../steering/workflow.md#4-definition-of-done).
