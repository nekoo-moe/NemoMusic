# Living backdrop: requirements

Owner pre-approved 2026-11-15 (UI polish wave). Owner override of `design.md` sections 7 and 9 ("static" backdrop, 0 idle frames), 2026-11-15: the backdrop drifts slowly.

## Goal
The window background takes the colours of the song that is playing, changes with every song, and moves very slowly, so the app feels alive but stays calm. Text keeps AA contrast over any cover, and the motion stops when it should (reduced motion, reduce transparency, window not focused).

## User Stories
- As a listener, I want the whole window to change colour when the song changes, so that the screen follows the music.
- As a listener, I want a calm, slow movement in the background, so that the app feels alive without pulling my attention.
- As a listener who turned on reduced motion or reduce transparency, I want a still or flat background, so that the app respects my setting.
- As a laptop user, I want the motion to cost little power, so that a long listening session does not drain the battery.

## Acceptance Criteria
1. WHEN the playing song changes and its cover is analysed (on the cover worker, never on the UI thread), THE SYSTEM SHALL build the backdrop and the accent palette from the new cover.
2. WHEN the backdrop changes from one cover to the next, THE SYSTEM SHALL cross-fade it over 400 ms (`design.md` section 2), together with the accent roles; with reduced motion the swap is instant.
3. WHILE the Dynamic theme is selected, THE SYSTEM SHALL colour the window background from the playing song on every screen, including an open album, playlist or artist page.
4. THE SYSTEM SHALL draw the backdrop from the cover's pre-blurred canvas texture (blur radius at least 60 px at window scale), two copies over each other so two soft colour fields show, with a vignette toward the bottom, under the panes.
5. THE SYSTEM SHALL keep every text ink at 4.5:1 or more on the panes and in the gaps between them, over the fixture covers all-black, all-white, grey, saturated, yellow and noisy, in dark, AMOLED and light modes.
6. WHILE a song plays, the window is focused, the backdrop is visible and neither reduced motion nor reduce transparency is on, THE SYSTEM SHALL move the backdrop on a closed path with a 30 to 60 s cycle, a translation of a few percent and a slight scale breathe.
7. WHILE the backdrop moves, THE SYSTEM SHALL draw at most 24 frames per second for it, without running the app's `update` or `view` for the drift.
8. WHILE reduced motion is on, the window is not focused, playback is paused or stopped, the full-screen Now Playing covers the backdrop, or the CPU renderer is in use, THE SYSTEM SHALL keep the backdrop still and request no frames for it.
9. WHILE reduce transparency is on, THE SYSTEM SHALL draw a flat tonal colour from the cover instead of the texture.
10. THE SYSTEM SHALL never re-blur the cover per frame and never touch the audio thread for the backdrop.

## Out of Scope
- Motion of the right Now Playing pane gradient and of the full-screen Now Playing backdrop (owned by the lyrics lane).
- A custom wgpu shader (a textured quad moved by the widget is enough).
- The playlist and album page layout (task 2 of the wave, `collection-page-ytm`).

## Open Questions
- The owner's real Linux run was not reproduced here (no display). The mock run shows the backdrop rebuilding on each song; the root cause below explains the owner's screenshot. Please confirm on the real build.
- The GPU cost of the drift was not measured (no GPU in the build container). See design.md, Testing Strategy.
