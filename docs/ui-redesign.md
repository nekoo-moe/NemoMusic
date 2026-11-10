# NemoMusic UI redesign (Material 3 Expressive)

Owner direction 2026-11-10: the first build looked raw. This file is the detailed spec for the
redesign; `.kiro/steering/design.md` keeps the rules (honest badges, contrast, tokens) and
`docs/ui-reference.md` the older numbers. Where they differ, this file wins for layout and motion.
Sources read: m3.material.io motion and shape pages, the Material Components Android motion
guide (spring values), simpmusic.org desktop docs and the owner's SimpMusic screenshot. No asset
is copied. All numbers are logical px.

## 1. What we build against
SimpMusic desktop: three rounded dark panes, a library list with art, a content pane with chips,
"Quick picks" and large cards, a right pane with a big cover and one live lyric line, a floating
capsule. It is flat: static hover colours, tiny type, no motion, a mixed icon set, no real cover
colour in the chrome. We keep the layout idea and win on finish (section 6).

## 2. Layout
Window background `surface-container-lowest`. Panes: `surface-container`, radius 28, 8 px outer
margin and 8 px gaps. Scroll content keeps 16 px side padding, so nothing pokes out of a corner.
| Window | Left pane | Centre | Right (Now Playing) | Capsule |
|---|---|---|---|---|
| 1920 x 1040 (W >= 1600) | 288, full nav + library | rest (1220), 5 card columns | 380 open, art 356 | 1040 x 72 |
| 1366 x 697 (1280 to 1599) | 248 | 770, 3 card columns of about 226 | 316 open (user can collapse, spring), art 292 | 1040 x 72 |
| 1100 to 1279 | 248 | rest | collapsed; opening it overlays the centre (340, opaque) | W - 64 |
| below 1100 (min 960) | 80 icon rail | rest | overlay on demand | W - 64, min 560 |
Capsule: bottom centre, 16 px above the window edge, floats over the panes. Every scroller keeps
96 px bottom padding so the capsule never hides a row. Heights: at 697 the right pane shows art,
title, actions, live line, tabs, and about 3 lyric lines; at 1040 it shows about 10 lines.

Left pane: nav pills Home, Search, Library (active pill = `secondary-container` of the cover
palette), then "Your library" with filter chips Playlists, Artists, Albums and rows of art 48 +
title + kind (playlists, artists as circles, albums). Rail mode: icons only.
Centre pane top bar (64): avatar button (or a "Sign in" pill), back and forward, filter chips on
Home (All, Songs, Albums, Artists, Playlists: real filters over the shelves, not fake moods),
search pill, settings, pane toggle. Home: "Quick picks" as a 2-column list of 8 songs (row 64,
art 56) with "Play all", then shelves of large cards (title and artist over a bottom scrim,
radius 20). Search, Album, Artist, Playlist, Library, Settings keep their content in this pane.
Right pane: art fills the width (radius about 18), title (`title-lg`), artist, actions (like,
more, expand to the full Now Playing window), the live lyric line (`title-sm`), tabs Lyrics |
Queue | Info. Info holds the honest quality badge and the signal path. Lyrics keep the provider
label and sync state in a footer. The pane background is a gradient from the cover's
`primary-container` to `surface-container`. Nothing playing: calm empty state.
Capsule (72, radius 36): shuffle, previous, play (circle that morphs to a rounded square),
next, repeat | mini art 48, title, artist | thin progress with times | like, quality badge
(from 1000 px), queue, volume. Glass tier Low: cover colours behind a tint, as before.
Honest controls: shuffle really shuffles the queue. Repeat and like have no backend yet and are
drawn disabled with a tooltip that says so.

## 3. Tokens
- Colour: neutral dark baseline from design.md, plus `surface-container-lowest` `#090A0C`,
  `surface-container-highest` `#272A32`, `outline` white 18%. Accent roles
  (`primary`, `secondary-container`, `primary-container`) follow the cover and cross-fade 400 ms.
- Type: Google Sans Flex, weights 400, 500, 600, tabular digits. Scale: label-sm 11/16, label-md
  12/16, body-md 14/20, title-sm 16/24, title-md 20/28, title-lg 22/28, headline-sm 24/32,
  headline-md 28/36, headline-lg 32/40, display-md 40/48. Two weights per view; 500 for
  controls only.
- Shape: xs 4, sm 8, md 12, lg 16, 20 for large cards, xl 28 panes and sheets, full for pills.
- Spacing: 4 8 12 16 24 32 48 64. Icons: Material Symbols Rounded, 24 grid, drawn at 20 or 24.
- State layers: hover 8%, press 10% of `on-surface`, drawn over the control, animated.
- Elevation: tone first; shadow `0 8 24 #00000052` only on the capsule, sheets, lifted cards.

## 4. Motion
All springs are `design.md` section 8 (stiffness, damping ratio): spatial fast 1400/1.0, default
700/1.0, slow 300/1.0, effects fast 3800/1.0; the play button uses 1400/0.8 (the only overshoot).
Non-spring fades use emphasized-decelerate `cubic-bezier(0.05,0.7,0.1,1)`.
| Motion | What moves | Timing |
|---|---|---|
| Page transition | content fades in and slides up 16 px on every route change | 320 ms emphasized-decelerate |
| Staggered entrance | cards and rows of a page, item i starts i x 45 ms later (max 8) | 320 ms each |
| State layers | hover 8%, press 10% fade over every button, chip, row, card | effects fast spring (about 60 ms) |
| Card hover lift | card rises 4 px with a soft shadow | spatial fast |
| Play button morph | circle (paused) to rounded square (playing), radius 50% to 30% | 1400/0.8 |
| Progress | fill follows the audio clock every frame; thumb grows on hover | per frame |
| Cover cross-fade | old cover under the new one, in the pane and the capsule | 400 ms |
| Palette cross-fade | accent roles and the pane gradient | 400 ms (existing) |
| Lyric glide | list scrolls so the active line sits at 35% | spatial default |
| Live lyric line | new line fades in and slides up 8 px | 240 ms |
| Pane open and close | right pane width and the centre pane follow one spring | spatial default |
| Capsule entrance | rises and fades in when the first track starts | spatial default |
| Sheet | sign-in sheet fades in with a 16 px rise over a scrim | 320 ms |
Rules: reduce motion (OS or setting) snaps every value to its target and keeps progress live.
Nothing loops except the buffering ring. An idle window subscribes to no frames: widgets ask for
a redraw only while a value is still moving. F3 shows frame times.

## 5. Account
Avatar slot in the centre top bar: "Sign in" pill when signed out, the account picture when signed
in. The sheet (radius 28, `surface-container-high`, scrim 60%) shows the `design.md` section 11
texts word for word: where data lives, the Terms of Service notice (always visible), then
"Sign in with Google" (neutral tonal button, no logo) which calls `AccountService::sign_in`. While
it waits: a ring and "Finish signing in in the window that opened". "Paste cookie instead" opens a
masked field (`Secret`, never logged, cleared after use) and a Check button. Errors are plain
sentences (`copy::account_error`). Signed in: avatar, name, handle, and "Sign out" with the
confirmation text; the result line says what was removed. Sign-in stays disabled until the
first-run notice is accepted, with a button that reopens it.

## 6. What beats SimpMusic, and why
1. The chrome takes the cover's colours (nav pill, pane gradient, play button); SimpMusic's is grey.
2. Every hover, press, page change and pane toggle is animated with the Material springs; theirs
   snap. The play button morph is the visible Expressive touch.
3. One type family with tabular digits and a real scale; their text is 11 to 12 px throughout.
4. One icon family (Material Symbols Rounded) at consistent optical sizes, not three styles.
5. Cards carry title and artist on a scrim at 20 px radius with a lift; theirs are bare images.
6. The right pane is a layout, not an overlay: art, live line, tabs, quality badge, lyrics with
   provider and sync label. Quality and lyric attribution are honest, which SimpMusic does not show.
7. It works at 1366 x 697 without clipping and degrades to a rail and an overlay pane.
8. Idle cost is zero frames, so the glass capsule and the animation do not tax a laptop battery.

## 7. Not done, said plainly
Live backdrop blur (tier Low only), repeat and like (no player or catalog command yet), a mood
chip row (no mood data), shape morph of cards (the play button is the only morph), Windows
high-contrast mode detection. See the final report for measured frame times.
