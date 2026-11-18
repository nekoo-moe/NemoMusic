# Player bar redesign: requirements

Owner request (2026-11-18): "The music navigation bar is far too ugly, redesign the whole thing." The owner approved the requirements by sending that request.

## Goal

The bottom player bar becomes a finished-looking floating capsule in Material 3 Expressive: a rounded surface inset from the window edges, with the song on the left, a large play button and a thick, easy seek slider in the middle, and the quality pill and volume on the right. On narrow windows the right side folds away in steps and the text never overlaps. Every control that exists today keeps working and sends the same messages.

## User Stories

- As a listener, I want the bar to look calm and finished, so that the app feels good to leave open all day.
- As a listener, I want a big play button and a seek slider that grows under the pointer, so that pausing and seeking are easy.
- As a listener on a small window, I want the bar to drop the least useful parts first, so that I can still read the title and use the controls.
- As a keyboard or screen-reader user, I want a visible focus ring, a tooltip and an accessible name on every icon button, so that I can use the bar without a mouse.
- As a user who set Glass `Off` or `Reduce transparency`, I want a plain opaque bar, so that nothing is see-through.

## Acceptance Criteria

1. WHEN a track is loaded THE SYSTEM SHALL draw the bar as a capsule 88 px high, inset 12 px from the left, right and bottom window edges, with `shape.xl` corners and a width of at most 1200 px, centred.
2. WHEN `glass_tier` is `Low` and `reduce_transparency` is off THE SYSTEM SHALL fill the capsule with an opaque `surface-container-high` carrying a soft tint from the cover's three sampled colours, plus the glass edge highlight; WHEN `glass_tier` is `Off` or `reduce_transparency` is on THE SYSTEM SHALL fill it with plain `surface-container-high` and a 1 px `outline-variant` border, with no tint and no highlight.
3. THE SYSTEM SHALL lay the capsule out in three zones: left (cover 56 px with `shape.md` corners that opens Now Playing, title `title-sm` over artist `body-sm`, one line each with ellipsis, then the like heart), centre (shuffle, previous, a 56 px play button, next, repeat, and under them the seek row) and right (quality pill, lyrics, queue, tools, speaker with an inline volume slider, full-screen).
4. WHEN the track is playing or paused THE SYSTEM SHALL draw the play button as an accent disc whose glyph cross-fades between play and pause with the existing expressive spring; WHEN the player is buffering THE SYSTEM SHALL show the calm busy ring in its place.
5. THE SYSTEM SHALL draw the seek row as `m:ss` (or `h:mm:ss` from one hour) tabular times on both sides of a 4 px rounded track that grows to 6 px under the pointer or while dragging, with the thumb shown on hover, focus and drag, the played part in the song accent, and a time tooltip while dragging. WHEN the duration is unknown THE SYSTEM SHALL show `-:--` and keep the slider at zero.
6. WHEN the capsule's width is below 1100 px THE SYSTEM SHALL hide the volume slider and keep the speaker button; WHEN it is below 1020 px THE SYSTEM SHALL also fold lyrics, tools and full-screen into one `More` button that opens the existing tools panel; THE SYSTEM SHALL keep the centre zone at least 320 px wide and SHALL NOT let any text overlap another control at window widths from 960 px up.
7. WHEN the speaker button is pressed THE SYSTEM SHALL set the volume to 0 and remember the old volume; WHEN pressed again at volume 0 THE SYSTEM SHALL restore it (the volume slider and the `Up` and `Down` keys keep working as today).
8. THE SYSTEM SHALL keep the quality pill text exactly as `crate::signal::badge_label` builds it (`Buffering…` while buffering), SHALL NEVER show `Lossless` for a stream, and pressing the pill SHALL open the signal path popover as today.
9. WHILE nothing changes THE SYSTEM SHALL draw no frames for the bar; WHEN hover, press, focus, the play morph, the slider growth or a pill change happens THE SYSTEM SHALL animate it with the shared motion springs, and WHEN reduced motion is on THE SYSTEM SHALL jump to the end state.
10. THE SYSTEM SHALL give every icon button a 500 ms tooltip, an accessible name, a 32 px or larger target and a visible 2 px focus ring.
11. THE SYSTEM SHALL keep every message the old bar sent (play or pause, previous, next, seek drag and release, shuffle, repeat, volume, queue, tools, signal path, like, open Now Playing) and the panes SHALL end above the capsule so no row is hidden.
12. THE SYSTEM SHALL keep the text readable on dark, AMOLED and light with a light cover and a saturated cover behind the capsule (contrast of the ink on the capsule fill at least 4.5:1).

## Out of Scope

- The full-screen Now Playing controls (lyrics agent), the window background (song-colour agent), the splash and the setup flow.
- A buffered range on the seek track and SponsorBlock ticks: the player snapshot carries neither, and `nemo-core` public API stays as it is. The track draws them when the engine reports them.
- A real equaliser screen. The tune button opens the existing tools panel (sleep timer, speed, window modes).
- Live-blur glass tiers (High, Medium).
- Audio behaviour of any kind. Mute only sends the existing `SetVolume` command.

## Open Questions

- The brief lists a "mute on click" behaviour that the old bar did not have. It is built with the existing `SetVolume` command and a remembered volume held in the UI. Say if the owner wants it to live in the engine instead.
- Should the lyrics button in the right zone, which matches a click on the cover, stay once the Now Playing pane is closed by default on narrow windows? It stays for now.
