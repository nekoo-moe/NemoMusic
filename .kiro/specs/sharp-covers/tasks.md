# Sharp covers: tasks

Spec approved by the owner on 2026-11-15.

## Tasks

- [x] 1. Fetch and bake cover art at slot size times the window scale. _Requirements: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10_
  - Crate and files: `nemo-ytm` (`src/images.rs`, `src/images/variant.rs`, `src/images/tests.rs`); `nemo-ui` (`src/gui/app/{art,query,job,dispatch,message,handle,update}.rs`, `src/gui/app.rs`, `src/gui/mod.rs`, `src/gui/headless.rs`, `src/cover.rs`, `src/cover/letterbox.rs`, `src/mock/cover.rs`).
  - Does: read the window scale factor, size steps, Google size-suffix rewrite, `i.ytimg.com` variants with 404 fallback and miss markers, letterbox cut, Lanczos3 shrink on the worker.
  - Checks: fmt; `cargo clippy -p nemo-ytm -p nemo-ui -p nemo-app --all-targets --all-features -- -D warnings`; `cargo test -p nemo-ytm -p nemo-ui -p nemo-app --all-features`.
  - Steering: design.md sections 9 and 15.

## Definition of Done

See [workflow.md section 4](../../steering/workflow.md#4-definition-of-done).
