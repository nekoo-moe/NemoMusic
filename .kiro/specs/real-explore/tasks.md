Spec pre-approved by the owner on 2026-11-15.

## Tasks

- [x] 1. Explore shows YouTube Music's real Explore. _Requirements: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10_
  - Crate and files: `nemo-ui` (`src/explore_feed.rs`, `src/explore_feed/tests.rs`, `src/lib.rs`, `src/route.rs`, `src/explore.rs`, `src/refresh.rs`, `src/gui/app.rs`, `src/gui/app/{explore.rs,explore/tests.rs,message.rs,job.rs,dispatch.rs,handle.rs,model.rs,query.rs,refresh.rs}`, `src/gui/views/{explore.rs,explore/feed.rs,explore/mood.rs,shell.rs}`, `examples/snapshots/feature.rs`); `nemo-ytm` (`src/feeds/tests.rs`, `src/feeds/tests/explore.rs`, `tests/fixtures/music/*`); `docs/ui/snapshots/explore{,-mood}-{1366x697,1920x1040}.png`.
  - Does: Explore is its own job per account owner and on Refresh; the page shows new releases, mood tiles that open `Route::Mood` pages from `mood_playlists`, charts and the other shelves; seed shelves only when Explore never loaded, with one short line; old data kept on a failed reload.
  - Checks: `cargo fmt --all -- --check`; `cargo clippy -p nemo-ytm -p nemo-ui -p nemo-app --all-targets --all-features -- -D warnings`; `cargo test -p nemo-ytm -p nemo-ui -p nemo-app --all-features`; the headless snapshots.
  - Steering: code.md (errors, tests, fixtures), design.md sections 1, 4, 9, 14 and 15.

## Definition of Done

See [workflow.md section 4](../../steering/workflow.md#4-definition-of-done).
