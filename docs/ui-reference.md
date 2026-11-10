# NemoMusic UI reference

Researched 2026-11-10. Input to UI agents. Binding rules stay in `.kiro/steering/design.md`; this file only proposes. Tags: (S) public source, (E) estimate from memory of screenshots, (U) unverified. Content paraphrased; no assets copied.

## 1. Survey (what public sources confirm)
Public pages give almost no pixel values. Every number below is (E) or (U) unless a URL backs it.

| App | Confirmed facts (source) | Numbers |
|---|---|---|
| Spotify desktop | Library sidebar has collapsed (art only), regular and expanded widths, drag to resize ([community](https://community.spotify.com/t5/Your-Library/Desktop-New-Your-Library-sidebar/td-p/5571384)). Now Playing View opens as a resizable right sidebar ([community](https://community.spotify.com/t5/Desktop-Windows/Desktop-New-quot-Now-Playing-View-quot-sidebar/m-p/5605345)). Bottom bar with art left, controls centre, volume/queue right (E). | Player bar 90 px appears as a variable in a third-party theme ([spicetify Matte](https://github.com/spicetify/spicetify-themes/blob/master/Matte/README.md)), stock value (U). Sidebar ~280 regular, ~72 collapsed (E). Cards ~180-200 wide, gap 16-24 (E). |
| Apple Music (macOS Tahoe) | Control bar moved to the bottom and is translucent; cover art scrolls behind it and can hurt icon legibility ([Apple forum](https://discussions.apple.com/thread/256142581)). MiniPlayer window exists ([Apple forum](https://discussions.apple.com/thread/256279613)). | Sidebar ~220-260 (E). |
| Apple Music iOS 26 | Floating glass tab bar, easier search, content-focused browsing ([overview](https://www.peepstune.com/heres-everything-new-for-apple-music-in-ios-26/)). Mini player docks above the tab bar, bar shrinks on scroll, mini player expands to full Now Playing by matched-geometry zoom ([sample code](https://github.com/ahmetbostanciklioglu/AppleMusicBottombarSwiftUI)). Glass guidance: [Apple newsroom](https://apple.com/newsroom/2025/06/apple-introduces-a-delightful-and-elegant-new-software-design). | Apple publishes no glass numbers (design.md header). |
| Apple Music web | Not fetched. | (U) |
| YouTube Music web | Not fetched (no public layout source). Known from use: left guide, top search, bottom player bar, Now Playing with Up next / Lyrics / Related tabs. | All (U) |
| Tidal, Qobuz | Not fetched. Tidal is cited by Plexamp users as a desktop reference ([forum](https://forums.plex.tv/t/revamp-the-artist-album-and-full-player-for-desktop-users/879297)). | All (U) |
| Plexamp | Desktop gained sidebar, mini player, context menu; keeps mobile layout when narrow ([forum](https://forums.plex.tv/t/happy-to-see-plexamp-desktop-is-starting-to-look-like-a-desktop-player/944083/1)). Users ask for Apple-Music-like smooth lyrics ([forum](https://forums.plex.tv/t/make-the-plexamp-ui-more-like-apple-music-mod-lyrics-display/895746)). | (U) |
| Cider | Sidebar playlists show artwork ([devlog](https://cidercollective.itch.io/cider/devlog/1014454/cider-v31-solarium)); Immersive mode with gradient-mesh background, lyric translations ([changelog](https://cider.sh/changelogs/4.0.30)). | (U) |
| SimpMusic desktop | Three Now Playing styles: Classic, Material 3 Expressive, Apple Music; Apple-style lyrics with word-by-word glow and depth blur; desktop renders glass surfaces including a floating capsule player unconditionally; accent source Default / Wallpaper / Custom, default seed `#8ECAE6`, dark default ([settings](https://www.simpmusic.org/docs/guide/settings), [v2.0.0](https://www.simpmusic.org/blogs/new-update-simpmusic-v200)). | Capsule size (U). |
| Feishin / Navidrome | Feishin: full-screen player with a lyrics tab, line-animated timestamped lyrics, unsynced fallback ([feishin.net](https://feishin.net/)). Navidrome: web UI ([docs](https://navidrome.org/docs/overview)). | (U) |

## 2. What the apps share (pattern, not pixels)
- Left navigation (collapsible to icons) + one scrolling content column + bottom player. Now Playing is a separate mode, entered from the player art.
- Art dominates: hero covers 240-400 px (E), grid covers 160-200 px (E), 8-16 px radius (E).
- Lyrics: one large active line, dimmed neighbours, blurred cover backdrop (SimpMusic, Apple, Cider pattern).
- Glass appears only on the bottom bar / tab bar / mini player (Apple iOS 26, SimpMusic). Same layer rule as design.md section 1.
- Colour from cover: accent from the dominant colour, surface tinted (Cider gradient mesh, SimpMusic seed). Known risk: translucent bars over busy art hurt icon legibility (Apple forum above), which design.md section 6 already guards.

## 3. Responsive grid (4 px base, matches design.md section 4)
Rule: sidebar 240 at W >= 1100, else 64. Padding 24. Content width C = min(1200, W - sidebar - 48 - (queue 320 if open)). Gap 16 (W < 1600) or 24 (W >= 1600). Columns n = max(2, floor((C + gap) / (160 + gap))). Card width = (C - gap(n-1)) / n, rounded down to 4 px. Card = art square + 8 + title (14/20, 600) + subtitle (12/18).

| Window W | Sidebar | C | Gap | Columns | Card art |
|---|---|---|---|---|---|
| 960 | 64 | 848 | 16 | 4 | 200 |
| 1100 | 240 | 812 | 16 | 4 | 188 |
| 1280 | 240 | 992 | 16 | 5 | 184 |
| 1280 + queue | 240 | 672 | 16 | 3 | 208 |
| 1600 | 240 | 1200 | 24 | 6 | 180 |
| 1920 | 240 | 1200 | 24 | 6 | 180 |
| 2560 | 240 | 1200 | 24 | 6 | 180 |

Rows: track row 48 (compact 40). Hero art 320; Now Playing art up to 480. Bottom scroll padding = 88 + 12 + 12 = 112 so the floating bar never hides the last row.

## 4. Wireframes (dimensions in px, W x H)
### Shell, 1280 x 800
```
+--------+--------------------------------------------------+
|S 240   | Top bar 56: [< >] [ Search 480 ]          [avatar]|
| Logo56 +--------------------------------------------------+
| Home   | Content: padding 24, max 1200, scrolls           |
| Search |                                                  |
| Library|                                                  |
| ------ |                                                  |
| Playl. |                                                  |
|  (rows | 12 inset                                         |
|   40)  | +----------------------------------------------+ |
|        | | Player bar 88, glass, radius 28 (W-24 wide)  | |
+--------+-+----------------------------------------------+-+
```
Player bar columns: art 64 + title/artist (280) | transport centre, seek bar below | badge, volume, queue (280). Bar floats over content (12 inset); sidebar goes to the window bottom.

### Home / Search results, content column 992
```
Greeting headline-md 28/36                         (h 36)
24 gap
Shelf title title-md 20/28 ........ [Show all]     (h 28)
12 gap
[card184][card184][card184][card184][card184]      16 gap
 art 184 sq, radius 12; title 14/20; sub 12/18     (h 184+8+20+18)
32 gap between shelves
Search: chips row 40 | Top result 320x184 card | Songs list rows 48 (4) | Albums / Artists shelves (artists round)
```

### Album page
```
+------------------------------------------------------------+
| backdrop: blurred cover >=60 px, overlay >=0.60 (hero only)|
| [Cover 320 sq r16]  32  ALBUM label-sm                      |
|                         Title display-md 40/48 (500)        |
|                         Artist title-sm . year . n songs    |
|                         [Play 48 filled] [Shuffle] [..]      |
+------------------------------------------------------------+
| # | Title / artist          | Format badge | time    row 48  |
| tabular numerals, hover state layer 8%, no zebra            |
```
Content max 1200; hero height 320 + 2x24.

### Now Playing, 1280 x 800 (art + lyrics)
```
+----------------------------------------------------------+
| header 56: [v close]                [lyrics offset][..]  |
|  +---------------------+   +----------------------------+ |
|  | Art 480 sq r16      |   | Lyrics column 640 max      | |
|  | (min(480, H-280))   |   | active 28/36 600 @35% H    | |
|  | Title 28/36         |   | neighbours 20/28, rest 16  | |
|  | Artist 16/24        |   | footer: Lyrics: X . Synced | |
|  | seek + transport    |   +----------------------------+ |
|  +---------------------+      gap 64                      |
+----------------------------------------------------------+
```
Backdrop blurred cover; no glass behind lyrics. Floating bar hidden here; transport lives under the art.

### Now Playing compact (W < 1100 or H < 700)
```
+-------------------------------+
| header 48                     |
| Art 280 sq (centred, r16)     |
| Title / artist (centre)       |
| seek 4 high (hit 40)          |
| transport row 56              |
| [Lyrics | Queue] tabs 40      |  tab swaps lower area (lyrics shows 3 lines)
+-------------------------------+
```
At 960 x 600 art = 240.

### Queue panel (right, 320)
```
+---------------------+
| Queue title-md   [x]| 56
| Now playing row 56  |
| Next up label-sm    |
| rows 48 (art 40)    | drag handle, remove
| Clear (text button) |
+---------------------+
```
Opaque `surface-container`, not glass; pushes content (C shrinks), never overlays under 1280.

### Settings, content 720 max
```
Left column 200: Account, Playback, Audio output, Lyrics, Appearance, Privacy, About
Right: section title headline-md; rows 56 (label 14/20 + helper 12/18 left, control right 40);
group card surface-container, radius 12, gap 24.
```

## 5. Ten styling rules for a premium feel (each checkable)
1. Spacing rhythm: only 4/8/12/16/24/32/48/64; shelf gap 32, header-to-row 12. Lint the theme module for other values.
2. Art scale: hero >= 320, Now Playing >= 400 at H >= 800; art is the largest element on every screen (compare bounding boxes).
3. Type contrast: title 600 vs meta 400 with >= 4 px size step and `on-surface-variant` for meta; max 2 weights per view.
4. Tabular numerals on every time, count, rate; verify no layout shift while a timer ticks.
5. One accent: a single derived palette on screen, applied to play button, seek fill, focus ring only; crossfade 400 ms.
6. Glass restraint: glass area <= 35% of window; exactly one glass layer (bar). Screenshot test at 1280 x 800.
7. Radius hierarchy: art 12-16, bar 28, pills full; nested radius = outer - padding.
8. Motion with purpose: only springs/fades from design.md section 8; hover to feedback < 100 ms; no loop. Idle renders 0 frames.
9. Content-first scroll: bottom padding 112; header collapses (title moves to top bar) after 64 px scroll, no jump.
10. Calm density: max 1 primary button per screen region, <= 6 columns, text rows one-line ellipsis; no badges or counters.

## 6. Eight things to avoid
1. Glass or translucency behind lists, cards, or lyric text (legibility failure seen on Apple's translucent bar).
2. Stacked glass (popover from the bar must be opaque).
3. Upscaled or stretched covers; non-square crops without `cover` rule.
4. Text over cover art without a scrim; accent colour as text colour inside glass.
5. Autoplaying or looping motion (visualizers, marquees, pulsing badges).
6. "Lossless / Hi-Res / HD" labels on YouTube streams, or a green success colour on lossy.
7. Dense recommendation shelves, counters, "For you" feeds that fight the calm principle.
8. Fixed pixel layouts that break at 150/200% DPI or 960 px width; hover-only actions with no keyboard path.

## 7. Recommended changes to design.md (not applied)
- Section 4: replace fixed `card` 160 with "min 160, flexible to the width formula in this file's section 3 (about 180-208)", and add the column rule and the 24 px gap at W >= 1600.
- Section 4: state that the sidebar is opaque `surface-container-low`; the glass "navigation rail overlay" in section 1 applies only to the collapsed overlay when width < 1100.
- Section 9: add `art.now-playing` 480 (400 min, shrink to fit H - 280) and `art.compact` 280.
- Section 4: add Now Playing compact breakpoint (W < 1100 or H < 700) and the tab swap for lyrics/queue.
- Section 4: add content bottom padding 112 so the floating bar never covers rows.
- Section 4: queue panel pushes content when W >= 1280, overlays (opaque) below.
- Section 3: add `headline-lg` 32/40 or state album title uses `display-md` 40/48 at weight 500 (currently implied only).
- Section 6: note that SimpMusic ships glass off by default on mobile and always-on on desktop; NemoMusic keeps Tier Low as the shipped default until the prototype gate passes.

## 8. Unverified items
Pixel sizes for Spotify, Apple, YouTube Music, Tidal, Qobuz, Plexamp, Cider, Feishin; YouTube Music, Tidal, Qobuz and Apple Music web were not fetched (no public layout source found). All Section 3-4 numbers are NemoMusic choices derived from design.md, not measurements of other apps.
