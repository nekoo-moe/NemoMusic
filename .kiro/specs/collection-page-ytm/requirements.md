# Collection page in the YouTube Music layout: requirements

Owner pre-approved 2026-11-15 (UI polish wave). Owner request: "the playlist page background is not good, see how YTM does it and copy it" (screenshot of playlist "Nồi cám heo.").

## Goal
Album and playlist pages look like YouTube Music's: a tall gradient from the cover's colour behind the page, a centred column with the cover, title, owner, meta and round actions, and the track list beside it on wide windows. Every action, the keyboard order and the honest quality labels stay.

## User Stories
- As a listener, I want an album or playlist page to take the colour of its cover, so that the page feels like that record.
- As a listener on a wide window, I want the cover and the buttons to stay in view while I scroll the songs, so that I can play or shuffle at any time.
- As a listener on a small window, I want the same page stacked in one column, so that nothing is cut off.

## Acceptance Criteria
1. WHEN an album or playlist page is open, THE SYSTEM SHALL draw a tall gradient from the page cover's colour (seed from the cover worker) that is darker toward the bottom and fades into the pane, and fade it in with the page entrance.
2. THE SYSTEM SHALL keep every text ink at 4.5:1 or more and the Play disc at 3:1 or more over that gradient, for any cover seed, in dark, AMOLED and light modes.
3. WHILE the window is at least 1280 px wide and the content column at least 640 px wide, THE SYSTEM SHALL show two columns: a centred left column (38 % of the content, 260 to 360 px) and the track list; each column scrolls on its own. Otherwise THE SYSTEM SHALL stack them in one scrolling column.
4. THE SYSTEM SHALL show in the left column: the page cover (or a 2 x 2 mosaic of the first four song covers when the page has none), the title, the owner (album artists or the playlist author) when known, a kind line (`Album • 2023`, `Playlist`) and a count line (`12 songs • 43 min`), and round Shuffle and Play buttons.
5. THE SYSTEM SHALL show each playlist row with its cover, the title, `Artist • Album` (the album left out when unknown) and the duration; album rows show the position and the artists. The source or quality badge shows when the row is at least 560 px wide; the playing row's badge comes from the signal path, so a YouTube stream is never labelled lossless.
6. THE SYSTEM SHALL keep Play (from the first song), Shuffle, row press (play from that row), the row menu (button and right click), Show more, and the Tab order: Shuffle, Play, then the rows.
7. THE SYSTEM SHALL never show a field the catalog does not give (views, privacy, year of a playlist, owner avatar).
8. THE SYSTEM SHALL draw covers only through the existing art widget (never upscaled).
9. WHILE the page loads, THE SYSTEM SHALL show a skeleton in the same layout.

## Out of Scope
- A Sort control over the list (see Open Questions).
- Edit, share and more buttons on the page: the app has no such collection actions yet.
- The Artist page (keeps its hero layout).
- Art widget internals (covers lane).

## Open Questions
- Sort: YouTube Music has a Sort control over the list. Playing from a row uses the list order (`PlayAt { index }`), so a sorted view needs a decision: does Play follow the sorted order, and is the sort remembered per page? Not built until the owner decides.
- Owner avatar, views, privacy and the playlist year are not in the catalog types (`nemo-core::catalog`); adding them is a `nemo-core` change for the search lane.
