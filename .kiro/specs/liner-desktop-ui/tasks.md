# Liner-style Desktop UI Refinement: Tasks

## Tasks

- [x] 1. **Refine Synced Lyrics (Line-timed) typography and colors**
  - In `crates/nemo-ui/src/gui/views/lyrics.rs`:
    - Remove `glow_layers` (eliminate coral/accent colorful glow behind active line-timed lyrics).
    - Change active text run font weight to regular (`display_font(false)` or clean unbold font).
    - Ensure active line text is rendered in pure white (`on-surface` / `#FFFFFF`) and non-active lines in clean neutral grey (`on-surface-variant`).
    - Verify Syllable Lyrics (`syllable_text.rs`) remain completely untouched.
  - _Requirements: 1_

- [x] 2. **Implement Liner-style docked horizontal player bar**
  - In `crates/nemo-ui/src/gui/views/bar.rs` and `crates/nemo-ui/src/gui/views/shell.rs`:
    - Redesign player bar container from floating pill capsule with 12 px margin/shadow to a docked bottom bar spanning full window width (`Length::Fill`).
    - Set clean background (`surface-container` / `surface-container-low`) with a 1 px top border (`outline-variant`).
    - Reorganize into Liner Desktop layout:
      - Left: Album cover (subtle rounded corners `shape.sm`), title, artist, Like button.
      - Center: Transport controls (Previous, circular Play button, Next) and integrated Scrubber bar.
      - Right: Shuffle, Repeat, Lyrics toggle, Volume slider, Cast/Devices, and Queue toggle.
    - Update `layout.rs` constants (`DOCKED_BAR_HEIGHT`, reserve padding).
  - _Requirements: 2_

- [x] 3. **Enhance ambient background lighting (Liner Desktop style)**
  - In `crates/nemo-ui/src/canvas.rs` and `crates/nemo-ui/src/gui/views/backdrop.rs`:
    - Retune backdrop ambient glow to create a deep, smooth, atmospheric ambient light derived from album artwork.
    - Blend softly into deep base dark surface (`#0a0b0e` / `#0E0F12`), ensuring text contrast >= 4.5:1 across all panels.
    - Validate fallback behavior under reduced motion and reduced transparency.
  - _Requirements: 3_

- [ ] 4. **Run workspace tests and verification**
  - Execute `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test -p nemo-ui`.
  - Verify layout responsiveness across window sizes (compact < 1100 px, standard 1280 px, wide 1600+ px).
  - _Requirements: 1, 2, 3_

## Definition of Done

See [Workflow Section 4](../../steering/workflow.md#4-definition-of-done).
