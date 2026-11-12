# Foundation: common brief for implementation agents

Read this file first. Each agent's own task text names its crate and its slice.

## Setup
- Project: the repository root (the folder with the workspace `Cargo.toml`), a Rust Cargo workspace (`crates/nemo-*`). Windows 11 with PowerShell, or Linux.
- Rust is pinned in `rust-toolchain.toml` (1.99.1; `rustup toolchain install` in the repo root adds it). In every new shell run first:
  `$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path ; $env:CARGO_BUILD_JOBS = '2'`
- Other agents may build at the same time. Check cores and free memory first and keep `CARGO_BUILD_JOBS` low (2 on a 4-core machine with 6 GB RAM). Cargo may wait on the build-directory lock. Wait. Do not kill other cargo processes.

## Read first
`.kiro/steering/` (code.md, structure.md, tech.md, workflow.md, coc.md, product.md; design.md for UI), `.kiro/specs/foundation/` (requirements, design, tasks), the matching ADR in `docs/adr/`, and `crates/nemo-core/src/*.rs` (the shared types you must use).

## Rules
- You own ONLY the files named under YOUR CRATE. Do NOT edit: the root `Cargo.toml`, `Cargo.lock` by hand, `nemo-core`, other crates, steering files or specs. If `nemo-core` lacks something, do not change it. Write the need in your final report.
- Crate-specific dependencies go in your own `Cargo.toml`: exact pins with `=`, `default-features = false` where possible, latest stable version checked on crates.io (`https://crates.io/api/v1/crates/<name>` with a User-Agent header, or docs.rs). Only crates named in `tech.md`, including its "Utility crates" row. Any other crate: do NOT add it. Report it.
- Use `nemo-core.workspace = true` and `[lints] workspace = true`.
- No `unsafe` unless your task says so. No `unwrap`/`expect` outside tests (`expect("invariant: ...")` with a reason is allowed). Every `pub` item has a doc comment. No `println!`/`eprintln!` except in examples.
- No secrets, cookies or real YouTube ids/URLs in code or fixtures. Fixtures are synthetic and written by you.
- Tests make NO network call and need NO audio device or GPU. Tests that need one are `#[ignore]`.
- Do not run `git commit` or `git push`. Run `cargo fmt`.
- Never claim you heard audio or saw a window if you could not.

## Done means
From the repo root, check `$LASTEXITCODE` right after each cargo command, without piping it:
1. `cargo fmt --all -- --check`
2. `cargo clippy -p <your-crate> --all-targets -- -D warnings`
3. `cargo test -p <your-crate>`

If a pedantic clippy lint is wrong for your code, fix the code. Use `#[allow]` only with a one-line reason.

## Final report (max 40 lines, plain text)
Files created. The three command results. Dependencies added (version, license, one-line reason). Anything you could not verify. Deviations from the brief. Needs for `nemo-core`. What you did NOT test.
