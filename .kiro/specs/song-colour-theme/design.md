# Song colour theme: design

## Overview
Builds on `living-backdrop` (pre-blurred cover, drift, stop rules, 24 fps cap).

### Rule table (one place: `scheme::song_seed`, `scheme::accent_palette`, `scheme::window_neutrals`, `backdrop::window_field`)
| `song_colour_theme` | Cover known | Background field | Panes (`surface-container`) | Accent roles |
|---|---|---|---|---|
| On | yes | song mesh (drift, heartbeat, bloom) | song pane tint (tone 8 dark, 4 AMOLED, 94 light) | song seed, over the theme |
| On | no | one flat tonal colour from the theme | neutral | theme seed, else fallback |
| Off | any | one flat tonal colour from the theme | neutral | theme seed, else fallback (`Dynamic` has no seed) |
Reduce transparency: the song field becomes one flat tonal colour from the cover (unchanged).

### Mesh (task 2)
The cover worker picks three to four dominant colours (hue buckets of 30 degrees, centres at least 40 degrees apart, weighted by count and chroma; a grey cover gives greys; a one-hue cover gives lightness and slight hue variants). Each colour becomes one soft radial blob texture per base mode (64 px, gaussian alpha), toned to a lightness band that keeps 4.5:1 for every ink over the panes and the gaps, with chroma up to 0.16 (accent roles stay at 0.14). The window draws a flat base, the blobs at fixed anchors drifting on the living-backdrop path, and the vignette.

### Heartbeat and bloom (task 3)
`drift::beat(ms)` is a double pulse over `HEART_CYCLE_MS` 1600 (two raised-cosine bumps at 0 and 0.28 of the cycle, the second at 0.7 height). The pulse scales the blobs by up to 2.5 % and adds up to 5 % white over the field; the widget advances its heart clock beside the drift clock and shares its redraw requests (24 fps). `Anim::bloom` runs 600 ms with `emphasized-decelerate`: new blobs fade in and settle from 1.04 to 1.0 while the old ones fade out; reduced motion swaps at once.

## Affected Crates
- `nemo-ui` (settings, scheme, cover worker, backdrop, drift, anim, Settings view), `nemo-app` (config, bridge).

## Data Flow
Cover worker: `cover::analyze` builds the mesh with the canvas. UI thread: `App::on_cover` keeps it with the previous one, `sync_theme` retargets the accent, the pane tint and the cross-fade. The `Field` widget paces its own redraws; no message for the pulse. Nothing touches the audio thread.

## Interfaces
```rust
// settings.rs
UiSettings { song_colour_theme: bool, song_heartbeat: bool }
SettingChange::{SongColourTheme(bool), SongHeartbeat(bool)}
UiSettings::heartbeat_available(&self, reduced_motion: bool) -> bool
// scheme.rs
pub fn song_seed(song_colour_theme: bool, cover: Option<Seed>) -> Option<Seed>;
pub fn pane_tint(seed: Seed, base: BaseMode) -> Rgba;
pub fn window_neutrals(base: BaseMode, song: Option<Seed>) -> Neutrals;
// nemo-app config.rs: Settings { song_colour_theme: bool, song_heartbeat: bool } (serde default true)
```

## Error Handling
No new failure paths. A cover that fails to decode leaves no cover: the flat theme field shows and the old fields never flash a wrong colour.

## Testing Strategy
Unit tests: the palette rule, the defaults and the old-config migration, the mesh picks and contrast fixtures (peak of the pulse included), the stop rules, the bloom curve and its reduced-motion swap. Snapshots: song A to song B (before, mid, after), three fixture covers, heartbeat peak and rest, dark and light. Frame cost of one drift plus heartbeat frame is measured headless and given in the PR. No audio code changed: no bit-exact test.

## Risks and Alternatives
- The 600 ms bloom and the heartbeat are owner overrides of `design.md` (400 ms cap, one allowed loop).
- Alternative for the pulse: a `window::frames()` subscription (runs `update` and `view` per frame, rejected). Alternative for the mesh: a wgpu shader (more code, no tiny-skia snapshots, rejected).
