# Skeleton loading: design

## Overview

Three small parts.

1. `nemo-ui/src/skeleton.rs` (pure, no iced): a `Gate` per region says whether a skeleton is wanted and drives the timing (150 ms delay, 300 ms hold, 200 ms cross-fade), and `Clock` is the one shimmer phase (1.4 s loop). `Skeletons` holds one gate per `Region` plus the clock. Time is passed in, never read.
2. `gui/views/kit/skeleton.rs` (iced): one public concept, `Skeleton`, a copy of the current look (phase, opacity, animate) with builders for blocks, circles, text lines and the ready-made shapes (`card`, `row`, `hero`, `chip_row`, `section_header`, `top_result`, `list_item`). Blocks are a small widget that draws one rounded quad with a gradient; the band position comes from the viewport, so one band crosses the whole page.
3. Wiring in `App`: `sync_skeletons` runs after every update and sets each gate from the state (`wanted = loading && no cached data && region on screen`); `on_frame` advances them; `needs_frames` asks for frames only while a gate needs them.

Loading, empty and error are told apart by existing state, plus two small additive helpers:

| Surface | Loading | Cached data | Error | Empty |
|---|---|---|---|---|
| Search All | `query` not empty, `results` none, `error` none | `results` | `error` | `results.is_empty()` |
| Search chip | `filter` set, `filtered_now()` none | `filtered_now()` | `Filtered.result` is `Err` | `count == 0` |
| Home | YouTube on and (`home.phase != Done` or `refresh.busy()`) | shelves, daily sections, recent, local albums | none | existing "Nothing here yet" |
| Explore | `explore.phase() == Loading`, `data()` none | `data()` | `shows_fallback()` | n/a |
| Library, sidebar | `App::library_loading()` (new): signed in, `account_lists()` none, not `library_failed` | `account_lists()` | `library_failed` | `Some` with no rows |
| Detail | `details.get(key)` none or `Loading` | `Ready` | `Failed` | `Ready` with no tracks |

## Affected Crates

- `nemo-ui` only. Nothing in `nemo-audio`, nothing in the public API of `nemo-core`.

## Data Flow

UI thread only. `update` -> `after_update` -> `sync_skeletons` sets `want` on each gate. `Message::Frame` -> `on_frame` -> `skeletons.advance(dt)`. `view` reads `app.skeletons.show(region)` (`Content`, `Skeleton`, `Fade(opacity)`) and `app.skeletons.look(region)`. No thread, channel or audio buffer is involved.

Frames: `Skeletons::needs_frames(reduced)` is true while a gate waits (150 ms), holds (up to 300 ms), fades (200 ms), or shows with motion on (the shimmer). `App::needs_frames` already requires focus; a region is wanted only while its page is on screen (route match; sidebar not hidden by full screen or the mini player). With Reduce motion a shown skeleton is still: no frames.

## Interfaces

```rust
// nemo-ui/src/skeleton.rs
pub const DELAY_MS: f32 = 150.0;
pub const HOLD_MS: f32 = 300.0;
pub const SHIMMER_MS: f32 = 1400.0;
pub enum Region { Search, Home, Explore, Library, Sidebar, Detail }
pub enum Show { Content, Skeleton, Fade(f32) }
pub struct Gate { /* stage, ms */ }
impl Gate { pub fn set(&mut self, wanted: bool, reduced: bool); pub fn advance(&mut self, dt_ms: f32, reduced: bool); pub fn show(&self) -> Show; pub fn needs_frames(&self, reduced: bool) -> bool; }
pub struct Skeletons { /* gates, clock */ }
impl Skeletons { pub fn set(&mut self, r: Region, wanted: bool, reduced: bool); pub fn advance(&mut self, dt_ms: f32, reduced: bool); pub fn show(&self, r: Region) -> Show; pub fn phase(&self) -> f32; pub fn needs_frames(&self, reduced: bool) -> bool; }

// nemo-ui/src/gui/views/kit/skeleton.rs
pub struct Skeleton { /* phase, opacity, animate */ }
impl Skeleton { pub fn block(..); pub fn circle(..); pub fn line(..); pub fn card(..); pub fn row(..); pub fn hero(..); pub fn chip_row(..); pub fn section_header(..); pub fn top_result(..); pub fn list_item(..); }
// nemo-ui/src/gui/app
impl App { pub fn library_loading(&self) -> bool; }
```

## Error Handling

A skeleton is never an error state. A failed or empty load turns `wanted` off, so the gate holds the skeleton to 300 ms at most and fades into the existing sentence or empty state. A stale answer cannot leave a skeleton on screen: the gate follows the state every update, and a page that is left (route change) drops its gate to `Off` at once.

## Testing Strategy

- Pure tests in `skeleton.rs`: no skeleton before 150 ms, fast answer never shows it, 300 ms hold, 200 ms fade, re-want during the fade, reduced motion (no fade, still), `needs_frames` per stage, shimmer phase loop and wrap.
- App tests (`frame_tests.rs` style): skeleton only with no cached data; hidden on data, error, empty; 0 frames when nothing is visible, when the window is unfocused and when Reduce motion is on.
- Layout tests: each shape has the same outer size as the real shape. Shared constants are asserted in plain tests; the measured bounds of skeleton and real element at the same width are asserted in a headless test (`ICED_TEST_BACKEND=tiny-skia --ignored`, like the other renderer tests).
- Snapshots (`examples/snapshots/skeleton.rs`): Search pending, Home first load, Explore first load, Playlist page loading, sidebar loading, dark and light, 960 x 600 and 1366 x 697, plus one mid-shimmer frame. No audio is touched, so no bit-exact test.

## Risks and Alternatives

- The delay needs a clock. The frame ticker is the only one; the 150 ms wait costs at most 9 frames. A separate timer thread would add a second clock; rejected.
- A true cross-fade needs group opacity, which iced lacks. The content fades in from the pane colour with the existing `Fx` entrance (no slide) while the skeleton fades out on top: continuous at both ends.
- One page-wide band needs absolute x. The block widget reads the viewport of its scroll area, which is stable for a vertical scroller.
- The old still skeleton of the detail pages (`skeleton_block`, `SKELETON_PHASE`) is replaced by the kit, so there is one skeleton.
