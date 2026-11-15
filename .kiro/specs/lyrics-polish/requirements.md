Owner pre-approved 2026-11-15 (UI polish wave, with screenshots of a Linux run). Where a request below goes against `design.md`, the owner's request wins; each case is marked "owner override, 2026-11-15" and the matching `design.md` edit ships in the same PR.

## Goal

Synced lyrics feel calm and premium in every place they show. Before the first sung line and during long instrumental breaks, a music note sits where the next line will appear and fills up with the song, full at the moment the line starts. The full-screen lyrics get quiet, icon-only controls, a thin progress bar, a small Timing popover in place of the offset pills, and a lyric column with fewer, larger, better spaced lines that fade toward the edges. Lyrics and display text use a refined open-licence sans with clean Vietnamese diacritics instead of the rounded Google Sans Flex.

## User Stories

- As a listener, I want to see that the singing is about to start, so that I am not left looking at a silent screen.
- As a listener, I want an instrumental break to be marked, so that I know the lyrics did not stall.
- As a listener in full-screen lyrics, I want the controls to step back, so that the words and the cover carry the screen.
- As a listener who fixes lyric timing, I want the offset tools one click away, so that they are there when I need them and hidden when I do not.
- As a Vietnamese listener, I want lyrics in a refined font with clean, unclipped diacritics, so that the lyrics read like a quality app.

## Acceptance Criteria

### Intro and gap note (task 1)
1. WHEN synced lyrics are shown and the position is before the first sung line, and that line starts at least 1 s into the song, THE SYSTEM SHALL show one music note in the place of that line, anchored like the active line.
2. WHEN the break between two sung lines is at least 5 s, THE SYSTEM SHALL show the note above the next line from the end of the previous line (its end time, the end of its last timed word, or a blank LRC line after it) until the next line starts. Shorter breaks, and breaks with no known end, SHALL show no note.
3. THE SYSTEM SHALL fill the note from the bottom by elapsed / (line start - gap start), driven by the playback position (no free-running timer), and the note SHALL be full at the moment the line starts. "Starts" is the moment the line becomes active: its start time minus the 150 ms lead of `design.md` section 10, after the user's offset.
4. WHEN the line starts, THE SYSTEM SHALL fade the note out with `motion.fast` (100 ms) while its row folds away, and the line SHALL become active in the usual way. The note fades in with `motion.fast` at the gap start.
5. WHILE reduced motion is on, THE SYSTEM SHALL keep the fill and SHALL drop the fades and the fold (the note appears and goes at once; the list jumps).
6. THE SYSTEM SHALL show the note in the right-pane lyrics, the full-screen lyrics, the compact full-window layout and the Classic style. Unsynced and instrumental lyrics SHALL show no note.
7. THE SYSTEM SHALL draw no extra frames for the note: it changes only with position updates, which already run while playing.

### Full-screen lyrics and controls (task 2)
8. THE SYSTEM SHALL draw the full-screen controls under the art icon-only and smaller, in `on-surface-variant` at rest and `on-surface` on hover or focus. Play/pause SHALL be the only filled control, in a neutral tone, not the accent.
9. THE SYSTEM SHALL draw a thin progress bar in full screen, with both times in tabular numerals.
10. THE SYSTEM SHALL move the lyric offset tools (`-50 ms`, value, `+50 ms`, `Reset`) into one small Timing popover opened by an icon button; `[` and `]` SHALL keep working. The quality badge SHALL become a quiet text label that still opens the signal path. (owner override, 2026-11-15: `design.md` section 10 shows the offset buttons inline)
11. THE SYSTEM SHALL show fewer lyric lines with more space between them in full screen: the active line large, neighbour lines one step down in size and opacity, and lines further away fading toward the top and bottom edges with a soft mask. The max line width stays 640 px. (owner override, 2026-11-15: sizes and spacing of `design.md` section 10)
12. THE SYSTEM SHALL keep the provider footer, the sync state and the `Back to current line` pill.
13. THE SYSTEM SHALL scroll to the active line on the `spatial.default` spring, and jump under reduced motion.

### Lyric font (task 3)
14. THE SYSTEM SHALL draw lyrics, the full-screen title and display text in one refined OFL sans with full Vietnamese support, chosen from Inter, Be Vietnam Pro and Plus Jakarta Sans by a rendered comparison of the same Vietnamese lines. (owner override, 2026-11-15: `font.display` was Google Sans Flex)
15. THE SYSTEM SHALL bundle at most two weights of that family, with their OFL texts.
16. THE SYSTEM SHALL use a line height of at least 1.25 times the size for large lyrics, so stacked Vietnamese diacritics are not clipped.
17. Body UI text SHALL stay as it is.

## Out of Scope

- The Apple Music style builder (now-playing-styles task 6).
- Word-level fill inside the note, and a note for unsynced lyrics.
- Changes to the floating player bar.
- Any change to playback or audio.

## Open Questions

- None for task 1. Task 2 and task 3 record their choices in `design.md` of this spec.
