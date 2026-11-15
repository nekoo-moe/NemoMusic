# Search results that match YouTube Music: requirements

Status: pre-approved by the owner on 2026-11-15 (the owner delegated the design).

## Goal

Searching YouTube Music in NemoMusic shows what music.youtube.com shows for the same query: the Top result card first (for "HVL": the album "HVL" by RPT MCK, with Play and Shuffle), then the results grouped by kind in the order YouTube Music ranked them. Songs from an unrelated artist whose album title happens to match the query no longer fill the first section.

Owner report (2026-11-15, Linux, signed in, Vietnamese UI): for "HVL", NemoMusic showed a "Songs" section with one RPT MCK song and many songs by "An Nhiên" (album "HVL"), then "Albums", and no top result.

Root cause (checked in the code and with one anonymous request on 2026-11-15):
- `nemo-ytm` `catalog.rs` (anonymous) and `session.rs` (signed in) both ran four filtered searches (`search_songs`, `search_albums`, `search_artists`, `search_playlists`) and joined them. The songs filter ranks songs by text match, so every song on an album named "HVL" ranks high. YouTube's unfiltered search, which holds the top result and the ranking across kinds, was never asked.
- `web_search.rs` calls the same music search for all-of-YouTube search, so it had the same cause.
- `SearchResults` had no place for a top result or an order, and the Search screen drew a fixed order (Songs, Albums, Artists, Playlists).
- `nemo-app` `merged.rs` dropped the `videos` and `channels` of the remote answer.

## User Stories

- As a listener, I want the best match shown first, so that the album or artist I typed starts with one click.
- As a listener, I want the result kinds in YouTube Music's order, so that NemoMusic and the website agree.
- As a listener, I want Play and Shuffle on a top album or playlist, so that I need not open its page first.

## Acceptance Criteria

1. WHEN the user searches YouTube Music (signed in or not) THE SYSTEM SHALL send one unfiltered `youtubei/v1/search` request with only the query, through the client the catalog already uses (the session cookie when signed in).
2. WHEN the answer holds a top-result card (`musicCardShelfRenderer`) THE SYSTEM SHALL return it as `SearchResults.top` (song, album, artist, playlist or video) and not repeat it in the lists.
3. WHEN the answer lists results THE SYSTEM SHALL record the kinds in the order they first appear (`SearchResults.order`) and keep each list in YouTube's order.
4. WHEN a result row is read THE SYSTEM SHALL classify it by its navigation endpoint (page type, id prefix, music video type) and never by its text, so a localized answer (Vietnamese) gives the same result.
5. WHEN the answer layout is not recognised, or the request is refused for a reason other than the network, THE SYSTEM SHALL fall back to the four filtered searches; WHEN the network fails THE SYSTEM SHALL return the error without another request.
6. WHEN results are shown THE SYSTEM SHALL draw the Top result card first (cover, title, `Type · artist` line; Play and Shuffle for an album or playlist; Play for a song or video; an artist opens its page), then the sections in `SearchResults::sections()` order.
7. WHEN results are shown THE SYSTEM SHALL keep the existing track rows, their quality labels and the songs limit, and SHALL never label a YouTube item lossless.
8. WHEN local and YouTube Music results are merged THE SYSTEM SHALL keep YouTube Music's top result and order, and keep its videos and channels.
9. WHEN other crates build `SearchResults` THE SYSTEM SHALL keep them building with `..SearchResults::default()` (additive change).

## Out of Scope

- Filter chips (All, Songs, Albums, Artists, Playlists, Videos): `Catalog::search_scoped` takes only `SearchScope::{Music, AllOfYoutube}`, so there is no kind filter to call. Follow-up task in `tasks.md`.
- Podcasts, episodes and user profiles in search results: `SearchResults` has no field for them; they are skipped.
- Play counts on song rows ("23M plays"): `Track` has no field for them (`nemo-core/src/track.rs` belongs to another lane).
- Paging past the first answer ("Show all" continuations).

## Open Questions

- None blocking. Follow-ups for the owner: filter chips need a kind filter in `SearchScope` (public API change); podcasts and profiles in search need new `SearchResults` fields.
