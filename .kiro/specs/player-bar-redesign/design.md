# Player bar redesign: design

Steering: `design.md` sections 4 to 8 and 13 (shape, glass, fallbacks, motion, keys), `product.md` (Quality badge rule).

## Overview

`gui/views/bar.rs` stops drawing a docked, full-width strip and draws a floating capsule. Pure layout numbers (capsule width, zone widths, collapse step) live in `layout/bar.rs` and are unit tested. The view only reads them. The seek row is a small custom widget (`bar/seek.rs`) so the track can grow and the thumb can fade with the shared springs; the other controls reuse `icon_button`, `tip`, `Fx` and `Presence`.

Capsule width is `min(window_w - 24, 1200)`. Inside: 16 px padding, three zones with 16 px between them.

| Capsule width | Step | Right zone shows |
|---|---|---|
| 1100 and up | Full | pill, lyrics, queue, tools, speaker and slider (80), full-screen |
| 1020 to 1099 | NoSlider | the same without the slider |
| below 1020 | Folded | pill, queue, speaker, `More` (opens the tools panel) |

Left zone width is 30% of the inner width (never under 220). The centre zone takes the rest and is never under 320. At window 960 the capsule is 936 wide (Folded), at 1100 it is 1076 (NoSlider), from 1280 it is 1200 (Full).

Surface: `GlassTier::Low` without reduce transparency draws `surface-container-high`, a left to right gradient of the cover's `strip` colours at 18% over it, a 1 px top highlight (`glass.edge` alpha) and the `glass.shadow`. `Off` or reduce transparency draws `surface-container-high` and a 1 px `outline-variant`. Images cannot be clipped to round corners on the software renderer, so the tint is a gradient, not a texture.

Play button: 56 px disc, `accent.primary` fill, `accent.on_primary` glyph. The play and pause glyphs are drawn together and cross-faded by `anim.play` (the expressive spring, clamped to 0 and 1); the corner radius morph from `anim::play_radius` stays. Buffering swaps the glyph for the spinner.

Seek widget: height 16 px hit area, track 4 px (6 px while hovered or dragged, `spatial.fast`), thumb 14 px fading in with the same value, played part in `accent.primary`, remaining part `on-surface` at 20%. It publishes `Message::SeekDrag(f)` while the pointer is down and `Message::SeekRelease` on release, as the old slider did. A tooltip with the time under the thumb shows while dragging.

## Affected Crates

- `nemo-ui` only. No change to `nemo-core` public API, `nemo-audio` or any `Cargo.toml`.

## Data Flow

UI thread only. The bar reads `App` (player snapshot, position, seek drag, accent, cover, settings) and sends the same `Message` values as before. New: `Message::ToggleMute`, handled in `App::handle`, which sends `PlayerCommand::SetVolume` through the existing path and keeps the last non-zero volume in `App.volume_before_mute`. The audio thread is never touched.

## Interfaces

```rust
// layout/bar.rs
pub const MAX_WIDTH: u32 = 1200;
pub fn width(window_w: u32) -> u32;
pub enum Step { Full, NoSlider, Folded }
pub struct Zones { pub step: Step, pub left: u32, pub centre: u32, pub right: u32 }
pub fn zones(bar_w: u32) -> Zones;
// bar/seek.rs
pub fn seek_bar<'a>(frac: f32, accent: Color, label_at: impl Fn(f32) -> String + 'a) -> Element<'a, Message>;
// gui/app/message.rs
Message::ToggleMute
```

## Error Handling

No new failure modes. A track without a duration shows `-:--` and a slider at zero. A signed-out user pressing the heart gets the sign-in sheet as today. An unknown signal path shows `Unknown`.

## Testing Strategy

- Unit tests in `layout/bar.rs`: capsule width at 960, 1100, 1280, 1920 and 2560; collapse step by width; zones never overlap and the centre is at least 320 for every window width from 960 to 3000.
- Unit tests of the play button state (`Play`, `Pause`, `Buffering` from the snapshot) and of the badge text (YouTube stream never says `Lossless`; buffering text).
- Unit tests of `ToggleMute` (volume goes to 0, comes back) and of the seek fraction from a pointer position.
- Snapshots (`examples/snapshots/player_bar.rs`): playing, paused, buffering, slider hover, 960 and 1280 px, dark, AMOLED, light, light cover, saturated cover, glass Off. Every PNG is opened and checked by eye.
- Idle: the existing frame-idle tests must still pass (a settled bar asks for no frames).
- No audio code changes, so no bit-exact test.

## Risks and Alternatives

- The pre-blurred cover texture would look softer than a gradient, but cannot be clipped to the rounded corners on the software renderer. Gradient chosen; a texture can follow when clipping works.
- Folding into the tools panel reuses an existing popover instead of building a new menu. The panel is wider than a small menu but already holds full-screen, mini player and shortcuts. A dedicated menu would need a new surface and animation; not worth it for this task.
- A custom seek widget costs about 250 lines. The stock `slider` cannot animate track height or show a drag tooltip.
- Real window, real GPU, Windows rendering and 150% scale are not verified here.
