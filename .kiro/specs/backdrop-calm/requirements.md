# Backdrop calm: requirements

Owner approved 2026-11-19 by request, after three screenshots of the real app on Linux: "While music plays the background keeps flashing: blink blink, then repeat. It looks like a conflict between things. Instead of a smooth scene effect. It only happens while music plays; when I pause it stops. With Reduce transparency on it does not happen and it works much better. Fix the background." and "The music bar is not blurred like Material glass, it is very opaque." Owner overrides of `design.md` sections 7, 8 and 9 (the heartbeat) and section 6 (the bar material), each labelled `(owner, 2026-11-19; backdrop-calm)`.

## Goal
The window background moves like a slow, soft scene effect and never flashes, and the player bar is real translucent glass over it. Nothing else about the backdrop, the bar or their switches changes.

## User Stories
- As a listener, I want the background to move smoothly while a song plays, so that it feels alive and not like a blinking light.
- As a listener, I want the player bar to show the soft colours behind it, so that it looks like Material glass and not a grey slab.
- As a listener who prefers calm, I want the two song-colour switches, reduced motion and reduce transparency to keep working as before.

## Acceptance Criteria
1. WHEN a song plays in a focused window with the heartbeat on, THE SYSTEM SHALL move the colour fields in a slow breath: growth of 1.5 to 2.5 % and a small shift of their centres toward the window centre, eased in and out, one cycle every 2 to 3 s, and SHALL NOT lift, tint or flash any colour.
2. THE SYSTEM SHALL keep the mean lightness (CIE L*) of the composed backdrop within 1.5 points of the 0 to 100 scale over one breath and within 0.5 points from one frame to the next, at 24 and at 60 frames per second with uneven frame times, over the fixture covers all-black, all-white, grey, saturated, yellow, three hues and four hues, in dark, AMOLED and light.
3. THE SYSTEM SHALL have one frame clock for the window while a song plays: the app's frame ticker. The backdrop SHALL NOT run a timer or request redraws of its own, and SHALL read the time of each redraw.
4. WHILE nothing moves (paused, unfocused, reduced motion, reduce transparency, backdrop hidden, song colours off), THE SYSTEM SHALL ask for no frames for the backdrop; the CPU renderer draws one still image.
5. WHEN the backdrop is built again for the same cover, THE SYSTEM SHALL reuse the same image handles, so no texture is uploaded again.
6. THE SYSTEM SHALL start the bloom of the window backdrop once per song change, and SHALL NOT restart it on a position tick, a snapshot or a player event of the same song.
7. WHILE the song colours are on and the window is not Reduce transparency or tier Off, THE SYSTEM SHALL draw the player bar as translucent glass: `surface-container` over the window backdrop at an alpha of 0.49 to 0.55 in light mode and 0.38 to 0.42 in dark and AMOLED (owner, 2026-11-19, clearer glass), a 1 px highlight (white 0.22, brighter on the top edge), a soft outer shadow and a light inner gradient, with `on-surface` and `on-surface-variant` at 4.5:1 or more over the fixture covers in dark, AMOLED and light.
8. WHILE tier is Off or Reduce transparency is on, THE SYSTEM SHALL draw the bar as opaque `surface-container-high` with the 1 px `outline-variant` edge, as before.
9. THE SYSTEM SHALL keep every control, state and behaviour of the bar unchanged.

## Out of Scope
The Appearance settings page and the window settings, lyrics rendering, startup, audio, a live blur of the backdrop under the bar (tier High).

## Open Questions
- The owner's real Linux run was not reproduced here (no display, no GPU). The cause below is shown by numbers from the pure maths, not by a screen. Please confirm on the real build.
