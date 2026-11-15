# Search results that match YouTube Music: design

Status: pre-approved by the owner on 2026-11-15.

## Overview

`nemo-ytm` asks YouTube Music's unfiltered search, the same request the website's search box sends, and reads the top-result card and the ranked results from the raw JSON. `nemo-core` `SearchResults` gains two additive fields, `top` and `order`. The Search screen draws the Top result card, then the sections in that order.

Live layout (one anonymous request for "HVL", 2026-11-15, `WEB_REMIX`, recorded in `crates/nemo-ytm/tests/fixtures/music/search-hvl.json`): `contents.tabbedSearchResultsRenderer.tabs[0].tabRenderer.content.sectionListRenderer` with a `chipCloudRenderer` header (filter chips with `searchEndpoint.params`), then a `musicCardShelfRenderer` (the top result, here the album "HVL" by RPT MCK, 2026, with Play and Shuffle `watchPlaylistEndpoint` buttons), then 24 `itemSectionRenderer` sections of one `musicResponsiveListItemRenderer` each, in YouTube's order: song, artist, songs, videos, artist, playlists, singles, profiles, episodes, podcasts. The older layout (titled `musicShelfRenderer` shelves and a card with `contents`) is read too.

Filter chips (task 3): a chip sends the same `search` request plus the chip's `params`, copied from the `chipCloudRenderer` of the live answer. The filtered answer (three recorded: Songs, Albums, Community playlists for "HVL") is one titled `musicShelfRenderer` of about 20 rows of that kind, which `parse_search` already reads; `search_kind` keeps only that kind's list. The UI keeps the unfiltered answer and the latest filtered answer apart (`SearchState.results` and `.filtered`), so switching back to All sends nothing. When the filtered layout is unknown or the request is refused, the typed filtered search of `ytmapi-rs` runs for songs, albums, artists and playlists; videos have no typed fallback and answer a parse error.

## Affected Crates

- `nemo-core`: `catalog/search.rs` (new: `SearchKind`, `TopResult`, `SearchResults::{count, is_empty, sections}`), `catalog.rs` (`SearchResults.{top, order}`).
- `nemo-ytm`: `search.rs` and `search/parse.rs` (new), `catalog.rs` and `session.rs` (call `search::search`), `web_search.rs` (keep music videos when web videos are added), fixture `tests/fixtures/music/`.
- `nemo-app`: `merged.rs` (merge `top`, `order`, `videos`, `channels`).
- `nemo-ui`: `gui/views/search.rs`, `gui/views/search/{top,section}.rs` (new), `gui/app/{handle,message,model,query}.rs` and `gui/app.rs` (`Message::ShuffleCollection`, `ListSource::SearchVideos`), `mock/catalog.rs` (mock top result for the snapshot).

## Data Flow

UI thread: `Catalog::search` future on the UI executor. `YtmCatalog` / `SessionCatalog` spawn the work on the app tokio runtime: one `POST music.youtube.com/youtubei/v1/search` through `ytmapi-rs` `raw_json_query` (anonymous visitor or the session cookie, same as every other raw call), then the pure `parse_search` on that task. The answer goes back through the join handle as `SearchResults`. No audio buffers are involved; nothing touches the audio thread. No new host: `music.youtube.com` is already on the egress list.

## Interfaces

```rust
// nemo-core::catalog
pub enum SearchKind { Songs, Albums, Artists, Playlists, Videos, Channels }
impl SearchKind { pub const DEFAULT_ORDER: [Self; 6]; }
pub enum TopResult { Song(Track), Album(AlbumSummary), Artist(ArtistSummary), Playlist(PlaylistSummary), Video(VideoSummary) }
pub struct SearchResults { /* existing fields */, pub top: Option<TopResult>, pub order: Vec<SearchKind> } // Default
impl SearchResults {
    pub fn count(&self, kind: SearchKind) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn sections(&self) -> Vec<SearchKind>; // non-empty kinds: `order` first, then DEFAULT_ORDER
}

// nemo-core::catalog (filter chips, task 3)
impl SearchKind { pub const FILTERS: [Self; 5]; } // Songs, Videos, Albums, Artists, Playlists
trait Catalog { fn search_kind<'a>(&'a self, query: &'a str, kind: SearchKind) -> BoxFuture<'a, Result<SearchResults, CatalogError>>; } // default: Unavailable("search filter")

// nemo-ytm (crate-private)
pub(crate) struct SearchQuery; // PostQuery, path "search", body {"query"}
pub(crate) async fn search<A: AuthToken>(api: &YtMusic<A>, query: &str) -> Result<SearchResults, CatalogError>;
pub(crate) fn parse_search(json: &str) -> Option<SearchResults>;
pub(crate) fn filter_params(kind: SearchKind) -> Option<&'static str>; // chip params as served
pub(crate) async fn search_kind<A: AuthToken>(api: &YtMusic<A>, query: &str, kind: SearchKind) -> Result<SearchResults, CatalogError>;

// nemo-ui
Message::ShuffleCollection(DetailKey);
ListSource::SearchVideos;
Event::SearchFilter(Option<SearchKind>); Event::SearchFilterDone { query, kind, result };
Effect::RunFilteredSearch { query, kind }; Job::SearchKind { query, kind };
SearchState::{filter, filtered, pick, filter_done, filtered_now, filter_loading, shown};
```

## Error Handling

- Network failure of the unfiltered request: `CatalogError::Network` (fixed text), no fallback request.
- Refusal (HTTP error code) or an unknown layout (`parse_search` gives `None`): a `warn` log with fixed text, then the four filtered searches as before. No query, URL or body is logged.
- Rows without a valid video id, title or browse id are dropped (`map_song`, `map_album`, ... as everywhere else); greyed-out rows are dropped.
- At most 50 results kept per kind.

## Testing Strategy

- `nemo-ytm/src/search/tests.rs`: the recorded "HVL" fixture (top album, order, lists, skipped profiles, episodes and podcasts); a Vietnamese type label is not taken as an artist; the older shelf layout and card rows; unknown layout gives `None`, an empty list gives empty results; greyed-out rows and bad ids; the request body holds only the query.
- `nemo-core/src/catalog/search.rs`: `sections()` order and `is_empty()` with only a top result.
- `nemo-app/src/merged.rs`: the remote top result and order survive the merge.
- UI: the `snapshots` example renders `search-*` with the mock top result (`docs/ui/snapshots/search-*.png`).
- No audio code changed: no bit-exact test needed.

## Risks and Alternatives

- YouTube changes the layout: the reader classifies by endpoint, reads both known layouts and falls back to the filtered searches when the section list is missing.
- Fewer songs than before (the unfiltered answer lists about five): the screen showed at most 10 anyway. A "Show all songs" through the filtered call is a possible follow-up.
- Alternative: one mixed list in YouTube's exact item order instead of sections. Rejected for now: it needs a new list type and row kinds in the UI, while sections in YouTube's order keep the existing rows and cards.
- Alternative: `ytmapi-rs` typed unfiltered `search`: deprecated in 0.3.4, and it returned nothing anonymously (comment in the old `catalog.rs`).
