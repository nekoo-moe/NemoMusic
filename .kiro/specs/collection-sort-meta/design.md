# Collection Sort and header facts: design

## Overview
- **Sort (pure):** `nemo-ui/src/collection_sort.rs` holds `CollectionSort`, `order(tracks, sort) -> Vec<usize>` (a stable index permutation), `sorted`, `page_key` (`album:<id>`, `playlist:<id>`) and `remember` (no entry for Default, at most 300 pages).
- **Play order:** `App::tracks_of(ListSource::Detail)` returns the album or playlist songs in the remembered order. The page draws row `k` from `order[k]` and sends `PlayAt { index: k }`, so the queue is the shown order. Shuffle, Play and the Home card's Play all go through `tracks_of`.
- **Memory:** `UiSettings.collection_sorts` (page key to order) is saved through the existing `on_settings` callback. `nemo-app` stores it in `settings.json` as `collection_sorts` (page key to label); unknown labels are dropped on load. No new file.
- **Control:** `gui/views/detail/sort_menu.rs` draws the pill (`surface-container-highest`, `shape.full`, 40 px) and, while open, an opaque menu (`surface-container-high`, `shape.md`, 200 px, 40 px entries; the selected one on `secondary-container`). The menu opens in the list's flow, never over rows. `App.sort_menu` holds the open page. It closes on a pick, the pill, Esc and a route change.
- **Header facts:** `nemo-core::catalog::CollectionMeta { owner, owner_avatar, privacy, year, view_count, track_count, duration_text }` is a field `meta` on `AlbumDetail` and `PlaylistDetail`. `nemo-ytm/src/feeds/header_meta.rs` reads it from `musicResponsiveHeaderRenderer`:
  - the owner from `straplineTextOne` + `straplineThumbnail` (albums) or `facepile.avatarStackViewModel` (playlists);
  - privacy and year from `subtitle`, or privacy from `musicPlaylistEditHeaderRenderer.privacy`;
  - views, count and length from `secondSubtitle` (`437K views` gives 437 000, `30 tracks`, `36 minutes`, kept as text).
  The typed `ytmapi-rs` album path fills the artist picture, the count and the length from `GetAlbum`.
- **Lines:** `gui/views/detail/meta_line.rs` writes the two lines (`kind_line`, `count_line`, `short_count`).

## Affected Crates
- `nemo-core`: `catalog/meta.rs` (new), `catalog.rs` (field `meta`).
- `nemo-ytm`: `feeds/header_meta.rs` (new), `feeds/playlist.rs`, `feeds.rs`, `pages.rs`, `catalog/{raw,adapt,map}.rs`, `offline/catalog.rs`; fixtures `tests/fixtures/music/browse-{playlist,album}.json`.
- `nemo-library`, `nemo-app` (`merged.rs`, `config.rs`, `ui_bridge.rs`): the new field and the stored sorts.
- `nemo-ui`: sort, view, mock data.

## Data Flow
No new network requests at run time: the facts come from the page answers the app already fetches. Parsing runs on the catalog's own runtime. Sorting runs on the UI thread on an index list: 1 200 songs sort in well under a millisecond. Saving goes through the existing settings saver thread. No audio buffers are involved.

## Interfaces
```rust
// nemo-core
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollectionMeta { pub owner: Option<String>, pub owner_avatar: Vec<Thumbnail>,
    pub privacy: Option<Privacy>, pub year: Option<u16>, pub view_count: Option<u64>,
    pub track_count: Option<u32>, pub duration_text: Option<String> }
impl CollectionMeta { pub fn is_empty(&self) -> bool; }
// AlbumDetail.meta, PlaylistDetail.meta: CollectionMeta

// nemo-ui
pub enum CollectionSort { Default, Title, Artist, Album, Duration }
impl CollectionSort { pub const ALL: [Self; 5]; pub const fn label(self) -> &'static str;
    pub fn from_label(&str) -> Option<Self>; pub fn offered(album_page: bool) -> Vec<Self>; }
pub fn order(&[Track], CollectionSort) -> Vec<usize>;
pub fn sorted(&[Track], CollectionSort) -> Vec<Track>;
pub fn page_key(album: bool, id: &str) -> String;
pub fn remember(&mut BTreeMap<String, CollectionSort>, &str, CollectionSort);
pub const MAX_REMEMBERED: usize = 300;
// UiSettings.collection_sorts: BTreeMap<String, CollectionSort>
// Message::SortMenu(Option<DetailKey>), Message::SortCollection { key, sort }
// App::sort_key(&DetailKey) -> Option<String>, App::collection_sort(&self, &DetailKey) -> CollectionSort
```

## Error Handling
- An unknown header layout gives an empty `CollectionMeta`, and the page hides the lines' extra parts.
- An unknown stored label loads as the default order.
- Counts that do not parse (`No views`) stay `None`.

## Testing Strategy
- `collection_sort::tests`: every order (stable, case-insensitive, missing last), labels, album pages without the Album order, bounded memory.
- `detail::sort_tests` (mock backend):
  - a sort reorders the page;
  - `PlayAt { index: 2 }` plays the 2nd shown song and queues the shown order;
  - Shuffle draws the same songs;
  - the choice reaches `on_settings`, comes back on a new start, and Default forgets it.
- `meta_line::tests`: both lines, short counts, no summed length for a cut list.
- `nemo-ytm` `feeds::tests::header` on the two new fixtures (owner, avatar, year, 437K views, 30 tracks, 36 minutes; album artist picture, 8 songs, 28 minutes) and the privacy in the subtitle and the edit header; `header_meta::tests::counts_…`; `catalog::tests` for the typed album path.
- `nemo-app` `ui_bridge::tests::page_sorts_round_trip…` (through JSON).
- Snapshots:
  - `playlist-sort-menu` (1366x697, 1920x1040), `playlist-sorted-title`, `playlist-no-meta`;
  - re-rendered: `playlist`, `album` (1366x697, 1920x1040).
- No audio code changed.

## Risks and Alternatives
- YouTube changes the header layout often; every reader falls back to "not known" rather than failing the page.
- `duration_text` is the source's English text (the app asks with `hl=en`). A typed duration would lose `13+ hours`.
- The menu in the list's flow pushes the rows down instead of floating over them. A floating layer inside the scroll area would be clipped on short lists. Owner may prefer a floating menu later.
