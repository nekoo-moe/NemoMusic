Spec pre-approved by the owner on 2026-11-15.

## Overview

Root cause: the Explore view never asked the catalog for Explore. `gui/views/explore.rs` drew the fixed `MOODS` tiles (each runs a search) and then `App::explore_shelves`, which only `Message::ExploreLoaded` fills. Nothing sends that message, so the view fell back to `app.home.shelves`, and with no home page those are the `home::SEED_SHELVES` searches. `Catalog::explore()` was only called inside `account_bundle` (job.rs) to feed the daily sections, and its answer never reached the page.

The rest of the chain works and was checked: `nemo-ytm` `pages::explore` fetches `FEmusic_explore` and `FEmusic_charts`; `parse_explore`, `parse_charts` and `parse_mood_page` read the real layouts (recorded answers in `crates/nemo-ytm/tests/fixtures/music/`, tests in `src/feeds/tests/explore.rs`); `YtmCatalog`, `AccountCatalog` (`current()` session), `WebSearchCatalog` and `nemo-app` `MergedCatalog` forward `explore` and `mood_playlists`.

Fix, in `nemo-ui`: Explore becomes its own job with its own state, the page draws the answer, and mood tiles open a new mood route.

## Affected Crates

- `nemo-ui`: `explore_feed.rs` (new, pure state), `route.rs` (`Route::Mood`), `gui/app/{explore.rs (new), app.rs, message.rs, job.rs, dispatch.rs, handle.rs, model.rs, query.rs, refresh.rs}`, `gui/views/{explore.rs, explore/feed.rs (new), explore/mood.rs (new), shell.rs}`, `explore.rs` and `refresh.rs` (docs), `examples/snapshots/feature.rs`.
- `nemo-ytm`: fixtures and tests only.

## Data Flow

UI thread only; no audio code is touched. `App::pump_explore` (called from `pump_loads` after every update) starts `Job::Explore` when `ExploreFeed::wants(owner)`, where `owner` is `refresh::Owner::of(account state)`; a pending sign-in waits. The job runs as an iced task, calls `Catalog::explore`, and returns `Message::Explore(ExploreMsg::Loaded(..))`. The answer goes into `ExploreFeed`, and its new releases and moods into `App::daily` (`fill_daily`, also after the account bundle and a reload's account step, which no longer call `explore` themselves; this keeps the request count as before). Refresh (`refresh_msg`) calls `ExploreFeed::reload()` when a reload starts. `Route::Mood { id, title }` makes `pump_explore` start `Job::Mood(id)` once (`MoodPages`, at most 16 pages kept), answered by `ExploreMsg::MoodLoaded`.

## Interfaces

```rust
// nemo-ui, crate::explore_feed
pub enum ExplorePhase { Idle, Loading, Ready, Failed }
pub struct ExploreFeed { .. }
impl ExploreFeed {
    pub fn wants(&self, owner: &Owner) -> bool;
    pub fn start(&mut self, owner: Owner);
    pub fn finish(&mut self, result: Result<Arc<Explore>, String>);
    pub fn reload(&mut self);
    pub fn data(&self) -> Option<&Explore>;
    pub fn phase(&self) -> ExplorePhase;
    pub fn error(&self) -> Option<&str>;
    pub fn shows_fallback(&self) -> bool;
    pub fn notice(&self) -> Option<&'static str>;
    pub fn shelves(&self) -> impl Iterator<Item = &FeedShelf>;
    pub fn shelf(&self, n: usize) -> Option<&FeedShelf>;
    pub fn fill_daily(&self, daily: &mut DailyData);
}
pub fn has_content(e: &Explore) -> bool;
pub fn shelf_tracks(shelf: &FeedShelf) -> Vec<Track>;
pub enum MoodPage { Loading, Ready(Arc<Vec<FeedShelf>>), Failed(String) }
pub struct MoodPages { .. } // wants, start, finish, forget, get, shelf

// nemo-ui, route
Route::Mood { id: BrowseId, title: String } // entry(): NavEntry::Explore

// nemo-ui, gui::app
pub enum ExploreMsg { Loaded(..), MoodLoaded { id, result }, Retry, RetryMood(BrowseId) }
Message::Explore(ExploreMsg)
Job::Explore, Job::Mood(BrowseId)
ListSource::Explore(usize), ListSource::Mood(BrowseId, usize)
```

These are additions to `nemo-ui` public enums; no other crate matches on them. `Message::ExploreLoaded` and `explore_shelves` stay (the fallback still prefers them over Home's shelves).

## Error Handling

Catalog errors become a short string. An answer with no new releases, moods or shelf items counts as a failure. A failure with no earlier answer shows `FALLBACK_NOTICE` with Retry, then the built-in mood tiles and Home's seed shelves; a failure after an answer shows `STALE_NOTICE` with Retry and keeps the answer. No toast, no automatic retry. A failed or empty mood page shows a calm status with Retry, which forgets the page so the next update asks again.

## Testing Strategy

- `nemo-ytm` `feeds::tests::explore`: the recorded Explore, charts and mood answers parse to new releases (albums), six moods with `FEmusic_moods_and_genres_category` ids and params, "Trending" and "New music videos" (videos), "Video charts" (playlists) and "Top artists" (artists), and two playlist shelves.
- `nemo-ui` `explore_feed::tests`: once per owner, pending waits, charts first, fallback only when nothing ever loaded, empty answer is a failure, old data kept, reload waits, daily fill, shelf tracks, mood page cache and cap.
- `nemo-ui` `gui::app::explore::tests` (headless app on a counting catalog): the page holds the catalog's answer, asked once, again on Refresh, nothing before the notice, fallback and Retry, old data kept, mood page loads once and Retry works.
- `nemo-ui` `route` and `views::explore::feed` tests: the mood route lights Explore; song and video cards play by their place in the shelf.
- Snapshots `explore-*` and `explore-mood-*` (mock catalog). No audio path changed, so no bit-exact test.

## Risks and Alternatives

- Keeping `explore` inside the account bundle would have reused the reload machinery, but the bundle answers only after every library list is paged (seconds to tens of seconds signed in), so Explore would stay empty that long. A separate job shows Explore as soon as it answers; the bundle drops its own call so the requests per load stay the same. A signed-out reload whose home feed fails but whose Explore works now reports the reload as failed (before, the Explore data counted for it).
- Showing mood pages inside Explore without a route would avoid the new `Route` variant but break Back. The variant is additive.
- YouTube Music may change the page layout; the readers drop what they do not know, and the fallback keeps the page useful.
