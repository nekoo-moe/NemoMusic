Spec approved by the owner on 2026-11-15 (design delegated).

## Overview

### Root cause

The backend was never the problem. `nemo-app` wraps the YouTube Music catalog in `AccountCatalog`, which reads the `SessionSlot` on every request; `GoogleAccount` writes the slot as soon as a sign-in succeeds. So the next catalog call after a sign-in already uses the account. The stale data is in `nemo-ui`:

- What loads at start (`App::pump_loads`, run after every non-frame message): the local library listing once (`library_requested`), the Home shelves once (`HomeState.phase` goes `Idle` to `Loading` to `Done`, then never back), the account bundle (`Job::AccountData`: library playlists, liked songs, albums, artists, history, explore, home feed), and the detail page of the route.
- What is cached, and where: everything stays in `App` memory: `home.shelves` (also the sidebar library list, Quick picks, the likes on rows and the Library fallback, because the session's `home()` adds "Your playlists" and "Liked songs" shelves), `account_library`, `daily` (history, liked, followed, explore), `details`, and `likes` (this session's like changes).
- What happens on an `AccountState` change: nothing is subscribed to it. `pump_loads` compared only the bool `signed_in()` with `daily_requested`. So:
  1. the Home shelves were never fetched again, and stayed anonymous until a restart;
  2. a switch from one signed-in account to another (bool `true` to `true`) reloaded nothing at all;
  3. a state change made by the backend in the background (a saved session without a cached header is checked at start: `SigningIn`, then `SignedIn`) was only seen on the next unrelated message, because frames do not run `pump_loads`;
  4. two bundles could be in flight (one started while signed out, one after the sign-in) and the slower one won, so anonymous data could overwrite the account's;
  5. `Message::AccountData` kept the old `account_library` when the new one was empty, so a second account with no playlists showed the first account's.

### Fix

A small pure state machine, `nemo_ui::refresh::Refresh`, owns "whose data is on screen" (`Owner`: signed out, pending, or signed in as name and handle) and the one reload that may run. The app compares the owner with `AccountService::state()` after every update and every frame (the call is cheap by contract). A new owner starts a reload; the Refresh button, `F5` and `Ctrl+R` start one too. A reload has two steps, run one after the other: `Home` (`Catalog::home`), then `Account` (the same `account_bundle` chain as the start-up load). Each step carries a generation number; an answer from an older run is dropped.

## Affected Crates

- `nemo-ui` only. `nemo-core` and `nemo-app` are unchanged.

## Data Flow

- UI thread: `App::watch_account` (in `pump_loads` and `on_frame`) calls `Refresh::observe(Owner::of(&state))`. A `Start { generation, step }` becomes `Job::Refresh(start)`.
- The job runs as an iced task on the backend, like every other catalog job: `Step::Home` calls `catalog.home()`; `Step::Account` calls the bundle chain (one request after the other). The answer comes back as `Message::Refreshed { generation, data }`.
- `App::on_refreshed` first looks at the account again (so a change that came with the answer makes it stale), then asks `Refresh::answered`: `Drop` (stale; maybe start the queued reload) or `Use` (apply when it worked, start the next step, and at the end show the failure notice if a step failed).
- At most one reload step is in flight. Start-up loads may still be running beside it; their late answers are dropped once a reload replaced that data (`Message::HomeFeed`/`HomeSeed` when `home.phase` is `Done`, `Message::AccountData` once a reload started).
- No audio buffers or player state are touched.

## Interfaces

```rust
// nemo_ui::refresh (pure)
pub const FAILED_NOTICE: &str;
pub const fn tooltip(busy: bool) -> &'static str;
pub enum Owner { SignedOut, Pending, SignedIn { name: String, handle: Option<String> } }
impl Owner { pub fn of(state: &AccountState) -> Self; }
pub enum Step { Home, Account }
pub struct Start { pub generation: u64, pub step: Step }
pub enum Answer { Drop { next: Option<Start> }, Use { next: Option<Start>, ended: Option<bool> } }
pub struct Refresh;
impl Refresh {
    pub fn busy(&self) -> bool;
    pub fn ready(&self) -> bool;
    pub fn generation(&self) -> u64;
    pub fn owner(&self) -> Option<&Owner>;
    pub fn begin(&mut self, owner: Owner);
    pub fn observe(&mut self, now: Owner) -> Option<Start>;
    pub fn request(&mut self) -> Option<Start>;
    pub fn answered(&mut self, generation: u64, step: Step, ok: bool) -> Answer;
}

// nemo_ui::gui::app (additive)
Message::Refresh;
Message::Refreshed { generation: u64, data: RefreshData };
pub enum RefreshData { Home(Result<Option<Arc<Vec<Shelf>>>, String>), Account(Arc<AccountBundle>) }
Job::Refresh(Start);

// nemo_ui::keys (additive)
Key::F5; KeyAction::Refresh;
```

## Error Handling

- `Step::Home`: `Err(CatalogError::Unavailable)` means the catalog has no home page (signed out) and is not a failure; the shelves stay. An empty answer keeps the shelves too. Any other error is a failure; the shelves stay.
- `Step::Account`: the bundle never errors as a whole. It counts as failed when signed in and no account call answered, or when signed out and it holds no data at all. Then `daily` and `account_library` stay.
- A failed run ends with one `Event::Notice(FAILED_NOTICE)`: "Could not refresh your music. Showing what loaded before." Nothing is retried automatically.
- Nothing is logged with account data; the owner name stays in UI memory only.

## Testing Strategy

- `refresh.rs` unit tests: a sign-in starts one full reload in sequence; a second request while busy is ignored; nothing reloads before the start-up loads; a failed step is reported at the end; a switch while busy drops the old answer and reloads once; sign-out runs only the account step.
- `gui/app/refresh/tests.rs`, against a scripted catalog and account (no network): a sign-in reloads Home and the library once and later updates start nothing; a switch reloads and forgets session likes; a background sign-in is seen on the next frame; a second Refresh while one runs does nothing; a failure keeps the old shelves, library and history and shows the notice once; no reload before the notice; the busy top bar builds.
- `keys.rs`: `F5` and `Ctrl+R` map to Refresh, also in a text field; `R` alone does not.
- Headless snapshots (`examples/snapshots`, tiny-skia): `home-signed-in-1366x697` and `shortcuts-1366x697` regenerated.
- No audio code is touched, so no bit-exact test is needed.

## Risks and Alternatives

- Two accounts with the same name and handle look like one owner. `AccountInfo` has no stable id today; the `AccountSwitcher` generation (account-actions spec) can replace the name check when it is wired.
- Alternative: a new `AccountEvent` stream in `nemo-core`. Rejected: it changes the public API, and polling the cheap `state()` after updates and frames covers every path we have.
- Alternative: reload all three requests at once. Rejected: the owner asked for human pace; two sequential steps cost a little more wall time.
- Detail pages are not reloaded; a private playlist opened before sign-in can still show its old page until `Retry`.
