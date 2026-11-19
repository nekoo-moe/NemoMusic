# Skeleton loading: requirements

## Goal

When a page waits for its first data (search with no answer yet, Home, Explore, Library, the sidebar library list, album, playlist and artist pages), it shows a skeleton frame of soft blocks in the shape of the real content, with a calm shimmer, instead of an empty page or a bare "Loading" line. A fast answer never flashes the skeleton, data already on screen is never replaced by it, and errors and empty answers keep their plain sentences.

Owner request (2026-11-19): "When search has no results yet, or parts have not fetched their data, replace the empty page with a skeleton loading frame."

## User Stories

- As a listener, I want to see the outline of the page while it loads, so that the window never looks broken or empty.
- As a listener, I want a quick answer to appear without a flash of grey blocks, so that the app feels calm.
- As a listener, I want what is already on screen to stay there while it refreshes, so that nothing jumps or blanks.
- As a listener who turned on Reduce motion, I want still blocks without a moving shimmer.
- As a laptop user, I want a loading window to cost no CPU when it is hidden or not focused.

## Acceptance Criteria

1. WHEN a search text has no answer yet (the debounce or the request is running) and no earlier answer is on screen, THE SYSTEM SHALL show a skeleton Top result and skeleton sections under the real filter chips.
2. WHEN a filter chip is picked and its answer is pending, THE SYSTEM SHALL show the skeleton of that kind under the real chips.
3. WHEN Home has no cached shelves, daily mixes, recent songs or local albums and its first load runs (also the first load after a sign-in or an account switch), THE SYSTEM SHALL show a skeleton hero, chip row and shelves.
4. WHEN Explore has no answer yet and its first load runs, THE SYSTEM SHALL show skeleton sections.
5. WHEN the account library loads (signed in, no library on screen, no failure), THE SYSTEM SHALL show skeleton rows in the Library page and the sidebar `Your library` list.
6. WHEN an album, playlist or artist page loads, THE SYSTEM SHALL show a skeleton header and rows in the layout of that page.
7. WHEN data is cached for a surface, THE SYSTEM SHALL keep it on screen and never draw a skeleton for it; the existing small refresh state stays.
8. WHEN a load fails, THE SYSTEM SHALL show the existing plain sentence with Retry; WHEN a load answers with nothing, THE SYSTEM SHALL show the existing empty state; neither shows a skeleton.
9. WHEN a skeleton is wanted, THE SYSTEM SHALL show it only after 150 ms and keep it for at least 300 ms once shown, then cross-fade into the content over `motion.base` (200 ms).
10. WHILE a skeleton is visible, the window is focused, the page is on screen and Reduce motion is off, THE SYSTEM SHALL sweep one soft band left to right every 1.4 s on the app's one frame clock; otherwise it SHALL draw no frames for it (the 150 ms delay and the 300 ms hold are the only frame requests of a hidden-skeleton state).
11. WHEN Reduce motion is on, THE SYSTEM SHALL draw still blocks and swap to the content without a fade.
12. THE SYSTEM SHALL draw every skeleton shape with the same outer size as the real shape it stands for, so content arrives without a layout jump.
13. THE SYSTEM SHALL colour skeleton blocks from `surface-container-high` at low alpha, with theme tokens only.
14. THE SYSTEM SHALL leave the lyrics loading line, the player bar, the splash and the setup flow unchanged.

## Out of Scope

- Lyrics loading, the player bar, the Appearance page, the splash and setup flow, audio.
- A channel page: the app has no channel route yet, so there is nothing to load (see Open Questions).
- Changing what the app fetches, when it fetches, or how errors are worded.
- Skeletons for Settings, Queue, Now Playing and mood pages.

## Open Questions

- Channel pages (owner list): the app has no channel route or page today, only a Channels search kind with no section. Is a skeleton for a future channel page enough, or should a channel page be a separate spec? Assumed: the kit already covers it (a header and rows), nothing is built now.
- After a sign-in the Home shelves of the signed-out visit stay cached, so no skeleton shows there; only an empty Home shows one. Assumed this matches "cached data stays on screen".
