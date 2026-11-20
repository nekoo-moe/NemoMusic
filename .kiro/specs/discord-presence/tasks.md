## Tasks

- [x] 1. Spec, `PresenceState` builder, rate limiter and sleeping rule (PR 1)
  - Crate and files: `nemo-integrations` (`src/discord/{mod,text,activity,state,limiter,away}.rs`, `src/lib.rs`).
  - Does: criteria 1 to 12, 14 (limiter), 15 and the sleeping rule of 6, all pure and tested with fake clocks.
  - _Requirements: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 15_
  - Depends on: none

- [x] 2. IPC client, session, engine, worker and wiring (PR 2)
  - Crate and files: `nemo-integrations` (`src/discord/{ipc,session,engine,worker}.rs`, `src/service.rs`), `nemo-core` (`integrations.rs`, additive), `nemo-app` (`backend.rs`, one hook).
  - Does: criteria 1 to 3 from real play events, 13, 14, 16 to 19 and 21 (the switch and its confirmed version are kept in `integrations.sqlite3`). Tests with a fake Discord on a Unix socket and with a fake connector; one test binary captures the log. NOT verified: a real Discord client, the Windows pipe, Flatpak and Snap sockets, `cargo deny` (not installed here; no crate was added).
  - _Requirements: 13, 14, 16, 17, 18, 19, 21_
  - Depends on: 1

- [ ] 3. Settings row, `Hide song names` switch, notice text, README (PR 3)
  - Crate and files: `nemo-ui` (`gui/views/settings/discord.rs`, `gui/app/discord.rs`, copy), `nemo-integrations` (`info.rs`), `README.md`.
  - Does: criteria 10 (switch), 20 and 21; the README egress note `Discord presence adds no host` and the asset names.
  - _Requirements: 10, 20, 21_
  - Depends on: 2

## Definition of Done

Every task meets [section 4 of workflow.md](../../steering/workflow.md#4-definition-of-done).
