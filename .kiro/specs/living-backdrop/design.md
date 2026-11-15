# Living backdrop: design

## Overview

### Root cause of "the background does not change with the song and has no motion"
The pipeline did rebuild the colour on every track change: `state::reduce` emits `Effect::LoadCover` for each new track, the cover worker runs `cover::analyze`, and `App::on_cover` stores the new seed. Three things hid that:
1. **The page colour won.** `App::sync_bg` used the seed of the open album, playlist or artist page before the song's seed (rule from `.kiro/specs/app-mvp/wave-ui-brief.md` item 1, "page colour wins while it is open"). The owner's screenshot shows playlist "Nồi cám heo." open, so the window kept that cover's red while songs changed. Test: `an_open_album_page_does_not_hold_the_window_colour` fails on the old rule.
2. **The field was nearly the same for every cover.** `scheme::bg_colors` used only the hue of the dominant colour, with chroma capped at 0.045 and fixed lightness. Two warm covers gave almost the same field, and it showed only through the 8 px gaps and the panes at 0.74 alpha (26 % of an already faint colour).
3. **It was static by rule** (`design.md` sections 7 and 9).

### What changes
- With the Dynamic theme and a cover, the window field is the cover's toned canvas texture (`canvas::Canvas`, already built on the cover worker from the 96 px pre-blurred cover, sigma 5 texels, about 70 px at 1366 px width). It is drawn twice: straight, and turned half a circle at 0.5 opacity, so two soft colour fields drift against each other like a mesh gradient. A vignette (`surface-container-lowest` up to 0.45 at the bottom) sits on top.
- The window field follows the song on every screen (`scheme::field_seed`). The page colour moves into the page itself (task 2 of the wave).
- Cross-fade: the old cover's texture under the new one, at `anim.cover()` (400 ms, `COVER_FADE_MS`). The gradient fallback field (`BgFade`) now uses the `effects.fast` spring (stiffness 1400), which settles in under 400 ms.
- Pane alpha `scheme::PANE_ALPHA` goes from 0.74 to 0.66, so the colour and the drift show through the panes. 0.66 stays above the 0.60 scrim of `design.md` section 9; contrast is tested over the drifting field.
- Motion (owner override, 2026-11-15): each copy moves on a figure-eight path, 3 % of the window's larger side, with a scale of 1.12 plus or minus 0.02, over a 48 s cycle. The layers run opposite ways, a third of a cycle apart. One frame moves a layer by under 1 px at 1920 px width.
- Frame cap: 24 fps (`theme::motion::AMBIENT_FPS`, compile-time check 20 to 30).
- Still under reduced motion, while unfocused, paused or stopped, under the full-screen Now Playing, and on the CPU renderer (tiny-skia). Flat tonal colour (the canvas middle stop) under reduce transparency.
- Fixed themes (Ocean, Sakura, ...) and songs with no cover keep the gradient field.

## Affected Crates
- `nemo-ui` only.

## Data Flow
- Cover worker (task thread): `cover::analyze` decodes, crops, shrinks to 96 px, blurs once (`imageops::blur`), and builds the canvas textures for the three base modes. Unchanged; it already ran per track.
- UI thread: `App::on_cover` keeps the new `CoverView` and the previous one, restarts the 400 ms cover fade and rebuilds the accent palette.
- `views::backdrop::window_field` builds a `Field` widget with the texture layers and the drift inputs (`drift::Inputs`).
- The `Field` widget keeps a `drift::Clock` and the window focus in its widget state. On each `RedrawRequested` it advances the clock and, while moving, asks for the next redraw at `now + 1/24 s` (`Shell::request_redraw_at`). No message is published, so `App::update` and `App::view` do not run for the drift. It draws each layer with `draw_image` into a square placed by `drift::placed` (one textured quad per layer; no blur at draw time).
- No audio buffers are involved. Nothing reads or waits on the audio thread.

## Interfaces
```rust
// crates/nemo-ui/src/drift.rs (pure)
pub enum Mode { Moving, Still, Flat }
pub struct Inputs { pub reduce_motion: bool, pub reduce_transparency: bool, pub focused: bool,
                    pub playing: bool, pub visible: bool, pub software: bool }
impl Mode { pub const fn of(i: Inputs) -> Self; }
pub fn frame_interval() -> Duration;
pub struct Clock { pub elapsed_ms: f64, /* last redraw */ }
impl Clock { pub fn tick(&mut self, now: Instant, moving: bool) -> Option<Instant>; }
pub struct Pose { pub dx: f32, pub dy: f32, pub scale: f32 }
pub fn pose(elapsed_ms: f64, layer: u8) -> Pose;
pub fn placed(bounds: (f32, f32, f32, f32), pose: Pose) -> (f32, f32, f32);
pub const SHIFT: f32; pub const BASE_SCALE: f32; pub const BREATHE: f32;
pub const BLOB_ALPHA: f32; pub const VIGNETTE_ALPHA: f32; pub const MAX_STEP_MS: f64;

// crates/nemo-ui/src/theme.rs, module motion
pub const AMBIENT_CYCLE_MS: u32; pub const AMBIENT_FPS: u32;

// crates/nemo-ui/src/scheme.rs
pub fn field_seed(choice: ThemeChoice, cover: Option<Seed>) -> Option<Seed>;
pub const PANE_ALPHA: f32; // 0.66

// crates/nemo-ui/src/gui/views/backdrop.rs
pub fn window_inputs(app: &App) -> Inputs;
pub fn window_field(app: &App) -> Element<'_, Message>;
pub struct Field; pub struct Layer { pub handle: image::Handle, pub opacity: f32 }
impl Field { pub fn new(layers: Vec<Layer>, inputs: Inputs) -> Self; }
```

## Error Handling
- No new failure paths. A cover that fails to decode leaves `App::cover` empty, and the gradient field shows (as before).
- A stalled frame adds at most 100 ms to the drift clock (`MAX_STEP_MS`), so the drift never jumps.

## Testing Strategy
- `drift::tests`: mode switch (reduced motion, unfocused, paused, hidden, CPU renderer, reduce transparency); the frame cap (following every request for 10 s gives 240 frames, each at least 1/24 s apart); the clock freezes and resumes without a jump; the path is closed, small and under 1 px per frame; a layer always covers the window at every pose and window size.
- `canvas::tests::every_ink_reaches_4_5_over_the_drifting_window_backdrop`: every ink over pane-over-(second copy at 0.5 over the first, with and without the vignette), and in the gaps, for the fixture covers in all base modes.
- `scheme::tests`: `field_seed` follows the cover and keeps a fixed theme's seed; the fallback field fade ends within 400 ms.
- `gui::views::backdrop::tests` (mock backend): a track change builds the backdrop from the new cover and keeps the old one for the cross-fade; an open album page no longer holds the window colour; the mode follows the settings, pause and the full-screen Now Playing.
- Headless snapshots: `bg-song-a/b/c`, `bg-page-album`, `bg-flat` (reduce transparency), `theme-home-light`.
- Frame cost, measured with a scratch example (not committed) on tiny-skia at 2x scale, 1366 x 697 window: the cover backdrop alone takes about 280 ms per frame against about 110 ms for the gradient field, so the CPU renderer stays still (criterion 8). `drift::pose` plus `placed` take about 40 ns per layer. The GPU path draws 2 textured quads of a 96 px texture (4 during a cross-fade), and a drift frame skips `update` and `view`; its GPU and CPU time on a real window was not measured (no display or GPU here). The owner can read it with the frame readout (F3).
- No audio code changed: no bit-exact test needed.

## Risks and Alternatives
- A software wgpu adapter (llvmpipe) is not detected as the CPU renderer; there the drift runs at 24 fps on the CPU. Mitigation: reduced motion turns it off; a future tier check can add it.
- Widget state resets when the field switches between the gradient and the texture (no cover to a cover); the drift then starts from its first pose. Harmless.
- Alternatives: a custom wgpu shader with a mesh gradient (more code, no tiny-skia snapshots); re-blurring on the CPU per frame (rejected by the brief); a `window::frames()` subscription at 24 fps (would run `update` and `view` for every drift frame).
