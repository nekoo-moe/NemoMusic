---
inclusion: always
---

# NemoMusic Code Standards

Process rules (commits, branches, PRs) live in workflow.md. Crate layout is in structure.md. Stack choices are in tech.md.

## Toolchain
- Rust edition 2021 or newer, set once in `[workspace.package]`. Crates inherit it with `edition.workspace = true`.
- MSRV is pinned in `rust-version` in the workspace `Cargo.toml` and in `rust-toolchain.toml`. Policy: latest stable minus 2 releases. Raising it needs a note in CHANGELOG.
- CI builds with the MSRV and with latest stable.

## Format and lint
- `rustfmt.toml` is committed. Run `cargo fmt --all --check` in CI.
- CI runs `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Enable `clippy::all`, `clippy::pedantic`, `clippy::cargo` and `clippy::perf` as warnings via `[workspace.lints]`. Allowing a pedantic lint needs a one-line reason on the `#[allow]`.
- Each crate sets `[lints] workspace = true`.

## Errors
- Library crates (`nemo-core`, `nemo-audio`, `nemo-library`, `nemo-ytm`, `nemo-ui`) define typed errors with `thiserror`. They never return `anyhow::Error`.
- `anyhow` is allowed only in `nemo-app`, at the top level.
- No `unwrap()`, `expect()`, `panic!` or unchecked indexing outside tests. Exception: a proven invariant, written as `.expect("invariant: <why it holds>")`.
- Never swallow errors with `let _ =` unless a comment says why.

## Unsafe
- Every crate has `#![forbid(unsafe_code)]` by default.
- Only a module named `ffi` or `sys` inside `nemo-audio` may use `#![allow(unsafe_code)]`, and only for OS audio APIs (WASAPI and similar).
- Every `unsafe` block has a `// SAFETY:` comment stating the invariant and who upholds it.

## Real-time audio thread
Applies to every callback and function reachable from it. Mark those functions `/// RT-SAFE` in the doc comment.
- No heap allocation. Preallocate buffers before the stream starts. No `Vec::push` that may grow, no `String`, `Box::new` or `format!`.
- No locks (`Mutex`, `RwLock`), no channels that block, no `Arc` clone or drop that could free memory.
- No blocking I/O: no file, network or syscalls that can sleep. No `println!`.
- No logging, including `tracing` macros. Report events through an atomic flag or a counter that a non-RT thread reads.
- Data crosses threads only through a lock-free SPSC ring buffer. Decoding and network fetch run on separate threads and feed the ring buffer.
- Underrun is counted, never blocked on.
- Proof required for each RT path:
  - a test with a counting global allocator that asserts zero allocations during 10 seconds of simulated callbacks
  - a CI run with `assert_no_alloc` or an equivalent tool
  - a documented worst-case callback time in the benchmark

## Bit-perfect
- Default signal chain has no volume scaling, resampling, dither, normalization, EQ or crossfade. Each is a stage that exists only when the user enabled it, and it is bypassed with no arithmetic when off.
- Software volume defaults to off. Level control uses the OS or device.
- The output stage never changes sample rate or bit depth silently. If the device cannot accept the format, the UI shows the mismatch.
- Required test: feed a known PCM buffer through the default chain and assert the output is byte-identical to the input (`assert_eq!` on samples or a checksum).
- A stream is labelled lossless only when the source is a local lossless file. A YouTube Music stream is never labelled lossless in code, UI strings, logs or tags.

## Testing
- Unit tests sit beside the code in `#[cfg(test)]`. Integration tests go in `tests/`.
- Golden audio tests: fixtures in `tests/golden/` with a SHA-256 checksum file. Decoders must reproduce the checksum of the decoded PCM.
- Property tests with `proptest` for parsers, ring buffer ordering, and gain or format conversions.
- Benchmarks with `criterion` in `benches/`. Budgets, failing CI on regression above 10%:
  - decode plus ring-buffer fill is at least 50x realtime for 44.1/16 FLAC and uses under 3% of one core
  - the audio callback stays under 25% of its period
  - output latency is configurable, and the default buffer is 20 ms or less in exclusive or low-latency mode
- Tests never call the real YouTube Music service. Use recorded fixtures with no cookies or tokens.

## Logging
- Use `tracing` only. No `println!` or `eprintln!` outside CLI output in `nemo-app`.
- Levels: `error` for failures the user sees, `warn` for recoverable ones, `info` for lifecycle, `debug` and `trace` for development.
- Use structured fields, for example `tracing::info!(track_id = %id, "loaded")`.
- Never log secrets, cookies, auth headers, tokens or full request URLs with signed query strings. Wrap such values in a `Secret<T>` type whose `Debug` prints `[redacted]`.
- No secrets, cookies or session files in git. `.gitignore` covers them, and secrets are stored in the OS credential store.

## Dependencies
- `deny.toml` is committed. CI runs `cargo deny check` and `cargo audit`. Both must pass.
- License allowlist: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, MPL-2.0, Unicode-3.0, Zlib. Anything else needs an explicit exception with a reason in `deny.toml`.
- Use `default-features = false` and enable only the needed features. Pin versions in `Cargo.lock`, which is committed.
- A new dependency needs a one-line justification in the change description. Prefer std and existing crates. Reject unmaintained crates (no release in 2 years).
- Run `cargo machete` or `cargo udeps` to catch unused dependencies.

## Docs and size limits
- Every `pub` item has a `///` doc comment. Each crate sets `#![warn(missing_docs)]`. Public functions that can fail list `# Errors`, and any that can panic list `# Panics`.
- A file stays under 500 lines and a function under 60 lines. Split when exceeded.
- Cognitive complexity per function stays at or below 15, enforced by `clippy::cognitive_complexity`. A function takes at most 5 parameters, otherwise use a struct.
