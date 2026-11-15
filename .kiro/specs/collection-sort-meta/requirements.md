# Collection Sort and header facts: requirements

Owner answers of 2026-11-15 to the open questions of `collection-page-ytm` (pre-approved; UI polish wave).

## Goal
Album and playlist pages get YouTube Music's Sort control and its header facts. The user can show the songs by title, artist, album or length. Play and Shuffle follow the order on screen, and the page remembers the choice. The header shows owner picture, privacy, year, views, track count and length when the catalog has them, and hides each one it does not have.

## User Stories
- As a listener, I want to sort a long playlist by title or artist, so that I can find a song and play from it.
- As a listener, I want Play to follow the order I see, so that the queue matches the page.
- As a listener, I want each playlist to keep the order I picked, so that I do not pick it again.
- As a listener, I want the playlist header to say who made it, how many tracks it has and how long it is, so that the page reads like YouTube Music.

## Acceptance Criteria
1. THE SYSTEM SHALL show a `Sort` pill above the track list of every album and playlist page. It opens a menu of Default order, Title, Artist, Album (playlists only) and Duration. The pill names the order in use (`Sort: Title`).
2. WHEN the user picks an order, THE SYSTEM SHALL reorder the rows at once (stable, case-insensitive, missing values last) and close the menu. Esc, the pill and leaving the page also close it.
3. WHEN the user presses Play, a row, or Shuffle on a sorted page, THE SYSTEM SHALL queue the songs in the shown order. Row `k` starts at the `k`-th shown song, and Shuffle draws from the same songs. Play from a Home card of that page uses the same order.
4. THE SYSTEM SHALL remember the order per page in the existing settings file (`settings.json`, field `collection_sorts`). It stores no entry for the default order and keeps at most 300 pages.
5. THE SYSTEM SHALL read from the album and playlist page header, when present: owner name and picture (the playlist facepile, the album strapline), privacy (subtitle or the edit header of one's own playlist), year, view count, track count and the length as written.
6. THE SYSTEM SHALL show `Playlist • Public • 2026` and `411 views • 249 tracks • 13 hours` (`songs` on albums), with the owner picture before the owner's name. Every missing fact is left out.
7. WHEN the header gives no length, THE SYSTEM SHALL sum the song lengths only when every song of the page was read. It never shows a count, a view number or a privacy the page did not give.
8. THE SYSTEM SHALL keep the new catalog fields additive (`CollectionMeta` on `AlbumDetail` and `PlaylistDetail`, all optional, with `Default`).

## Out of Scope
- "Newest first" and "Oldest first": they need the date each song was added, which the catalog does not give (YouTube Music shows them; the anonymous page has no dates).
- "Top voted" (collaborative playlists only).
- Edit, share and more buttons (no such actions exist).
- Sorting the Artist page.

## Open Questions
- Collaborative playlists show several avatars in the facepile; only the first is shown. Is that enough?
- Privacy shows only on one's own playlists (anonymous pages do not say it); the signed-in page was not captured as a fixture (it would carry the owner's data).
