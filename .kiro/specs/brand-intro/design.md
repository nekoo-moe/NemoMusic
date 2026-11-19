# Brand intro, rebuilt setup and folder picker: design

## Overview

Three parts, one PR each (the setup has two).

1. Brand intro. The timeline is a pure function of the time since the first frame (`nemo-ui/src/splash/intro.rs`), so tests and snapshots ask for any moment and nothing is stored. The splash view (`gui/views/splash.rs`) draws that moment with iced containers, text, the note icon and one radial texture; the shell keeps the existing frame subscription, which runs only while the splash is visible and has not failed. The mark scales with the closed form of an under-damped spring (`motion/response.rs`, stiffness 380, damping ratio 0.6, an overshoot of about 9.5 %); every other move is `emphasized_decelerate` over `motion.base`, as the exit is `emphasized_accelerate`.
2. Setup layout (PR 2). `gui/views/setup.rs` becomes a two-panel stage: a brand panel (mark, step title in `font.display`, vertical stepper) and a content card holding the step. Below 1100 px the brand panel shrinks to the mark and the stepper becomes a row of dots. The step bodies in `steps.rs`, `notice.rs` and `sources.rs` keep their logic and become two-column where a step has more than four rows (Look, Sources) so each fits 960 x 600.
3. Setup motion (PR 3). A pure timeline (`setup/motion.rs`) maps the step transition progress (already in `App::setup_progress`) to the slide, the per-row stagger, the marker morph and check draw, and the one-time Done settle. Hover and pressed states of the setup buttons use the existing `animated` button style.
4. Folder picker (PR 4). `nemo-app` builds the `on_add_music_folder` callback with `rfd::AsyncFileDialog::pick_folder` on the existing async runtime, and sends the chosen path to the library folder API through the same channel the settings use.

## Affected Crates

- `nemo-ui` (splash, setup views, motion helpers, snapshots)
- `nemo-app` (`UiOptions.on_add_music_folder`, the `rfd` dependency)

`nemo-core` public API and `nemo-audio` are not touched.

## Data Flow

Everything runs on the UI thread except the folder dialog.

- Intro: `Shell` (frame ticks, `Instant`) holds `Splash` (steps, floor, fade) and `bar` (progress spring). `splash_layer` builds `SplashLook { t_ms, reduced, exit, bloom, .. }` and the view asks `intro::Frame::at(t_ms, reduced)` for every number it draws. The bloom texture is built once in `Shell::boot` from the idle accent and held as an `image::Handle`, so no frame uploads a texture.
- Setup: unchanged events (`SetupEvent`, `SettingChange`); the view reads `App::setup_progress()` and the setup state. The Done settle starts when the step becomes Done and is stored as a start time in the setup state, so it plays once.
- Folder picker: the callback (`Arc<dyn Fn() + Send + Sync>`) spawns the dialog on the async runtime; the result returns as `Message` (existing library folder message) or nothing when cancelled. No audio buffer is involved.

## Interfaces

```rust
// nemo-ui/src/splash/intro.rs (pure)
pub const MIN_SHOW_MS: u64 = 1_200;
pub const WORDMARK: &str = "NemoMusic";
pub const TAGLINE: &str = "A quiet room for your music.";
pub struct Reveal { pub opacity: f32, pub rise_px: f32 }
pub struct Frame { pub mark_scale: f32, pub mark_opacity: f32, pub glyph: Reveal,
                   pub bloom_opacity: f32, pub bloom_scale: f32, pub tagline: Reveal, pub rail_opacity: f32 }
impl Frame { pub fn at(t_ms: u64, reduced: bool) -> Self; pub fn letter(i: usize, t_ms: u64, reduced: bool) -> Reveal; }
pub fn bloom_rgba(color: Rgba, peak: f32) -> Vec<u8>;
// nemo-ui/src/motion/response.rs
pub fn spring_response(t_s: f32, stiffness: f32, ratio: f32) -> f32;
// nemo-app
UiOptions { on_add_music_folder: Option<Arc<dyn Fn() + Send + Sync>> } // existing field, now set
```

## Error Handling

- A failed start shows the mark and wordmark at rest, the plain sentence and `Close`, with no frames (as before).
- A dialog that cannot open (no display server, missing GTK) shows nothing and logs one warning with no path; the user can still add a folder from the library settings if a text path control exists.
- The bloom texture is a fixed-size buffer; there is no failure path.

## Testing Strategy

- Unit tests of the timeline: the overshoot stays between 5 and 12 %, every number settles to its resting value, letters are 35 ms apart, reduced motion returns the resting frame at t = 0, the splash floor (1.2 s, 500 ms reduced), the exit curve.
- Contrast tests: wordmark, tagline and status ink against the surface in the three base modes (reuse the palette contrast helper).
- Shell tests (existing) updated for the new floor.
- Snapshots (`examples/snapshots/intro.rs`, `setup.rs`): splash at t = 0, 300, 700, 1100 ms and exit in dark and light; every setup step in dark and light at 960 x 600 and 1366 x 697, Done, narrow stepper. Each PNG is opened and judged by eye.
- Folder picker: a fake picker (a function returning `Some(path)` or `None`) tests the wiring and the cancelled case; the real dialog needs a desktop and is not tested here. No audio code is touched, so no bit-exact test.

## Risks and Alternatives

- Letter-by-letter text loses kerning pairs. The wordmark has none that matter at 40 px in Inter Display; checked on the snapshot.
- Re-laying the mark each frame (size changes) costs a few small layouts for about 1 s; it draws no frames afterwards. Alternative: a custom shader widget (more code, more risk).
- A glide of the mark to the sidebar logo needs the window size and the sidebar geometry in the shell; replaced by scale-down and fade (owner's brief allows it).
- `rfd` with `gtk3` needs GTK 3 at build and run time (already required). `xdg-portal` is the fallback if it fails to build.
