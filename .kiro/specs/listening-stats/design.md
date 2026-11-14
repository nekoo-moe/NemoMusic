## Overview

A new crate, `nemo-stats`, keeps a private play log in its own SQLite file and answers every chart question with SQL over it. Nothing is recorded until the user turns on the one switch on the Stats page (criterion 1). A play counts by Last.fm's rule, and the scrobbler uses the very same function, so both counts agree (criterion 3).

The work is split on purpose. `nemo-core` gets a pure play tracker (shared with the integrations design) that turns `PlayerEvent`s and `PlayerControl::position()` samples into "this track ended, heard N seconds, it counts or not". `nemo-stats` stores finished plays and aggregates them. `nemo-ui` draws the charts. No audio code changes, no new network host, and nothing leaves the machine except a file the user saves.

## Affected Crates

- `nemo-core` (new files `stats.rs`, `hooks.rs`): the `StatsService` trait, period and chart types, and a `NoStats` default. `hooks.rs` (`PlayRule`, `PlayTracker`, `PlayHookEvent`, `PlayObserver`) is defined in full in the integrations design; this design uses it unchanged.
- `nemo-stats` (new): `store.rs` (schema, migrations, writer thread), `period.rs` (calendar bounds), `songkey.rs`, `query.rs` (charts), `year.rs` and `recap.rs`, `export.rs` (CSV, JSON), `receipt.rs` (PNG), `service.rs` (`StatsService` implementation, `PlayObserver` implementation).
- `nemo-ui`: route `Route::Stats` and a sidebar entry; `gui/views/stats.rs` (switch, period picker, three ranked lists, receipt and export buttons); `gui/views/stats_chart.rs` (hour bars on `iced::widget::canvas`, with reduced motion honoured); `copy.rs` (Stats texts, and the sign-out text in `ACCOUNT_SIGN_OUT_CONFIRM`, which must now say that stats stay and name "Clear stats"); `fonts` accessor for the receipt. A save-file dialog and an image clipboard helper are needed; see Risks.
- `nemo-app`: build `StatsStore` at `paths.stats`, register it with the play hub (`play_hub.rs`, from the integrations design), put it on `Backend` as `stats: Arc<dyn StatsService>`.
- `nemo-library`, `nemo-ytm`: no change here. The recap playlist uses `LocalLibraryService::{create_playlist, add_tracks}` (local-library design) and `PlaylistEditor::create` (account-actions design); neither is defined here.
- `nemo-audio`: not touched.

## Data Flow

Threads: the play hub thread (integrations design) samples `position()` every 500 ms and drains a `PlayerEvent` receiver. It never touches the audio callback and holds no lock the callback uses.

1. `PlayTracker` adds to "heard" only while the state is `Playing` and only by `min(position delta, wall time elapsed)`. A forward jump larger than elapsed time is a seek and adds nothing; a backward jump resyncs. A `TrackChanged` event, which `nemo-audio` also emits for repeat one, closes the previous play and opens a new one.
2. At the end of a track the tracker emits `PlayHookEvent::TrackEnded(FinishedPlay { counts, heard, .. })`. `counts` comes from `PlayRule::counts(duration, heard)`: duration over 30 s and heard at least `min(duration / 2, 240 s)`. With an unknown duration the 240 s arm applies.
3. `StatsRecorder` (a `PlayObserver`) reads an `AtomicBool` for the switch. When on and `counts`, it sends a `NewPlay` through `std::sync::mpsc::sync_channel(64)` with `try_send`; a full channel drops the play and logs one warning without titles.
4. The `stats-writer` thread owns inserts into `Mutex<Connection>` (the `nemo-lyrics` cache pattern: WAL, parameterized statements). Queries run on `tokio::task::spawn_blocking` and return plain owned values to the UI.
5. On app exit the hub calls `flush`, which closes the current play, so at most one play is lost on a crash.

Buffers: no audio buffer is read or copied. A play row holds metadata only.

Storage `stats.sqlite3` under `paths.stats`, `PRAGMA user_version` migrations, one transaction each:

```sql
CREATE TABLE plays (id INTEGER PRIMARY KEY, started_at INTEGER NOT NULL,   -- Unix seconds, UTC
  tz_offset_min INTEGER NOT NULL,  -- local offset at start, so the clock survives DST and travel
  heard_ms INTEGER NOT NULL, duration_ms INTEGER, origin TEXT NOT NULL,    -- 'local' | 'ytm' | 'soundcloud'
  track_id TEXT NOT NULL, video_id TEXT, local_path TEXT,
  title TEXT NOT NULL, artist TEXT, album TEXT,
  song_key TEXT NOT NULL, artist_key TEXT, album_key TEXT);
CREATE INDEX plays_time ON plays(started_at);  CREATE INDEX plays_song ON plays(song_key);
CREATE TABLE recap_items (year INTEGER, month INTEGER, rank INTEGER, plays INTEGER, title TEXT, artist TEXT, album TEXT,
  origin TEXT, track_id TEXT, video_id TEXT, local_path TEXT, duration_ms INTEGER, PRIMARY KEY (year, month, rank));
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);  -- recording, monthly_recaps, last_recap_month
```

Retention: forever, until "Clear stats". A row is about 200 bytes, so 100 plays a day for 10 years (365,000 rows) is about 73 MB. No pruning in this spec.

One song across sources: `song_key` is the first 16 bytes of SHA-256 over the NFKC, lower-cased, letters-and-digits-only primary artist and title. A song played from a local file and from YouTube Music with the same names becomes one chart row; remasters with different titles stay separate. Charts show the spelling of the latest play. A play with no artist has a NULL `artist_key` and shows "Unknown artist" (criterion 22). Only the first artist is credited. Albums rank by `album_key` (album plus primary artist) so two "Greatest Hits" stay apart.

Periods: Week is Monday 00:00 local to now, then month and year to date, plus any past month or year. Bounds are computed with `chrono` in the local zone (a DST gap resolves to the next valid time) and passed to SQL as Unix seconds. Hours come from `started_at + tz_offset_min * 60`.

The YouTube Music account history is not imported (Out of Scope). The recap for a finished month is frozen into `recap_items` at the first start after the month ends, if monthly recaps are on, so later plays do not change it.

## Interfaces

```rust
// nemo-core/src/stats.rs  (no I/O; futures are BoxFuture)
pub enum StatsPeriod { Week, Month, Year, AllTime, MonthOf { year: i32, month: u8 }, YearOf(i32) }
pub struct RankedSong { pub track: Track, pub plays: u32, pub heard: Duration }
pub struct RankedName { pub name: Option<String>, pub plays: u32 }   // None shows "Unknown artist/album"
pub struct HourClock { pub plays: [u32; 24] }
pub struct YearSummary { pub year: i32, pub plays: u32, pub heard: Duration, pub songs: Vec<RankedSong>,
    pub artists: Vec<RankedName>, pub albums: Vec<RankedName>, pub busiest_month: (u8, u32), pub busiest_hour: (u8, u32) }
pub struct Recap { pub year: i32, pub month: u8, pub songs: Vec<RankedSong> }
pub enum ExportFormat { Csv, Json }
pub struct Receipt { pub png: Vec<u8>, pub width: u32, pub height: u32 }
pub enum StatsError { Unavailable, Database(String), Io(String), Empty }   // thiserror in the impl crate's wrapper

pub trait StatsService: Send + Sync {
    fn recording(&self) -> bool;
    fn set_recording(&self, on: bool) -> BoxFuture<'_, Result<(), StatsError>>;
    fn top_songs(&self, p: StatsPeriod, limit: usize) -> BoxFuture<'_, Result<Vec<RankedSong>, StatsError>>;
    fn top_artists(&self, p: StatsPeriod, limit: usize) -> BoxFuture<'_, Result<Vec<RankedName>, StatsError>>;
    fn top_albums(&self, p: StatsPeriod, limit: usize) -> BoxFuture<'_, Result<Vec<RankedName>, StatsError>>;
    fn clock(&self, p: StatsPeriod) -> BoxFuture<'_, Result<HourClock, StatsError>>;
    fn periods_with_plays(&self) -> BoxFuture<'_, Result<Vec<StatsPeriod>, StatsError>>; // months and years for the picker
    fn year_in_music(&self, year: i32) -> BoxFuture<'_, Result<Option<YearSummary>, StatsError>>;
    fn monthly_recaps(&self) -> bool;
    fn set_monthly_recaps(&self, on: bool) -> BoxFuture<'_, Result<(), StatsError>>;
    fn recap(&self, year: i32, month: u8) -> BoxFuture<'_, Result<Option<Recap>, StatsError>>;
    fn receipt(&self, p: StatsPeriod, limit: usize) -> BoxFuture<'_, Result<Receipt, StatsError>>;
    fn export(&self, format: ExportFormat, dest: &Path) -> BoxFuture<'_, Result<u64, StatsError>>; // rows written
    fn clear(&self) -> BoxFuture<'_, Result<u64, StatsError>>;  // plays deleted
}
pub struct NoStats;   // recording() false, every query Err(Unavailable)

// nemo-stats
pub struct StatsStore { /* Mutex<Connection>, AtomicBool, writer handle */ }
impl StatsStore {
    pub fn open(dir: &Path, font: Vec<u8>) -> Result<Self, StoreError>;  // font: bytes of the bundled UI font
}
impl StatsService for StatsStore { /* ... */ }
impl PlayObserver for StatsStore { /* TrackEnded only */ }
pub fn period_bounds<Tz: chrono::TimeZone>(p: StatsPeriod, now: chrono::DateTime<Tz>) -> (i64, i64);
pub fn song_key(artist: Option<&str>, title: &str) -> String;
pub fn counts_as_play(duration: Option<Duration>, heard: Duration) -> bool; // wraps nemo_core::hooks::PlayRule
```

The charts are ranked by play count, ties by heard time, then by latest play. The receipt is 640 px wide: the NemoMusic name, the period, up to 10 rows (rank, title, artist, plays), the total, drawn with `ab_glyph` and the bundled Google Sans Flex, NFC-normalized, long names cut with an ellipsis. No cover art, no account name, no lyrics. It is made in memory on demand, so there is no receipt cache to clear.

Export writes to `dest.tmp` in the same folder, then renames. Columns: `started_at` (RFC 3339 UTC), `heard_seconds`, `title`, `artist`, `album`, `source`. No ids, paths, cookies or lyrics. CSV cells that start with `=`, `+`, `-` or `@` get a leading `'` so a spreadsheet does not run a title as a formula; the writer is about 30 lines, so no `csv` crate.

## Error Handling

- The database cannot open or a write fails: `StatsStore::open` returns `StoreError`; `nemo-app` logs a warning and wires `NoStats`. The Stats page shows one short line and playback goes on (criterion 23). A failed insert counts into `last_error`, which the page shows once.
- A full writer channel drops the play and warns once per minute. Nothing blocks the hub thread.
- A missing field: `artist` or `album` NULL, never guessed. A duration of 0 or unknown uses the 240 s arm.
- An empty period returns an empty list or `Ok(None)`; the UI shows the empty state (criterion 11).
- Export or receipt I/O errors map to `StatsError::Io` with a short text and no path in the log.
- Sign-out: no wipe step is registered, by the owner's decision of 2026-11-14 (see Risks). "Clear stats" runs `DELETE FROM plays`, `recap_items`, then `VACUUM` off the UI thread.

## Testing Strategy

- `PlayRule` and `PlayTracker` have table tests (29 s, 30 s, 31 s, 8 min tracks; unknown duration; pause; seek forward and back; repeat one counts twice; skip at 29 s; gapless handover) and a `proptest` that heard time never exceeds wall time.
- `period_bounds` with `FixedOffset` and a DST zone: week start, month end, leap day, a DST gap.
- `song_key`: case, accents, punctuation, NFD against NFC give one key; different titles differ.
- Store: migrations from an empty file, then twice (idempotent); insert and chart queries against a seeded in-memory database; `clear` empties every table; the recording switch off records nothing; a read-only file gives `StoreError`.
- Export: the output parses back, has exactly the listed columns, and a title `=1+1` is escaped. A test greps the JSON for `path`, `id`, `cookie` and `lyrics` keys.
- Receipt: the PNG decodes with `image`, has the expected width, and a Vietnamese title draws without panic.
- UI: `iced_test` snapshots of the Stats page empty, populated and with reduced motion; keyboard order of the controls (criterion 25).
- Audio is untouched; `cargo test -p nemo-audio --test bit_exact` must keep passing. The hub reads only `position()` atomics, so no counting-allocator test is added; the existing real-time tests stay as they are.

## Risks and Alternatives

- Sign-out keeps stats (owner decision). `product.md` and ADR 0004 say sign-out wipes cached account data, and `nemo-stats` registers no wipe step. This is a deliberate exception: plays are this computer's history. The sign-out text and "Clear stats" carry the explanation. ADR 0004 should list the exception.
- A play is counted when the track ends, so a crash loses the current play. Writing at the threshold would fix that but would store a partial "heard" value; the end-of-track write is kept.
- Heard time is sampled every 500 ms, so it is off by up to half a second per track. The rule's 50 % line is not affected in practice.
- One song across sources by name only: tags that differ ("(Remastered)") split a song in two. A fuzzy match would invent merges, which criterion 22 forbids.
- The receipt uses one font with no fallback, so CJK or Thai titles show missing-glyph boxes. Alternative: render with the iced headless renderer; rejected because `iced_test` needs `ICED_TEST_BACKEND` set, which needs `unsafe`.
- Receipt text uses `ab_glyph` 0.2.32 (Apache-2.0, released 2025-09-28, inside the 2 years `code.md` allows; ADR 0004, Decision 5). It is already in `Cargo.lock` (through `sctk-adwaita`, Wayland only), so it adds no new version to audit. `fontdue` 0.9.5 (released 2026-10-19, `rust-version` not listed, unverified) is newer but a new crate for the same job. `ab_glyph` lays out nothing, so the receipt places glyphs one by one from NFC text, which is enough for one font and no shaping.
- A save-file dialog (`rfd` 0.18.0, MIT, MSRV 1.92, 2026-11-07) and an image clipboard (`arboard` 3.7.0, MIT OR Apache-2.0, MSRV 1.71, 2026-10-15) are not in the workspace. The first design that lands one adds it to `nemo-ui`; both are unverified against `cargo deny` and Linux build packages.
- Alternative storage in `nemo-library`'s database: rejected, ADR 0004 says one SQLite file per owning crate and Clear stats must not touch the library.

## Steering and README changes

- Opt-in: the recording switch and the monthly-recaps switch use the shared `OptInRow` and `OptInInfo` (ADR 0004, "Opt-in settings pattern"). Sends: nothing; hosts: none; the risk text says the history stays on this computer, survives sign-out and is deleted by "Clear stats".
- `tech.md`: add `chrono` =0.4.45 (MIT OR Apache-2.0, 2026-06-04, `clock` feature, already in `Cargo.lock`); `ab_glyph` =0.2.32 (Apache-2.0, 2025-09-28, already locked; direct dependency of `nemo-stats`); `rusqlite`, `image`, `sha2`, `unicode-normalization` and `tokio` are existing pins. Confirm `rusqlite` 0.41.1 stays pinned (0.42.0 exists since 2026-11-11).
- `structure.md`: new crate `nemo-stats`, edge `nemo-app -> nemo-stats`, it depends on `nemo-core` only; note `Backend.stats`.
- README egress list: no new host. README privacy text: stats are off until switched on, stay local, survive sign-out, export is a file the user saves.
- ADR 0004: record the sign-out exception and the `nemo-stats` crate list (`ab_glyph`, `chrono`).
- `copy.rs` text for sign-out and the Stats page (implementing task).

<!-- Sources:
- docs/adr/0004-feature-wave-crates.md (crate map, sign-out rule, one SQLite per crate)
- crates/nemo-core/src/player.rs (PlayerEvent, PlayerControl), services.rs (Backend, PlayerControl::position), track.rs (Track)
- crates/nemo-audio/src/control.rs, control_tick.rs (TrackChanged fires on every start and handover, including repeat one)
- crates/nemo-lyrics/src/cache.rs (SQLite pattern), crates/nemo-app/src/paths.rs, backend.rs (wiring)
- crates/nemo-ui/src/route.rs, copy.rs (ACCOUNT_SIGN_OUT_CONFIRM), gui/headless.rs
- https://www.last.fm/api/scrobbling (scrobble rule)
- https://crates.io/api/v1/crates/{fontdue,ab_glyph,chrono,rfd,arboard,rusqlite} (versions, licences, dates, queried 2026-11-14)
-->
