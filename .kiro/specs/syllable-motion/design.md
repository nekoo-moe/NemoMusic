# Syllable motion: design

## Overview

`lyric_pieces.rs` (pure) gives, per piece and playback time: state (unsung, singing, sung), fill
progress, lift, scale and glow, and which pieces are plain or animated. `gui/syllable_text.rs` is
a small widget that draws the active line from two paragraphs shaped once per change of the
animated range (Advanced shaping, so Vietnamese, CJK and RTL stay correct): the base paragraph
holds sung words bright and the rest grey; the second shows only the animated words, drawn grey,
then bright clipped to the filled share, with a soft half-bright edge, a lift, glow copies and a
held-syllable scale applied as a draw-time transformation. `lyrics_view::line_alpha` fades the
other lines by distance.

## Affected Crates

- `nemo-ui` (`lyric_pieces.rs`, `lyrics_view.rs`, `gui/syllable_text.rs`, `gui/views/lyrics.rs`,
  `gui/headless.rs` and a snapshot).

## Data Flow

UI thread only. Frames already run while a song plays (`needs_frames`); the position is read
as before. No audio code is touched. Paragraphs are rebuilt only when the animated range
changes (a few times a second), not per frame; a frame draws at most about eight text runs per
animated word.

## Interfaces

```rust
pub fn timings(line: &LyricLine) -> Vec<Timing>;
pub fn state_at(t_ms: i64, w: Timing) -> WordState;
pub fn fill_at(t_ms: i64, w: Timing) -> f32;
pub fn fx_at(t_ms: i64, w: Timing, reduced: bool) -> WordFx;
pub fn frame(timings: &[Timing], t_ms: i64, reduced: bool) -> Frame;
pub fn held_scale(len_ms: i64) -> f32;
pub fn is_rtl(text: &str) -> bool;
pub fn lyrics_view::line_alpha(dist: i32) -> f32;
```

## Error Handling

Pieces whose text cannot be found fall back to the word texts (existing `texts`). An empty
timing list draws the plain line.

## Testing Strategy

Pure tests for state, fill at start, middle, end and just before; held-scale curve; lift easing;
reduced-motion mapping; the line-alpha falloff; contrast; a paused syllable doc needs no frames.
Headless snapshot of a mid-word frame (`syllable-mid-word-*`). No audio change, no bit-exact test.

## Risks and Alternatives

- The glow is four offset copies clipped to the word box, not a blur.
- Text clipping under a transformation was checked on tiny-skia only; wgpu multiplies clip
  bounds by the transformation as well (read in the iced source), not run here.
- Alternative: split the singing word into spans by character for a stepwise wipe. Rejected:
  not smooth.

## Steering conflicts

Owner override 2026-11-16: `design.md` section 10 future-word alpha 0.55 becomes 0.38 on
`on-surface-variant`, sung words full `on-surface` (was 0.9), other lines dimmer than that, and
the "no karaoke wipes" rule is lifted for word-timed lyrics. See the proposed edit in the PR.
