Owner pre-approved 2026-11-15. The three tasks stack: 2 builds on 1, 3 on 2.

## Tasks

- [x] 1. Intro and gap note
  - Crate and files: `nemo-ui`: new `src/lyrics_view/note.rs` (+ tests), `src/lyrics_view.rs` (`scroll_target_focus`), `src/state.rs` (`lyric_focus`), `src/state/reduce.rs`, `src/gui/app/sync.rs`, `src/gui/views/lyrics.rs` with new `lyrics/note.rs` and `lyrics/three.rs`, `src/gui/views/now_playing.rs`, `src/gui/views/np_classic.rs`, `examples/snapshots/lyrics_polish.rs`.
  - Does: The note before the first line and in breaks of 5 s or more, filled by position, full at the line start, fading with `motion.fast`; in the right pane, full screen, compact layout and Classic.
  - _Requirements: 1, 2, 3, 4, 5, 6, 7_
  - Branch: `fix/nemo-ui-lyrics-intro-note`
  - Checks: unit tests of `(show, fill)` at the intro, in a gap, in a short gap and at the exact line start; `lyrics-*` snapshots.

- [ ] 2. Refined full-screen lyrics and controls
  - Crate and files: `nemo-ui`: `src/gui/views/now_playing.rs` and new `now_playing/` modules, `src/lyrics_view.rs`, `src/gui/views/lyrics.rs`, the state for the Timing popover.
  - Does: Quiet icon-only controls and a neutral play button, a thin progress bar with tabular times, the Timing popover, a quiet quality label, and a lyric column with more air, size and opacity steps and an edge fade; footer, sync state and pill kept.
  - _Requirements: 8, 9, 10, 11, 12, 13_
  - Branch: `fix/nemo-ui-lyrics-fullscreen` (stacked on 1)
  - Checks: unit tests of the line steps and the edge fade; snapshots; `design.md` edit for section 10.

- [ ] 3. Refined lyric font
  - Crate and files: `nemo-ui`: `assets/fonts/`, `src/gui/assets.rs`, `src/gui/headless.rs` font list, the lyric views; `tech.md` "UI type family", `design.md` section 3.
  - Does: A comparison snapshot of Inter, Be Vietnam Pro and Plus Jakarta Sans; the pick bundled in at most two weights with its OFL text; lyrics, the full-screen title and display text use it; line height at least 1.25 for large lyrics.
  - _Requirements: 14, 15, 16, 17_
  - Branch: `fix/nemo-ui-lyrics-font` (stacked on 2)
  - Checks: glyph coverage test for the Vietnamese strings; comparison PNG; snapshots.

## Definition of Done

See `.kiro/steering/workflow.md` section 4.
