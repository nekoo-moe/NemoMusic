# Collection page in the YouTube Music layout: design

## Overview
`gui/views/detail.rs` keeps the router, the Artist page and its skeleton. Albums and playlists go to the new `detail/collection.rs` (layout, gradient, hero column, skeleton) and `detail/song_row.rs` (the row). The gradient colour comes from the page seed that the app already computes on the cover worker (`Job::PageSeed`, `App::page_seeds`). Since the living-backdrop spec, that seed no longer colours the window, so the page shows its own colour here.

Before: a 320/240 px cover top-left, the title beside it, a flat pane, rows with an album column. After: the layout below.

```
two columns (W >= 1280, content >= 640)          stacked
+--------------------------------------------+   +----------------------+
| gradient: page colour, darker, fades out   |   | gradient (520 px)    |
|  [ cover L x L ]   | [art] Title     3:30 :|   |     [ cover 240 ]    |
|   Title 28/36      |       Artist • Album  |   |      Title           |
|   Owner 16/24      | [art] ...             |   |      Owner           |
|   Playlist         |                       |   |  Playlist / 12 songs |
|   12 songs • 43 min|  (scrolls on its own) |   |   (Shuffle) (PLAY)   |
|  (Shuffle) (PLAY)  |                       |   | rows ...             |
+--------------------------------------------+   +----------------------+
```

- Left column L = 38 % of (content - 32), clamped to 260..360 and rounded down to 4; list = content - L - 32.
- Gradient: `scheme::page_glow(seed, base)` gives a top colour (dark L 0.36, AMOLED 0.31, light 0.88; chroma at most 0.10, light 0.05) and a middle one (0.27 / 0.21 / 0.92). Stops: top at 0.92 alpha, middle at 0.55 alpha at 45 % of the height, transparent at the bottom. All alphas are multiplied by the page entrance. In two columns it fills the pane behind both columns; stacked, it is 520 px tall and scrolls with the page.
- Play: a 64 px white disc (`on-surface`, `surface` glyph). Owner override of the accent Play, 2026-11-15, as on YouTube Music. Shuffle: a 48 px round tonal button. Both have tooltips.

## Affected Crates
- `nemo-ui` only.

## Data Flow
- No new jobs. The page seed comes from `Job::PageSeed` (cover worker), stored in `App::page_seeds`, now `pub(crate)` so the view can read it. Rows send the same messages as before (`PlayAt`, `Shuffle`, `Ui(OpenMenu)`, `ShowMore`). No audio buffers are involved.

## Interfaces
```rust
// crates/nemo-ui/src/scheme.rs
pub const GLOW_TOP_ALPHA: f32; pub const GLOW_MID_ALPHA: f32; pub const GLOW_MID_AT: f32;
pub fn page_glow(seed: Option<Seed>, base: BaseMode) -> [Rgba; 2];

// crates/nemo-ui/src/gui/views/detail/collection.rs
pub const TWO_COLUMN_WINDOW: u32; pub const TWO_COLUMN_CONTENT: u32;
pub enum Columns { Two { left: u32, list: u32 }, Stacked { art: u32 } }
pub fn columns(window_w: u32, content_w: u32) -> Columns;
pub struct Collection<'a> { pub kind: &'static str, pub artwork: Artwork, pub title: &'a str,
    pub owner: Option<String>, pub year: Option<u16>, pub tracks: &'a [Track], pub numbered: bool }
pub fn kind_line(kind: &str, year: Option<u16>) -> String;
pub fn count_line(tracks: &[Track]) -> String;
pub fn view<'a>(app: &'a App, key: &DetailKey, c: &Collection<'a>, content_w: u32) -> Element<'a, Message>;
pub fn skeleton(app: &App, content_w: u32) -> Element<'_, Message>;

// crates/nemo-ui/src/gui/views/detail/song_row.rs (private module)
pub struct Rows { pub width: f32, pub source: ListSource, pub numbered: bool }
pub fn second_line(track: &Track, with_album: bool) -> String;
pub fn view<'a>(app: &'a App, track: &Track, index: usize, rows: &Rows) -> Element<'a, Message>;
```

## Error Handling
- No new failure paths. A failed page load shows the existing status block with "Try again".
- Without a seed yet, the gradient is a neutral grey that the seed replaces when it arrives.

## Testing Strategy
- `scheme::tests::page_glow_keeps_text_and_the_play_disc_legible`: 109 seeds by 3 base modes. Every ink is checked over the glow at the top and middle alphas, over a pane, over the real field colours (retoned fixture covers and the gradient field). The Play disc reaches 3:1.
- `detail::collection::tests`: the breakpoints and column widths; the kind and count lines, which leave out what is unknown.
- `detail::song_row::tests`: `Artist • Album` and its fallbacks.
- Headless snapshots: `playlist-1366x697`, `album-1366x697`, `album-1920x1040`, `album-ytm-light-1366x697`, `playlist-ytm-stacked-1100x700`, `playlist-ytm-stacked-960x600`, `album-loading-1366x697`, `playlist-loading-1366x697`.
- No audio code changed.

## Risks and Alternatives
- The track row is a copy of the shared `kit::track_row`, shaped for this page. Changing the shared row (a new `RowCtx` field) would touch every list, including those of the search lane. The cost is two row implementations to keep in step.
- The two scroll areas in two columns: the left column scrolls only when the window is too short for it.
- Alternative: keep one scroll for the whole page (simpler), but the cover and Play would scroll away, unlike YouTube Music.
