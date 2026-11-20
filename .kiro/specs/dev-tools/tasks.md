# Developer tools: tasks

## Tasks

- [x] 1. Spec files; the ring, the redaction pass, the capture layer and its filter in `nemo-app` (`dev_capture`), `logging::init` on layers; tests. _Requirements: 5, 6, 7, 8_
- [x] 2. `nemo-core` `dev` module: trace target, field names and `nemo_trace!`; calls at the provider choke points (`nemo-ytm`, `nemo-soundcloud`, `nemo-lyrics`, `nemo-spotify`, resolver tiers, cache, `yt-dlp`, sign-in stages); a test that every trace field is on the allow-list. _Requirements: 9, 10_
- [x] 3. `DevProbe`, `DevSnapshot` and `Backend.dev` in `nemo-core`; the `nemo-app` implementation (aggregates from the ring, facts from existing status calls, `Unknown` otherwise). _Requirements: 14, 15, 16, 17, 22_
- [x] 4. The click counter, the toast and `Ctrl+Shift+D` in `nemo-ui`. _Requirements: 1, 2, 3, 4_
- [x] 5. The panel: overlay, tabs `Trace`, `Log`, `Providers`, `Account`, `System`, `Copy report`, `Clear`; headless PNGs; the `design.md`, `coc.md` and `structure.md` lines. _Requirements: 11, 12, 13, 14, 15, 16, 18, 19, 20, 21_

## Definition of Done

See section 4 of `.kiro/steering/workflow.md`.
