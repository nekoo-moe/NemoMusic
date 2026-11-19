# Backdrop calm: design

## Overview
Cause of the flashing: the heartbeat of `song-colour-theme` drew a double pulse ("lub-dub", 1.6 s) that laid up to 5 % white over the whole backdrop and scaled the fields by 2.5 %. Over the dark fields (lightness about 0.14 to 0.30) that is a rise of about 5 points of CIE L* in 160 ms, twice per 1.6 s, then a pause: "blink blink, then repeat". It runs only while a song plays (the drift clock stops when paused) and not under Reduce transparency (flat colour, no fields), which matches the report.

Rejected as the cause, by reading the code and by tests: new image handles each frame (the handles are built once per cover in `MeshView::new` and cloned, same id), the bloom or the scheme cross-fade restarting on position ticks (`Anim::restart_cover` runs only from `on_cover` for a new track), `needs_frames` toggling (it stays true for the whole song), layered alpha of the settled layer (one layer at opacity 1).

Two frame sources did exist: the app's frame ticker (`Message::Frame`, 16.7 ms, runs while a song plays) and the backdrop widget's own 24 fps redraw requests. The clock reads the redraw time, so it never jumped, but the second source had no purpose. The widget now has no timer.

The new heartbeat is a breath: `heartbeat::breath` (raised cosine, 2.6 s) drives `heartbeat::nudge` (growth 2 %, centre shift up to 3 % of the distance to the window centre). No colour changes.

Player bar (task 3): the capsule body becomes `surface-container` at alpha 0.50 over the backdrop with a vertical inner gradient, a white 0.22 rim and a brighter glint on the top edge.

## Affected Crates
`nemo-ui` only. Nothing in `nemo-audio`, nothing in the public API of `nemo-core`.

## Data Flow
UI thread only. The frame ticker (`App::needs_frames` includes `backdrop::moves`) sends `Message::Frame`; each redraw reaches `Field::update`, which ticks `drift::Clock`; `Field::draw` asks `drift::field_rect` for each field rectangle. No audio thread, no channel added.

## Interfaces
```rust
// heartbeat.rs
pub fn breath(elapsed_ms: f64) -> f32;
pub fn nudge(elapsed_ms: f64, ax: f32, ay: f32) -> (f32, f32, f32);
// drift.rs
pub fn field_rect(bounds: (f32, f32, f32, f32), i: usize, elapsed_ms: f64, layer_scale: f32, breathing: bool) -> (f32, f32, f32);
impl Clock { pub fn tick(&mut self, now: Instant, moving: bool); pub fn stop(&mut self); }
// backdrop.rs
pub fn moves(app: &App) -> bool;
```
Removed: `heartbeat::{beat, active, LIFT, LIGHT}`, `drift::frame_interval`, `theme::motion::AMBIENT_FPS`.

## Error Handling
None new. A stalled frame adds at most `drift::MAX_STEP_MS` to the clock.

## Testing Strategy
`gui/views/backdrop/calm_tests.rs`: a model of the composed backdrop (the real field textures at the rectangles `field_rect` gives) gives the mean L*; sequence tests step the clock for 600 frames at 24 and 60 fps with +-35 % frame-time jitter; a test reproduces the old lift and shows it swings the window by about 5 points and 2 points in one frame; handle ids are compared across two builds; the subscription count differs by exactly one between playing and paused; the bloom is checked across 300 position ticks and a song change. No audio code changed: no bit-exact test.

## Risks and Alternatives
- Not seen on a real display. Alternative kept in reserve: a brightness-neutral breath (scale only, no shift).
- Alternative rejected: keep a 24 fps cap in the widget. It saves nothing: the ticker already redraws the whole window at 60 fps while a song plays.
