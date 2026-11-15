Spec approved by the owner on 2026-11-15.

## Tasks

- [x] 1. Ship the sign-in window inside `nemo-app`. _Requirements: 1, 2, 3, 4, 5, 6, 7_
  - Crate and files: `nemo-signin` (`src/lib.rs`, was `src/main.rs`; `Cargo.toml`); `nemo-ytm` (`src/account.rs`, `src/account/tests.rs`, `tests/diagnose_*.rs`); `nemo-app` (`src/main.rs`, `src/backend.rs`, `Cargo.toml`); `README.md`; `.github/workflows/release.yml`; `.kiro/steering/structure.md`, `tech.md`.
  - Does: library `nemo_signin::run`; `main` dispatches `--signin` first; `AccountConfig.helper_args`; the app passes `current_exe()` and `--signin`; release packs only `nemo-app`.
  - Checks: fmt; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo test -p nemo-signin -p nemo-ytm -p nemo-app --all-features`; `nemo-app --signin` exits 64 with bad arguments and 3 with no display.
  - Steering: structure.md (nemo-signin row, layout, graph), tech.md (Sign-in window row).

## Definition of Done

See [workflow.md section 4](../../steering/workflow.md#4-definition-of-done).
