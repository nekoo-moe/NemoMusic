# Search results that match YouTube Music: tasks

Status: pre-approved by the owner on 2026-11-15.

## Tasks

- [x] 1. Unfiltered search with the top result and the order of kinds
  - Crate and files: `nemo-core` (catalog.rs, catalog/search.rs), `nemo-ytm` (search.rs, search/parse.rs, search/tests.rs, catalog.rs, session.rs, web_search.rs, tests/fixtures/music/), `nemo-app` (merged.rs).
  - Does: `SearchKind`, `TopResult`, `SearchResults.{top, order}`; one raw `search` request for anonymous and signed-in catalogs, parsed by endpoint; filtered searches only as the fallback; the merge keeps top, order, videos and channels.
  - _Requirements: 1, 2, 3, 4, 5, 8, 9_
  - Checks: fmt, workspace clippy and tests; fixture tests in `nemo-ytm`.
  - Steering: code.md (tests use fixtures only), coc.md (no new host, human-paced).

- [ ] 2. Top result card and ranked sections on the Search screen
  - Crate and files: `nemo-ui` (gui/views/search.rs, gui/views/search/{top,section}.rs, gui/app.rs, gui/app/{handle,message,model,query}.rs, mock/catalog.rs), `docs/ui/snapshots/search-*.png`.
  - Does: the Top result card (Play and Shuffle for an album or playlist, Play for a song or video, an artist opens its page), then the sections in `SearchResults::sections()` order, with a Videos section of track rows.
  - _Requirements: 6, 7_
  - Checks: fmt, clippy, tests; design.md section 15 checklist; snapshot.
  - Steering: design.md.

- [ ] 3. Follow-up (owner to decide): filter chips All, Songs, Albums, Artists, Playlists, Videos
  - Needs a kind filter in `SearchScope` or a new `Catalog` method (public API change), mapped to the filtered searches or the chip `params` of the answer.
  - _Requirements: none yet_

## Definition of Done

See [workflow.md section 4](../../steering/workflow.md#4-definition-of-done).
