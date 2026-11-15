# Search results that match YouTube Music: tasks

Status: pre-approved by the owner on 2026-11-15.

## Tasks

- [x] 1. Unfiltered search with the top result and the order of kinds
  - Crate and files: `nemo-core` (catalog.rs, catalog/search.rs), `nemo-ytm` (search.rs, search/parse.rs, search/tests.rs, catalog.rs, session.rs, web_search.rs, tests/fixtures/music/), `nemo-app` (merged.rs).
  - Does: `SearchKind`, `TopResult`, `SearchResults.{top, order}`; one raw `search` request for anonymous and signed-in catalogs, parsed by endpoint; filtered searches only as the fallback; the merge keeps top, order, videos and channels.
  - _Requirements: 1, 2, 3, 4, 5, 8, 9_
  - Checks: fmt, workspace clippy and tests; fixture tests in `nemo-ytm`.
  - Steering: code.md (tests use fixtures only), coc.md (no new host, human-paced).

- [x] 2. Top result card and ranked sections on the Search screen
  - Crate and files: `nemo-ui` (gui/views/search.rs, gui/views/search/{top,section}.rs, gui/app.rs, gui/app/{handle,message,model,query}.rs, mock/catalog.rs), `docs/ui/snapshots/search-*.png`.
  - Does: the Top result card (Play and Shuffle for an album or playlist, Play for a song or video, an artist opens its page), then the sections in `SearchResults::sections()` order, with a Videos section of track rows.
  - _Requirements: 6, 7_
  - Checks: fmt, clippy, tests; design.md section 15 checklist; snapshot.
  - Steering: design.md.

- [x] 3. Filter chips under the search box (owner said yes, 2026-11-15)
  - Crate and files: `nemo-core` (catalog.rs `Catalog::search_kind`, catalog/search.rs `SearchKind::FILTERS`), `nemo-ytm` (search.rs, search/tests.rs, catalog.rs, session.rs, account/catalog.rs, web_search.rs, tests/fixtures/music/search-hvl-{songs,albums,playlists}.json), `nemo-app` (merged.rs), `nemo-ui` (search.rs, state.rs, state/event.rs, gui/app/{dispatch,job,message,query}.rs, gui/views/search.rs, gui/views/search/{filter,section}.rs, gui/headless.rs, mock/catalog.rs, examples/snapshots/screen.rs), `docs/ui/snapshots/search-*.png`.
  - Does: chips All, Songs, Videos, Albums, Artists, Community playlists; a chip runs one search with YouTube Music's own chip `params` and shows that kind in full ("Show more" on long song lists); typing a new query returns to All.
  - _Requirements: 10, 11, 12, 13_
  - Checks: fmt, workspace clippy and tests; fixture tests of the filtered answers; snapshots `search-songs-*`, `search-albums-*`.
  - Steering: design.md (chips), code.md (fixtures only), coc.md (one request per chip, no new host).

- [ ] 4. Follow-up: Podcasts, Episodes and Profiles chips
  - Needs additive `SearchResults` fields (`podcasts`, `episodes`, `profiles`) and `SearchKind` variants, their sections, and a page for user profiles (`MUSIC_PAGE_TYPE_USER_CHANNEL`), which the app cannot open yet. The chip `params` are recorded in the fixture README.
  - _Requirements: none yet_

## Definition of Done

See [workflow.md section 4](../../steering/workflow.md#4-definition-of-done).
