Spec approved by the owner on 2026-11-15.

## Tasks

- [x] 1. Install and update the app-managed yt-dlp copy. _Requirements: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11_
  - Crate and files: `nemo-ytm` (`src/ytdlp/managed.rs`, `src/ytdlp/managed/tests.rs`, `src/ytdlp_install.rs`, `src/ytdlp.rs`, `src/ytdlp/locate.rs`, `examples/install_yt_dlp.rs`); `nemo-app` (`src/config.rs`, `src/backend.rs`); `README.md`; `.github/workflows/release.yml` (release notes text); `.kiro/steering/tech.md`.
  - Does: background install when nothing is found, Linux one-file and Windows onedir builds, SHA-256 check, `ProgramSlot` switch without restart, update check at most once per 24 h for the managed copy only, `yt_dlp_auto_install` switch (default on).
  - Checks: fmt; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo test -p nemo-ytm -p nemo-app --all-features`.
  - Steering: README egress rows `github.com`, `release-assets.githubusercontent.com`; tech.md run-time download note.
- [ ] 2. Follow-up: a settings row for `yt_dlp_auto_install` in `nemo-ui`, with the hosts it contacts and a status line ("yt-dlp installed", "yt-dlp missing"). _Requirements: 8_

## Definition of Done

See [workflow.md section 4](../../steering/workflow.md#4-definition-of-done).
