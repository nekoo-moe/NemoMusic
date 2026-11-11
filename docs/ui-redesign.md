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
Capsule: bottom centre, 8 px above the window edge. While a song is loaded the three panes END above it (80 px reserve plus the pane gap), so the capsule never covers a row; scrollers keep 16 px of air. Checked at 1366 x 697 and 1920 x 1040.
Heights: at 697 the right pane shows art,
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
Round 4 (owner, 2026-11-11): the repeated "rise from the bottom" on every shelf and on scroll is
gone (`anim::SLIDE_PX` is 0). Each motion now has one job:
| Motion | What moves | Timing |
|---|---|---|
| Page cross-fade | the new page fades in and grows from 98% to 100%, no slide | 240 ms emphasized-decelerate |
| Entrance stagger | once per page load only, items 0 to 4, 28 ms apart; scrolling never replays it | 240 ms each |
| Open a card | the page header cover grows from 86% to 100% with the page fade (shared-element feel) | 240 ms |
| Background cross-fade | window and pane gradient (3 low-chroma tones of the song or the open page) | spring 150/1.0, about 0.7 s; snap on reduce motion |
| Cover breathe | Now Playing cover rests at 94% paused, 100% playing, on the play button spring | 1400/0.8 |
| Equaliser bars | 3 bars on the playing row in lists and the queue; still when paused or buffering | per frame while playing |
| Active lyric line | ink eases dim to bright, a faint accent glow fades in, the line grows to the active size | 240 ms, glide spatial default |
| Hover | cards lift 4 px with a shadow and the cover zooms 2.5% | spatial fast |
| Chips | a selection pill slides between chips (Home filters, Library tabs) | spatial fast |
| Queue reorder | the moved row and its neighbour slide to their new places | spatial default |
| State layers | hover 8%, press 10% fade over every button, chip, row, card | effects fast spring (about 60 ms) |
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
Nothing loops except the buffering ring and the equaliser bars while a song plays. An idle window
subscribes to no frames: widgets ask for
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

## 6a. Round 2 additions (owner review)
- Repeat: `RepeatMode {Off, All, One}` in nemo-core (`SetRepeat`, `PlayerSnapshot.repeat`, `RepeatChanged`).
  nemo-audio wraps (All) or replays (One) through the same gapless prefetch; Next and Previous stay manual. The
  capsule button is a tonal toggle (`secondary-container`) with a repeat-one glyph.
- Like: `Catalog::rate_song` (default: not available). nemo-ytm implements it with `ytmapi-rs` 0.3.4
  `YtMusic::rate_song`; the account-aware and merged catalogs forward it. The heart fills with the accent when
  liked, opens the sign-in sheet when signed out, and is disabled for local files.
- Text: `gui/textfit.rs` measures with the real advances of the bundled font, cuts at word ends and breaks card
  titles into two lines. The 0.55 em estimate is gone.
- Lyrics: no lyrics shows one empty state (icon, "No lyrics for this song", providers tried) and no footer. The
  right pane shows 5 to 7 lines with the spring glide and rests on a row edge.
- Right pane with nothing playing: recent songs, then one suggested song card from Home.
- Every button animates (Settings, sheets, menus, toast Undo too). Tab walks the controls; the focused one gets a
  2 px accent ring, and Enter or Space presses it.

## 6b. Round 4 additions (owner review 2026-11-11)
- Background: `scheme::bg_colors` turns the seed of the playing cover (or of the open album or
  playlist page, which wins while it is open) into three dark, low-chroma tones. The window draws
  them as a soft gradient and the panes sit on it as tinted translucent surfaces. Fixed themes
  keep their own seed. Text pairs are checked on the panes with `Palette::passes_on`.
- Home: `daily.rs` builds "Your daily mix", "Because you listened to <artist>", "Rediscover",
  "Mood of the day", "New for you", a time-of-day greeting and a hero card, seeded with the local
  date (stable within a day). Signed out, only the hero suggestion and catalog shelves show.
- Album and playlist pages draw a shimmer skeleton at once; the error page needs a failed call.
- Lyrics: the footer showed a provider switcher (from `LyricsService::lookup_all`, remembered
  with `choose`) when two or more sources answered; replaced in the lyrics wave (6c) by the
  Tools panel section. The credit line ("Lyrics from Unison · by …") shows whenever the shown
  lyrics carry one. Settings > Lyrics lists every provider with an on/off switch, its hosts and
  the risk note.

## 6c. Lyrics and song canvas wave (owner review 2026-11-11)
- Wrapping: lyric lines wrap (`Wrapping::WordOrGlyph`) in the right pane, the full screen and the
  compact layout, and the live line of the pane wraps too; nothing in a lyric line is cut with an
  ellipsis. `textfit::rows` counts the wrapped rows with the bundled font's advances; every row is
  that tall, and the reducer anchors the scroll on the same counts (`LyricsRows`). A line taller
  than the room under the 35% anchor moves up until its last row shows.
- Line change: the list glides on `spatial.default` (700/1.0, settles in about 250 ms); the old
  and the new active line cross-fade size, line height and ink over 240 ms
  (emphasized-decelerate). Reduced motion jumps. Manual scroll pause and the `Back to current
  line` pill are unchanged.
- Per-piece highlight (owner request, extends `design.md` section 10): for syllable- and
  word-timed lyrics the active line is rich text with one span per timed piece. Sung pieces are
  `on-surface` at 0.9; the piece being sung fades from the "coming" opacity to 1.0 over its own
  duration (at least 100 ms) instead of a fixed `effects.fast` step; pieces still to come stay
  at the "coming" opacity. Pieces of one word join without a separator. No bounce, no wipe.
  Line-timed lyrics keep the line highlight and its accent glow (piece-timed lines get no glow,
  it tinted the coming pieces).
- Lines around the active one use `on-surface` at the "coming" opacity (0.55 dark and AMOLED,
  0.72 light) instead of `on-surface-variant` at 0.72: the latter fails 4.5:1 on a coloured
  canvas and on light surfaces.
- Sources: no chip row under the lyrics. The best source is shown by itself (`lookup_all` is
  best first); the footer keeps `Lyrics: <provider> · <level>` (`Syllable synced`, `Word
  synced`, `Synced`, `Not synced`) and the credit (Unison submitter; "Lyrics via Better Lyrics"
  with its link host), both wrapped, never cut. The Tools panel has a "Lyrics source" section:
  `Best (auto)` (calls `clear_choice`) and every candidate with a level tag; the chosen row is
  filled, the shown one has a check; rows are 40 px buttons in the Tab order. The panel scrolls
  when it is taller than the window. Settings > Lyrics sorts sources by their `best` level
  (stable, chain order within a level) with a level tag.
- Song canvas: the cover worker tones the pre-blurred cover per base mode (`crate::canvas`):
  hue kept, chroma x1.7 (x1.2 light) capped at 0.10 (0.045 light), lightness squeezed into
  0.19-0.34 (Dark), 0.12-0.30 (AMOLED) or 0.90-0.96 (Light). A grey cover stays grey. The full
  screen draws the toned texture (no scrim needed; tests check every text ink on every pixel
  at 4.5:1 over black, white, grey, saturated, yellow and noisy covers); the right pane draws
  five top-to-bottom stops of it as a gradient, because tiny-skia cannot round-clip images.
  Both cross-fade with the cover in 400 ms, are static and work with every theme, AMOLED
  included. Tier Low only: no live blur.

## 7. Not done, said plainly
Lyrics wave: the credit link is shown as text (the app has no "open link" action yet); the
wrapped-row count measures with the widest bundled weight, so a line can get one spare row;
the canvas follows a base-mode switch at once while the neutrals cross-fade.
Round 4: with the lyrics switcher and the Unison credit shown, the small pane has room for about
3 lyric lines and the line under the active one is cut at the edge; the nav pill stays on Home
while an album opened from Home is shown; the equaliser bars are a state indicator, not audio
analysis; the mock lyric texts are identical for every provider.
The last lyric line of the small pane can still be cut at its bottom edge; sliders, toggles, checkboxes and text
fields keep iced's instant hover; the focus ring is clipped inside scroll areas and focus does not scroll a control
into view; a song's like state is known only from the Liked songs shelf and this session.
Earlier list:
Live backdrop blur (tier Low only), repeat and like (no player or catalog command yet), a mood
chip row (no mood data), shape morph of cards (the play button is the only morph), Windows
high-contrast mode detection. See the final report for measured frame times.
