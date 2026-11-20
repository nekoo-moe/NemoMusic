## Overview

`Source` loses `All` and moves from `gui/app/model.rs` to the pure module `crates/nemo-ui/src/workspace.rs`, which also holds what each area offers: its sidebar entries (`nav`), its landing page (`landing`), the pages it has (`allows`) and who owns a page id (`owns_page`, `is_foreign`). The app keeps the active area in `App::source`, starts it from `UiSettings::listening_area`, and switches it in one function (`App::switch_source`, `gui/app/workspace.rs`). Every screen that used to read a source switch reads `App::source` instead: the sidebar list, Library, Search, the nav entries, Refresh and the background loads.

The control is one shared view (`views/listening_area.rs`), drawn in the account sheet and in the SoundCloud section of Settings. It reuses the sliding chips of the kit, so it has the same focus ring and the same pill motion as the other chip rows.

## Affected Crates

- `nemo-ui`: `workspace.rs`, `settings.rs` (`listening_area`, `SettingChange::ListeningArea`), `gui/app/workspace.rs`, the views named above, copy, the headless scenario (`area`), snapshots.
- `nemo-app`: `config.rs` (`listening_area: String`) and `ui_bridge.rs` (label to `Source` and back). No other crate.

## Data Flow

- Start: `App::start_source` takes the saved area when SoundCloud is on, running and signed in (or a saved session is being checked), else YouTube Music, and stores the fallback.
- Switch (`apply_source`): swap `ui.search` and `lib_sort` with `App::rest` (what the other area remembers), set `source`, start `ui.nav` on the landing page (so Back stays inside the area), restart the page entrance, send `SettingChange::ListeningArea` (the app saves it), then `sc_watch` starts the SoundCloud library read when SoundCloud is now the area.
- Fall back (`watch_source`, after every update): SoundCloud is the area but is not signed in (nor being checked) -> `apply_source(YouTube)`.
- Search: `Job::Search` and `Job::SearchKind` answer as `source: YouTube`, the SoundCloud jobs as `SoundCloud`. An answer for the area that is not on screen is applied to `App::rest.search` (a pure `SearchState`), never to the visible one.
- Pages: `Event::Go` of Home, Explore or a mood page in the SoundCloud area is replaced by the landing page (`Source::allows`). Detail pages open in either area.
- Background loads: `pump_loads` starts YouTube Music's account data, Home feed and Explore only while YouTube Music is the area. `refresh_msg` reloads only the active area.
- The `Create playlist` button of the sidebar opens the SoundCloud name sheet in the SoundCloud area.

Sidebar highlight: `App::nav_selected` returns the entry `Nav::active_entry` gives if the area offers it, and `None` for a detail page of the other service. A page opened from Now Playing therefore lights nothing, and the pill fades out.

## Interfaces

```rust
// nemo-ui::workspace
pub enum Source { YouTube, SoundCloud }
impl Source {
    pub const ALL: [Self; 2];
    pub const fn label(self) -> &'static str;
    pub fn from_label(text: &str) -> Option<Self>;
    pub const fn index(self) -> usize;
    pub const fn nav(self) -> &'static [NavEntry];
    pub fn landing(self) -> Route;
    pub fn nav_index(self, entry: NavEntry) -> Option<usize>;
    pub fn allows(self, route: &Route) -> bool;
    pub fn owns_page(self, id: &BrowseId) -> bool;
    pub fn is_foreign(self, route: &Route) -> bool;
}
// nemo-ui::settings
UiSettings { pub listening_area: Source, .. }
SettingChange::ListeningArea(Source)
// nemo-ui::gui::app
App::source_offered(&self, Source) -> bool
App::nav_selected(&self) -> Option<NavEntry>
Message::SearchDone { source, query, result }
Message::SearchKindDone { source, query, kind, result }
```

Removed: `Source::All`, `Source::has_soundcloud`, `ScGroup`, `ScGroupPhase`, `Message::ScGroup`, `Job::ScGroup`, `ListSource::ScSearch`, `NAV_ENTRIES`, the sidebar, Library and Search source chips.

## Error Handling

A failed SoundCloud library read keeps what is shown and says so (unchanged). An area that cannot run is never entered: the control shows a sentence instead, a stale saved value falls back to YouTube Music, and a session that ends while SoundCloud is the area returns to YouTube Music with no popup. The fallback is stored so the next start agrees.

## Testing Strategy

No audio code changes, so no bit-exact test. Pure tests in `workspace.rs` (labels, nav, landing, allowed pages, page owners). App tests in `gui/app/workspace/tests.rs`: one press, no-op presses, per-area search text and sort, fall back after sign-out and after turning SoundCloud off, the saved area at start, background loads, which entry is lit. Search routing in `sc_search.rs`, the sidebar rows per area in `sc_sidebar_tests.rs`. Config and bridge round trips in `nemo-app`. Snapshots of the control, the sheet and the sidebar in both areas (dark and light, 960 x 600 and 1366 x 697).

## Risks and Alternatives

- Landing on Search was rejected: an empty box is a poor first screen. Landing on Library lets a listener who switches see their likes at once.
- A SoundCloud Home built from `home_feed` was rejected for now: it needs a feed state, a job and a view next to the YouTube Music one.
- Keeping one `SearchState` and clearing it on a switch was rejected: the text would be lost and an in-flight answer could land in the wrong area. Swapping two states keeps both.
- A disabled second choice was rejected in favour of a sentence: a control that cannot be pressed is a dead end.
- A switch that also resets Back history loses the other area's history; that is the point, so Back never shows a page of the other service.
