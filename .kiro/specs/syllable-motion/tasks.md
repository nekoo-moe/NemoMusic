Spec pre-approved by the owner on 2026-11-16.

## Tasks

- [x] 1. Grey-to-bright fill, word lift, glow and held scale, line falloff, tests and snapshot. _Requirements: 1, 2, 3, 4, 5, 6, 7_
  - Crate and files: `nemo-ui` (`src/lyric_pieces.rs`, `src/lyrics_view.rs`, `src/gui/syllable_text.rs`, `src/gui/views/lyrics.rs`, `src/gui/views/lyrics/three.rs`, `src/gui/headless.rs`, `examples/snapshots/syllable_motion.rs`); `.kiro/steering/design.md` (section 10 proposal).
  - Checks: `cargo fmt --all -- --check`; `cargo clippy -p nemo-ui -p nemo-app --all-targets --all-features -- -D warnings`; `cargo test -p nemo-ui -p nemo-app --all-features`.

## Definition of Done

See [workflow.md section 4](../../steering/workflow.md#4-definition-of-done).
