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
- Audio untouched, so no bit-exact test is needed.

## Risks and Alternatives

- Clipping a second SVG copy with a layer was tried first: tiny-skia drew the clipped copy whole. The gradient inside the SVG works on every renderer and needs no custom widget.
- One SVG raster per fill level: bounded by the 1/64 rounding.
- An LRC file with long lines and no blank lines shows no gap note. Accepted: guessing would put a note over singing.
