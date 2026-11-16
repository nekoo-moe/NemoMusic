# Syllable motion: requirements

Owner request and approval: 2026-11-16 (pre-approved).

## Goal

On word- and syllable-timed lyrics (Unison, Better Lyrics API, TTML) the words still to be sung
are clearly grey and turn into the real lyric colour as they are sung, with a smooth
left-to-right fill inside the word being sung, so the owner can see the motion. Ideas come from
better-lyrics (per-syllable fill, lift, glow, scale of held syllables, opacity by distance).

## User Stories

- As a listener, I want unsung words grey and sung words bright, so that I can follow the song.
- As a listener, I want the word being sung to fill from grey to bright and rise a little, so that
  the line feels alive.
- As a listener who reduces motion, I want a plain colour change per word, so that nothing glides.

## Acceptance Criteria

1. WHEN a word-timed line is active, THE SYSTEM SHALL draw unsung words in `on-surface-variant`
   at 0.38 alpha and sung words in full `on-surface`; other lines SHALL stay dimmer than the
   unsung words; line-timed lyrics SHALL keep their current look.
2. WHEN a word is being sung, THE SYSTEM SHALL fill it from grey to bright left to right (right
   to left for RTL text) by `(position - start) / (end - start)`, clamped and eased.
3. WHEN a word is being sung, THE SYSTEM SHALL lift it up to 2 px and add a soft glow, easing in
   over 100 ms and out over 200 ms after its end; a syllable held 1 s or more SHALL scale up to
   1.04. Layout SHALL NOT change (draw-time transform only).
4. THE SYSTEM SHALL fade lines by their distance from the active line (ahead: 0.30, 0.24, 0.18;
   behind, faster: 0.20, 0.12, 0.07 of `on-surface-variant`); true blur is not used.
5. WHEN reduced motion is on, THE SYSTEM SHALL show no lift, glow, scale or wipe: each word turns
   bright when it starts.
6. WHILE playback is paused and animations are settled, THE SYSTEM SHALL request no frames.
7. THE SYSTEM SHALL apply this in the right pane, the full-screen lyrics and the Now Playing
   styles that reuse the lyric view.

## Out of Scope

- True per-line blur (no layer blur in iced 0.14); a new spring for the line size step (the
  existing line cross-fade and the `spatial.default` scroll glide stay).
- Changing line-timed or unsynced lyrics.

## Open Questions

- The 0.38 grey is about 2.2:1 on the dark surface, below the 3:1 large-text rule of
  `design.md` section 13. The owner asked for 0.38; confirm or raise toward 0.5.
- Not verified on a GPU (wgpu) run: the snapshot uses the tiny-skia renderer.
