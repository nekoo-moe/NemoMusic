Owner pre-approved 2026-11-15.

## Overview

Three stacked changes in `nemo-ui`, one PR each.

1. Intro and gap note. A pure function in `lyrics_view::note` turns the position into a `Focus`: the line drawn as active, and an optional `GapNote { before, fill, presence, waiting }`. The reducer anchors the scroll on the note row while it waits; the views insert the note row above line `before`. The glyph is the Material "music-note" symbol (the same file as `Icon::Note`), filled from the bottom with a hard-stop gradient inside the SVG, so the fill follows the note shape exactly.
2. Refined full-screen lyrics and controls. Quiet full-screen transport and progress, a Timing popover, and a lyric column with more air and an edge fade.
3. Refined lyric font. A display family for lyrics and display text, picked from a rendered comparison.

### Task 1 choices

- One note glyph, not three dots. It is the calmer of the two: one shape, one slow change, and it is what YouTube Music shows and what the owner described ("a music-note indicator that fills up"). Three dots add a step every third of the gap.
- Gap threshold 5 s (`GAP_MIN_MS`). A sung line lasts about 2 to 4 s; pauses below 5 s are breaths and short fills, and a note there would blink in and out with the lines. At 5 s the fill moves visibly (over 20% a second at most) and the 100 ms fades are a small part of it. The intro needs only 1 s (`INTRO_MIN_MS`), so a song that starts singing at once shows no flash.
- A break needs a known end: the line's `end_ms`, the end of its last timed word, or a blank LRC line after it (LRC files mark breaks with empty lines). Plain LRC with no end and no blank line cannot tell singing from silence, so it shows no note there.
- "The line starts" = the moment it becomes active (`start - 150 ms` lead, after the offset). The note uses the same clock as `active_line`, so it is full in the same frame the line lights up.
- While the note waits, no line is drawn as active (every line takes the "other" role), so the last line cross-fades back when a break begins and the note holds the anchor at 35%.
- Fades are computed from the position too (`presence`), not from a timer: in 100 ms after the gap start, out in 100 ms after the line start, while the row height folds with it. Reduced motion: presence is 1 or 0.
- The fill level is rounded to 1/64, so the SVG cache of the renderer holds at most 65 rasters of the note per size.

### design.md section 15 checklist (task 1)

1. Tokens only: sizes from the type scale (active-line row), `motion.fast`, `on-surface` and the "coming" lyric ink. Yes.
2. Spacing on the 4 px scale (`LINE_GAP` 8). Yes.
3. No glass added; the note sits on the lyric canvas. Yes.
4. Contrast: the filled part is `on-surface`, the rest the "coming" ink that keeps 4.5:1 on the song canvas (`canvas.rs`). The glyph is a non-text icon, so 3:1 is enough. Yes.
5. Reduce transparency: nothing transparent added. Yes.
6. No quality label touched. Yes.
7. No cover touched. Yes.
8. Motion 100 ms fades, reduced motion keeps only the fill. Yes.
9. The note is not interactive; keyboard unchanged. Yes.
10. Provider footer, sync state, offset control and unsynced fallback unchanged; unsynced shows no note. Yes.
11. No wording touched. Yes.
12. No new frames: position updates already run each frame while playing; paused shows a still note. Nothing touches the audio thread. Yes.

### Task 2 choices

- Full-screen line sizes step 32/40, 24/32, 20/28 (`headline-lg`, `headline-sm`, `title-md`), line gap 24 instead of 8. About six lines are readable at 1366 x 697 instead of ten. The right pane keeps its dense sizes. Every line height is at least 1.25 times the size.
- Opacity steps: the active line is `on-surface`, the other lines use the "coming" ink that keeps 4.5:1 on the song canvas. On top of that, a soft edge mask (`lyrics_view::edge_fade`, smoothstep over the top and bottom 20% of the viewport) fades lines out toward the edges. It is computed per line from its position after the scroll, so no shader or extra layer is needed. A line under the mask is only its height, so it costs nothing to draw. Lines inside the band are decorative and may drop under 4.5:1 (owner override, 2026-11-15). The middle band, where the active line and its neighbours sit, keeps full contrast.
- Quiet controls (`now_playing/controls.rs`): previous and next are icon-only (24 px glyph, 40 px target), `on-surface-variant` at rest and `on-surface` while the pointer is over the glyph (the SVG hover state). Keyboard focus shows the usual focus ring of `fx`. Play/pause is an `on-surface` disc with a `surface` glyph, keeping the morph. The progress rail is 3 px, its handle 4 px and 6 px under the pointer, both neutral. Times are in `label-sm` under the rail, and the bundled digits are tabular.
- The Timing popover is opaque `surface-container-high` (`design.md` section 6: a popover is never glass). It opens from a timer icon in the top bar, holds `-50 ms`, the value, `+50 ms` and `Reset`, and names the `[` and `]` keys. A press outside, `Esc`, leaving the screen, or opening the signal path closes it (`UiState::timing_open`, `Event::ToggleLyricsTiming`). It shows only for synced lyrics, like the old pills.
- The quality badge is a borderless `label-sm` text button in `on-surface-variant`, and it still opens the signal path.
- The scroll keeps the `spatial.default` spring and jumps under reduced motion (unchanged code path).
- Classic keeps its own controls and pills. It is a separate style the owner did not ask to change.

### design.md section 15 checklist (task 2)

1. Tokens only: type scale, spacing scale, shape scale and colour roles. The rail width (3 px) and handle radii are new local numbers, like the old 4 px rail. Yes.
2. Spacing on the 4 px scale (gap 24, popover padding 16). Yes.
3. No glass: the popover is opaque. Yes.
4. Contrast: the controls are `on-surface-variant` (4.5:1 over the canvas) and the play disc is `on-surface` with a `surface` glyph. Lines in the edge band are decorative (owner override). Yes, with that override.
5. Reduce transparency: nothing transparent added. Yes.
6. The quality label shows the same honest badge text. Yes.
7. Covers untouched. Yes.
8. Motion: the spring is unchanged, nothing new animates, and reduced motion jumps. Yes.
9. Targets are 40 px and the focus ring is kept. `[` and `]` still work, and `Esc` closes the popover. Yes.
10. The provider footer, sync state, offset control (in the popover), unsynced fallback and pill are all kept. Yes.
11. No wording rule touched. Yes.
12. No new frames, and nothing touches the audio thread. Yes.

### Task 3 choices

- Comparison: `docs/ui/snapshots/lyrics-font-compare.png` (`examples/snapshots/font_compare.rs`; the candidate files come from `NEMO_FONT_DIR` and are not bundled) draws the same lines in Google Sans Flex (today), Inter, Inter Display, Be Vietnam Pro and Plus Jakarta Sans, all SemiBold. The lines are 32/40 lower case, 32/40 capitals with stacked marks (Ẩ, Ể, Ỗ, Ẳ) and 24/32.
- Pick: **Inter Display**, the display optical size of Inter. At 32 px it has the tightest spacing, open counters, a straight-sided, neutral shape (not rounded), and small, crisp marks that sit close to the letter without touching. Be Vietnam Pro draws excellent Vietnamese, but it is wider and geometric, and close to the soft look the owner called "childish". Plus Jakarta Sans is rounder still. Plain Inter (text optical size) is looser at large sizes.
- Weights: 400 for the lyric lines that are not active and for unsynced lyrics, 600 for the active line and titles. That keeps the two weights per view of `design.md` section 3. The static files of the Inter 4.1 release are used unmodified, because iced 0.14 cannot pick a variable-font instance (`assets/LICENSES.md`). They add about 830 KB to the binary. Subsetting would save most of that, but it makes a Modified Version, so it was left out.
- Measuring: `textfit` now reads the advances of either face (`Face::Ui`, `Face::Display`), so lyric wrapping and the scroll anchor use the display face's real widths.
- Used for: every lyric line (right pane, full screen, compact, Classic, unsynced) and the full-window titles of the M3 and Classic styles. Body UI text, the footer, controls and labels stay on Google Sans Flex.
- Line heights: every lyric role is at least 1.25 times its size (32/40, 24/32, 20/28; pane 20/28, 16/24, 14/20). A unit test checks this, and the capitals snapshot shows no clipped mark.

## Affected Crates

- `nemo-ui` only.

## Data Flow

UI thread only. `App` reads the player position each frame while playing (`Event::Position`, as before). `UiState::lyric_focus()` calls `lyrics_view::note::focus(doc, position, offset, reduced_motion)`. The reducer sets the scroll spring target with `lyrics_view::scroll_target_focus`; `App::sync` restarts the line cross-fade when `Focus::active` changes. Views read the same `Focus`. No audio buffer is touched, and no thread or channel is added.

## Interfaces

```rust
// nemo_ui::lyrics_view::note
pub const GAP_MIN_MS: i64;           // 5_000
pub const INTRO_MIN_MS: i64;         // 1_000
pub struct GapNote { pub before: usize, pub fill: f32, pub presence: f32, pub waiting: bool }
pub struct Focus { pub active: Option<usize>, pub note: Option<GapNote> }
pub fn gap_note(doc: &LyricsDoc, position_ms: i64, offset_ms: i64, reduced: bool) -> Option<GapNote>;
pub fn focus(doc: &LyricsDoc, position_ms: i64, offset_ms: i64, reduced: bool) -> Focus;
pub fn row_height(dense: bool) -> f32;

// nemo_ui::lyrics_view (task 2)
pub const LINE_GAP_ROOMY: f32;       // 24
pub const EDGE_FADE: f32;            // 0.2
pub const fn line_gap(dense: bool) -> f32;
pub fn edge_fade(center_y: f32, viewport_h: f32) -> f32;

// nemo_ui::state (task 2)
UiState::timing_open: bool;
Event::ToggleLyricsTiming;

// nemo_ui::lyrics_view
pub fn scroll_target_focus(focus: &note::Focus, viewport_h: f32, dense: bool, rows: &[LineRows]) -> f32;

// nemo_ui::state::UiState
pub fn lyric_focus(&self) -> lyrics_view::note::Focus;
```

## Error Handling

Pure arithmetic with saturating adds; a malformed document (no start times, overlapping lines) simply yields no note. No new failure path.

## Testing Strategy

- Unit tests in `lyrics_view/note/tests.rs`: intro fill, the fill at the exact line start (1.0, then the `motion.fast` fade), reduced motion, a long gap from a line end, a short gap (none, and the 5 s edge), a blank line as the break marker, word timing as the line end, unsynced/instrumental/short intro (none), the offset, and the scroll anchor on the note.
- Unit test of the note SVG (fill level, rounding, clamping).
- Headless snapshots `lyrics-*` from `examples/snapshots/lyrics_polish.rs`.
- Task 2: unit tests of the edge mask, the roomy gap and the line-height ratio; a reducer test for the Timing popover (toggle, one popover at a time, `Esc`, leaving the screen); snapshots `lyrics-after-gap-full-*` and `lyrics-timing-full-1366x697`.
- Task 3: glyph coverage of U+1EA0 to U+1EF9 and the horn and stroke letters in both display files; display-face measuring in `textfit`; the comparison PNG and the `lyrics-*` snapshots in the new face.
- Audio untouched, so no bit-exact test is needed.

## Risks and Alternatives

- Clipping a second SVG copy with a layer was tried first: tiny-skia drew the clipped copy whole. The gradient inside the SVG works on every renderer and needs no custom widget.
- One SVG raster per fill level: bounded by the 1/64 rounding.
- An LRC file with long lines and no blank lines shows no gap note. Accepted: guessing would put a note over singing.
