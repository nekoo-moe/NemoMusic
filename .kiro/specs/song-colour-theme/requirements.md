# Song colour theme: requirements

Owner approved 2026-11-18 by request: "Add an option (turned on by default): the whole app's background automatically blends the theme colours into the colours of the song that is playing, with a gentle moving heartbeat so it feels more alive, and an animation when the colours change to the next song." Owner overrides of `design.md`: the 600 ms bloom (section 8) and a second background loop, the heartbeat (sections 7 and 8).

## Goal
While a song plays, the whole window takes its colours from the song's cover: a clean mesh of three or four colour fields in the background, tinted panes and accent roles. The background breathes with a soft heartbeat and blooms into the next song's colours when the song changes. Both can be turned off, and text keeps 4.5:1 contrast over any cover.

## User Stories
- As a listener, I want the window to wear the colours of the song, so that the app feels like the music.
- As a listener, I want a gentle heartbeat in the background, so that the app feels alive.
- As a listener, I want the colours to bloom into the next song's colours, so that a song change is a moment and not a flash.
- As a listener who prefers calm, I want to turn the song colours and the heartbeat off, so that the window stays quiet and neutral.

## Acceptance Criteria
1. THE SYSTEM SHALL offer two switches in Settings, Appearance, with `On`/`Off` text: `Match colours to the playing song` (default On) and `Gentle heartbeat` (default On), saved in the config; a config file without the keys loads with both On.
2. THE SYSTEM SHALL grey out `Gentle heartbeat` WHILE `Match colours to the playing song` is Off or Reduce motion is on.
3. WHILE `Match colours to the playing song` is On and a cover is known, THE SYSTEM SHALL take the background fields, the pane tint and the accent roles from that cover on every screen, over the chosen theme.
4. WHILE it is Off, or no cover is known, THE SYSTEM SHALL take the accent roles from the chosen theme only, keep the panes neutral and draw one flat tonal background.
5. THE SYSTEM SHALL draw the background as a mesh of three to four colour fields from the cover's dominant colours, distinct hues kept apart, with richer chroma than the accent palette (which keeps the cap of `design.md` section 2), a vignette toward the bottom and the panes at their alpha as the scrim.
6. THE SYSTEM SHALL keep every text ink at 4.5:1 or more over the panes and the gaps, over the fixture covers all-black, all-white, grey, saturated, yellow and noisy, in dark, AMOLED and light, at the peak of the heartbeat too.
7. WHILE a song plays in a focused window, the heartbeat is on and the backdrop moves, THE SYSTEM SHALL pulse the fields in a double "lub-dub" beat every 1.6 s (about 2.5 % scale, about 5 % brightness) over the slow drift, at most 24 frames per second, without `update` or `view` calls and without re-blurring.
8. WHILE paused, stopped, unfocused, under reduced motion, under reduce transparency, with the backdrop hidden or on the CPU renderer, THE SYSTEM SHALL keep the pulse and the drift still and ask for no frames.
9. WHEN the next song's colours are ready, THE SYSTEM SHALL bloom the new fields in over the old ones in 600 ms (scale 1.04 to 1.0, `emphasized-decelerate`) and cross-fade the accent roles in 400 ms; under reduced motion the swap is instant. The new palette is built on the cover worker. While the next cover loads, the old fields stay.

## Out of Scope
Lyrics views, the player bar, the splash and the setup flow (other agents). The setup flow can show the two switches later through `SettingChange::SongColourTheme` and `SettingChange::SongHeartbeat`.

## Open Questions
- With the switch On, a fixed theme (Ocean, Sakura, ...) is overruled by the song while a cover is known (criterion 3). Should a fixed theme keep its accent and only the background follow the song?
- Real window, real GPU, Windows and 150 % / 200 % scale were not checked here (no display). The frame cost is a headless number (see the PR).
