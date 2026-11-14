## Tasks

Cross-spec ids written `<spec>#?` name the other spec's task by the ADR 0004 Decision 4 row it delivers; the number is filled in once that spec's `tasks.md` is merged. The play rule and tracker (`hooks.rs`) and the play hub belong to integrations and are used here unchanged.

- [ ] 1. Add the `StatsService` contract and `Backend.stats`
  - Crate and files: `nemo-core` (`src/stats.rs` new, `src/services.rs`, `src/lib.rs`); touches nemo-core/src/stats.rs and nemo-core/src/services.rs.
  - Does: Adds `StatsService`, `StatsPeriod`, `RankedSong`, `RankedName`, `HourClock`, `YearSummary`, `Recap`, `ExportFormat`, `Receipt`, `StatsError` and `NoStats` (recording off, every query `Unavailable`), and `Backend.stats` defaulting to `NoStats`. No I/O.
  - _Requirements: none (groundwork)_
  - Depends on: integrations#7 (same `services.rs`, serialize)
  - Wave: 0
  - Checks: `NoStats` returns `recording() == false` and `Err(Unavailable)`. ai-features depends on this task for `top_artists`.
  - Steering: `structure.md`, add the `stats` module and `Backend.stats` to the `nemo-core` entry.

- [ ] 2. Create the `nemo-stats` crate skeleton
  - Crate and files: `nemo-stats` (new: `Cargo.toml`, `src/lib.rs`, `src/service.rs`); workspace `Cargo.toml` members.
  - Does: Adds the crate with `StatsStore` and `StoreError` stubs and `counts_as_play`, a thin wrapper over `nemo_core::hooks::PlayRule::counts`.
  - _Requirements: 3_
  - Depends on: listening-stats#1, integrations#4
  - Wave: 0
  - Checks: `counts_as_play` agrees with `PlayRule` on the integrations table (29 s, 30 s, 31 s, 8 min, unknown duration counts at 240 s).
  - Steering: `structure.md`, new crate `nemo-stats` (depends on `nemo-core` only) and edge `nemo-app -> nemo-stats`.

- [ ] 3. Add the play log database
  - Crate and files: `nemo-stats` (`src/store.rs` new); `nemo-app` (`src/paths.rs`, `paths.stats`).
  - Does: Opens `stats.sqlite3` (WAL, `user_version` migrations, one transaction each) with `plays`, `recap_items` and `meta` as in the design, and a `stats-writer` thread that owns inserts with parameterized statements. The `recording` flag lives in `meta` and starts off. Open or write failures return `StoreError`.
  - _Requirements: 1, 2, 7, 23, 24_
  - Depends on: listening-stats#2
  - Wave: 1
  - Checks: migrations from empty, then twice (idempotent); a fresh file has recording off; a read-only file gives `StoreError`; no network code in the crate.

- [ ] 4. Add the cross-source song key
  - Crate and files: `nemo-stats` (`src/songkey.rs` new).
  - Does: `song_key` is the first 16 bytes of SHA-256 over NFKC, lower-cased, letters-and-digits primary artist and title; `artist_key` and `album_key` likewise. A missing artist gives NULL keys, never a guess.
  - _Requirements: 22_
  - Depends on: listening-stats#2
  - Wave: 1
  - Checks: case, accents, punctuation and NFD against NFC give one key; different titles differ; no artist gives `None`.

- [ ] 5. Add calendar period bounds
  - Crate and files: `nemo-stats` (`src/period.rs` new).
  - Does: `period_bounds` turns each `StatsPeriod` into Unix-second bounds in the local zone with `chrono`: week from Monday 00:00, month and year to date, past months and years, all time. A DST gap resolves to the next valid time.
  - _Requirements: 8, 9_
  - Depends on: listening-stats#2
  - Wave: 1
  - Checks: tests with `FixedOffset` and a DST zone: week start, month end, leap day, DST gap.
  - Steering: `tech.md` row `chrono` =0.4.45 (`clock` feature, already locked).

- [ ] 6. Record counted plays from the play hub
  - Crate and files: `nemo-stats` (`src/service.rs`: `PlayObserver` for `StatsStore`); `nemo-app` (`src/backend.rs`: build `StatsStore`, register with `play_hub.rs`, `Backend.stats`, `NoStats` on open failure).
  - Does: On `TrackEnded` with `counts` and recording on, sends a `NewPlay` with `try_send` on a `sync_channel(64)`; a full channel drops the play and warns once per minute without titles. Stores track id, title, artist, album, origin (`local` or `ytm`), start time, `tz_offset_min` and heard time. Repeat one gives a new row.
  - _Requirements: 1, 2, 3, 4, 5, 6, 23, 24_
  - Depends on: listening-stats#3, listening-stats#4, integrations#5, integrations#9
  - Wave: 2
  - Checks: integrations tracker tables feed the observer: pause and seek add nothing, repeat one counts twice, skip at 29 s and tracks of 30 s or less record nothing, recording off records nothing; a failed open wires `NoStats` and playback goes on. `nemo-audio` unchanged; bit-exact test keeps passing.

- [ ] 7. Record SoundCloud plays with their source
  - Crate and files: `nemo-stats` (`src/service.rs`).
  - Does: Maps `Origin::SoundCloud` to `origin = 'soundcloud'` and keeps its `source_url` out of the stored row.
  - _Requirements: 3_
  - Depends on: listening-stats#6, soundcloud#? (row 9, `Origin::SoundCloud`)
  - Wave: 2
  - Checks: a SoundCloud track records with origin `soundcloud`.

- [ ] 8. Answer the chart queries
  - Crate and files: `nemo-stats` (`src/query.rs` new, `src/service.rs`).
  - Does: Implements `top_songs`, `top_artists`, `top_albums`, `clock` (hours from `started_at + tz_offset_min`) and `periods_with_plays` on `spawn_blocking`. Ranks by plays, then heard time, then latest play; shows the latest spelling; albums by `album_key`. An empty period returns an empty list.
  - _Requirements: 8, 9, 10, 11, 22_
  - Depends on: listening-stats#5, listening-stats#6
  - Wave: 2
  - Checks: seeded in-memory database: rankings and ties, one song across local and YouTube Music, two "Greatest Hits" apart, NULL artist as `None`, empty period empty, clock across a DST change.

- [ ] 9. Add the year in music and monthly recaps
  - Crate and files: `nemo-stats` (`src/year.rs` new, `src/recap.rs` new).
  - Does: `year_in_music` returns total plays, heard time, top songs, artists, albums, busiest month and hour, or `None` for a year without plays. With monthly recaps on (off by default, in `meta`), a finished month is frozen into `recap_items` at the first start after it ends.
  - _Requirements: 15, 16_
  - Depends on: listening-stats#8
  - Wave: 2
  - Checks: seeded year summary; recap frozen once and unchanged by later plays; recaps off creates nothing; `last_recap_month` survives restart.

- [ ] 10. Export the user's plays to a file
  - Crate and files: `nemo-stats` (`src/export.rs` new).
  - Does: Writes CSV or JSON to `dest.tmp`, then renames. Columns: `started_at` (RFC 3339 UTC), `heard_seconds`, `title`, `artist`, `album`, `source`; no ids, paths, cookies or lyrics. CSV cells starting with `=`, `+`, `-` or `@` get a leading `'`.
  - _Requirements: 19_
  - Depends on: listening-stats#6
  - Wave: 2
  - Checks: output parses back with exactly the listed columns; `=1+1` is escaped; JSON has no `path`, `id`, `cookie` or `lyrics` key; an I/O error maps to `StatsError::Io` without the path in the log.

- [ ] 11. Clear all stats
  - Crate and files: `nemo-stats` (`src/store.rs`, `src/service.rs`).
  - Does: `clear` deletes `plays` and `recap_items`, then runs `VACUUM`, off the UI thread, and returns the count. Sign-out registers no wipe step (owner decision, ADR 0004 Decision 3).
  - _Requirements: 20, 21_
  - Depends on: listening-stats#9
  - Wave: 2
  - Checks: after `clear` every table is empty and every chart, year and recap query is empty; no `SignOutWipe` impl exists in the crate.

- [ ] 12. Draw the listening receipt image
  - Crate and files: `nemo-stats` (`src/receipt.rs` new); `nemo-app` (passes the bundled Google Sans Flex bytes to `StatsStore::open`).
  - Does: Draws a 640 px wide text-only PNG with `ab_glyph`: NemoMusic name, period, up to 10 rows (rank, title, artist, plays) and the total, NFC text, long names cut with an ellipsis. No cover, account name, email, id or lyrics. Made in memory, no cache.
  - _Requirements: 12, 13_
  - Depends on: listening-stats#8
  - Wave: 2
  - Checks: the PNG decodes with `image` at 640 px; a Vietnamese title draws without panic; a test that the inputs carry no account fields.
  - Steering: `tech.md` row `ab_glyph` =0.2.32 (direct dependency of `nemo-stats`, already locked); ADR 0004 note of the `nemo-stats` crate list (`chrono`, `ab_glyph`).

- [ ] 13. Add the Stats page with the recording switch and charts
  - Crate and files: `nemo-ui` (`src/route.rs` `Route::Stats`, sidebar entry, `src/gui/views/stats.rs` new, `src/copy.rs`).
  - Does: The recording switch is an `OptInRow` ("Record my listening on this computer", off, sends nothing, no host, the history stays here, survives sign-out and is deleted by "Clear stats"). A period picker and three ranked lists; "Unknown artist" and "Unknown album" for missing data; an empty state; one line when the database is unavailable. No "Wrapped" or other app name.
  - _Requirements: 1, 2, 8, 9, 11, 22, 23, 25, 27_
  - Depends on: listening-stats#8, integrations#2
  - Wave: 3
  - Checks: `iced_test` snapshots empty, populated and with `NoStats`; keyboard order and visible focus; `design.md` checklist with a screenshot.

- [ ] 14. Add the listening clock chart
  - Crate and files: `nemo-ui` (`src/gui/views/stats_chart.rs` new).
  - Does: Draws 24 hour bars on `iced::widget::canvas` for the chosen period, with the empty state when there are no plays. No animation under reduced motion.
  - _Requirements: 10, 11, 26_
  - Depends on: listening-stats#13
  - Wave: 3
  - Checks: `iced_test` snapshot with reduced motion on and off; keyboard reach.

- [ ] 15. Add receipt and export commands
  - Crate and files: `nemo-ui` (`src/gui/views/stats.rs`, save-file dialog with `rfd`, image clipboard with `arboard`).
  - Does: "Make receipt" saves the PNG to a file the user picks or copies it to the clipboard; "Export my stats" writes CSV or JSON to a picked path. Nothing is uploaded.
  - _Requirements: 12, 14, 19, 25_
  - Depends on: listening-stats#10, listening-stats#12, listening-stats#13
  - Wave: 3
  - Checks: keyboard reach; cancelled dialog writes nothing; manual check of the clipboard on Windows and Linux. `rfd` and `arboard` must pass `cargo deny` and the Linux build packages.
  - Steering: `tech.md` rows `rfd` =0.18.0 and `arboard` =3.7.0 for `nemo-ui`, only if local-library or lyrics-plus has not added them first.

- [ ] 16. Add the year in music and recap views
  - Crate and files: `nemo-ui` (`src/gui/views/stats_year.rs` new, `src/copy.rs`).
  - Does: Shows the year summary for any year with plays and a monthly recap list, and the "Monthly recaps" switch (off by default). A recap image uses the same save or copy path as the receipt.
  - _Requirements: 14, 15, 16, 25, 26_
  - Depends on: listening-stats#9, listening-stats#15
  - Wave: 3
  - Checks: `iced_test` snapshots with and without data; keyboard order; reduced motion.

- [ ] 17. Save a recap as a playlist
  - Crate and files: `nemo-ui` (recap view actions); `nemo-app` (wiring the library and account services).
  - Does: "Save recap as local playlist" calls `LocalLibraryService` to create a local playlist; local songs play from the file. "Save recap to YouTube Music" shows only when `PlaylistEditor` is available, asks before writing, adds only songs with a YouTube Music id and says how many were left out.
  - _Requirements: 17, 18_
  - Depends on: listening-stats#16, local-library#? (row 16, `LocalLibraryService` playlists), account-actions#? (row 27, `PlaylistEditor::create`)
  - Wave: 3
  - Checks: fakes for both services: local playlist created with local paths; YouTube Music write only after confirm; left-out count correct; signed out hides the YouTube Music action.

- [ ] 18. Add "Clear stats" and the sign-out text
  - Crate and files: `nemo-ui` (`src/gui/views/stats.rs`, `src/copy.rs` `ACCOUNT_SIGN_OUT_CONFIRM`).
  - Does: "Clear stats" with a confirm step calls `clear`. The sign-out confirmation says that local stats stay on this computer and points to "Clear stats".
  - _Requirements: 20, 21, 25_
  - Depends on: listening-stats#11, listening-stats#13
  - Wave: 3
  - Checks: `iced_test` of the confirm dialog with keyboard; a test that the sign-out text names "Clear stats".

- [ ] 19. Document stats and run the manual checks
  - Crate and files: `README.md`; results in the PR.
  - Does: README privacy text: stats are off until switched on, stay local, survive sign-out, export is a file the user saves; no egress line. A test greps UI copy for "Wrapped". Manual: a day of real listening offline and online, charts match plays, receipt and export on Windows and Linux.
  - _Requirements: 7, 24, 27_
  - Depends on: listening-stats#17, listening-stats#18
  - Wave: 3
  - Checks: egress-list test from integrations#3 unchanged.
  - Steering: README privacy text for listening stats.

## Definition of Done

See [workflow.md section 4](../../steering/workflow.md#4-definition-of-done). Also for this spec:
- `nemo-audio` is not changed; `cargo test -p nemo-audio --test bit_exact` keeps passing. No real-time path is added, so no counting-allocator test.
- The crate contacts no host and the README egress list does not change.
- Nothing is recorded while the switch is off, and no play row holds lyrics, cookies, tokens or account ids.
