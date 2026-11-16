# App motion: requirements

Owner pre-approved 2026-11-15 (UI polish wave). Owner request: "The app works but lacks the
core: motion across the whole app. Review and update everything, with nothing missing."

## Goal

Every state change the user can see in NemoMusic moves the way `design.md` section 8 says: a
calm fade, a short slide of 16 px or less, a shape morph or a sliding indicator, on the
`spatial.*`, `effects.*` and `motion.*` tokens, never longer than 400 ms and never looping. One
reduce-motion switch (the OS setting or the user toggle) turns all of it into instant swaps
while progress keeps updating. An idle window draws no frames, apart from the capped backdrop
drift.

## User Stories

- As a listener, I want pages, panes, popovers and the player to move smoothly, so that the app
  feels finished and calm instead of snapping.
- As a listener who gets motion sickness, I want the OS "reduce animations" setting or the app
  switch to stop all motion, so that nothing slides or fades.
- As a laptop user, I want the app to draw nothing while nothing changes, so that it costs no
  battery when idle.

## Acceptance Criteria

1. WHEN the route changes (Home, Explore, Search, Library, album, artist, playlist, Mood,
   Settings, back or forward) THE SYSTEM SHALL fade the new page in and grow it from 98% to
   100% in 240 ms (emphasized-decelerate).
2. WHEN a popover, menu, sheet, dialog or toast opens THE SYSTEM SHALL fade it in with a rise of
   at most 16 px over `motion.base`, and WHEN it closes THE SYSTEM SHALL fade it out over
   `motion.fast`.
3. WHEN the full-window Now Playing opens THE SYSTEM SHALL grow the cover from 86% to 100% with
   the page fade (shared-element feel).
4. WHEN a selection moves between chips, tabs or navigation entries THE SYSTEM SHALL slide one
   selection pill on `spatial.fast`.
5. WHEN a picture arrives THE SYSTEM SHALL fade it in over 150 ms (`design.md` section 9).
6. WHEN a page is loading THE SYSTEM SHALL show a skeleton that does not loop.
7. WHEN the track changes THE SYSTEM SHALL cross-fade the cover and the title, and the accent in
   400 ms.
8. WHEN the like state changes THE SYSTEM SHALL morph the heart (fill fades in on
   `effects.fast`).
9. WHEN the OS reduce-motion setting is on, or the user's Reduce motion switch is on, THE SYSTEM
   SHALL swap every animated value at once (0 ms) and keep progress and lyrics position live.
10. WHEN every animation has settled THE SYSTEM SHALL ask for no frames (only the backdrop
    drift may run, capped at 24 fps), proven by a test.
11. WHILE a track menu is open THE SYSTEM SHALL animate nothing but the menu.
12. THE SYSTEM SHALL allocate nothing per frame in the motion helpers and SHALL never touch the
    audio thread.
13. WHEN the window is resized THE SYSTEM SHALL reflow without motion and without a jump.

## Out of Scope

- Live backdrop blur, new loops, visualizers, marquees.
- True shared-element flight of the cover from the capsule to the full window (it would move
  far more than 16 px; the 86% grow stands in for it).
- Animated hover of iced's own sliders, toggles and text fields (no style hook for time).
- The collection page (`gui/views/detail.rs`, `detail/collection.rs`): another agent owns it
  now; it gets the shared widget motion (rows, cards, art fades) without edits.

## Open Questions

- The equaliser bars on the playing row and the buffering ring are loops that
  `docs/ui-redesign.md` section 4 allows and design.md section 8 does not list. Kept as they
  are; owner to confirm.
- Windows reduce-motion detection (SPI_GETCLIENTAREAANIMATION) needs an OS call; not read yet.
