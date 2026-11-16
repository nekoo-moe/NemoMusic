# Liner-style Desktop UI Refinement: Design

Follows `.kiro/steering/design.md` and `.kiro/steering/workflow.md`.

## Overview

This specification details three UI refinements in `nemo-ui`:
1. **Synced Lyrics Refinement:** Stripping accent glow copies from `crates/nemo-ui/src/gui/views/lyrics.rs`, ensuring active lines render with non-bold font (`display_font(false)`) in crisp white (`#FFFFFF`), while upcoming lines use neutral grey (`on-surface-variant`). Syllable lyrics remain unaffected.
2. **Docked Horizontal Player Bar (Liner Desktop style):** Replacing the floating pill capsule in `crates/nemo-ui/src/gui/views/bar.rs` and `crates/nemo-ui/src/gui/views/shell.rs` with a docked, full-width bottom bar (height 76–80 px) anchored to the window bottom edge, featuring a clean top border (`outline-variant`) and balanced 3-zone layout (left: track & like, center: transport & scrubber, right: actions & volume).
3. **Ambient Artwork Background:** Retuning `crates/nemo-ui/src/canvas.rs` and `crates/nemo-ui/src/gui/views/backdrop.rs` to generate a smooth, deep radial/mesh ambient illumination derived from album artwork, blending subtly into deep dark surfaces without overpowering content or violating contrast.

## Affected Crates

- `nemo-ui`:
  - `src/gui/views/lyrics.rs`: update line-synced lyrics text run, remove `glow_layers`, set font weight normal and monochrome colors.
  - `src/gui/views/bar.rs`: redesign player bar from floating capsule to docked full-width bar.
  - `src/gui/views/shell.rs`: update bottom layer anchoring from inset floating capsule to bottom-docked bar; update pane padding.
  - `src/layout.rs`: update bottom reserve layout constants (docked bar height without capsule inset).
  - `src/canvas.rs` & `src/gui/views/backdrop.rs`: retune ambient backdrop lighting and gradients.

## Data Flow

- No changes to audio data flow, real-time audio threads, or `nemo-core` services.
- UI continues to receive state via `PlayerSnapshot` and `App::anim` ticks.
- Cover art worker thread continues generating color palettes and pre-blurred textures via `ImageCache` and `canvas.rs`.

## Interfaces

```rust
// crates/nemo-ui/src/gui/views/bar.rs
/// Render the docked bottom horizontal player bar spanning full window width.
pub fn view<'a>(app: &'a App, width: f32) -> Element<'a, Message>;

// crates/nemo-ui/src/layout.rs
/// Docked bottom player bar height.
pub const DOCKED_BAR_HEIGHT: u32 = 76;
/// Reserve padding for content above docked bar.
pub const DOCKED_BAR_RESERVE: u32 = DOCKED_BAR_HEIGHT;
```

## Error Handling

- Missing album artwork gracefully falls back to neutral dark background gradient (`scheme::bg_colors`).
- Missing or malformed lyrics fall back to `No lyrics found` state as existing.
- Reduced motion / reduced transparency settings are strictly respected, rendering flat tonal backgrounds with zero frame updates when idle.

## Testing Strategy

- `cargo clippy --workspace --all-targets -- -D warnings` must pass.
- `cargo test -p nemo-ui` must pass, including snapshot and layout tests.
- Visual inspection against reference screenshots (Liner Desktop `tryliner/desktop` vs current NemoMusic).

## Risks and Alternatives

- *Risk:* Content clipping near the bottom of scrollable views. *Mitigation:* Ensure `CONTENT_BOTTOM_PADDING` in `layout.rs` matches `DOCKED_BAR_HEIGHT`.
- *Alternative:* Keeping a partial floating bar. *Rejected:* The owner explicitly requested removing the floating glass music bar and adopting Liner Desktop's docked style.
