# App MVP: addendum to the common agent brief

Read `.kiro/specs/foundation/agent-brief.md` first. Everything there applies, with these changes.

## What changed
- Crates allowed: the "App-MVP crates" row of `.kiro/steering/tech.md` (reqwest, image, rubato, lofty, rusqlite, unicode-normalization, tracing-subscriber, ytmapi-rs) plus the Utility crates row, plus `tokio` features your crate needs. Pin exact versions with `=` in your own `Cargo.toml` and verify them on crates.io. Any other crate: do not add it, report it.
- Contracts: `crates/nemo-core/src/{catalog,services,player,track,audio,lyrics,segment,stream}.rs` are the shared types. They are frozen for this wave. Do not edit them. Write needs in your report.
- Specs: `.kiro/specs/app-mvp/` (requirements, design, tasks). UI also reads `.kiro/steering/design.md` and `docs/ui-reference.md`.
- The dev machine has `yt-dlp` 2026.10.26 on PATH (installed with winget).
- Machine: Windows 11, 4 cores, about 7 GB RAM, Intel HD Graphics 530.

## Build isolation
Use your own target directory so agents do not block each other:
`$env:CARGO_TARGET_DIR = "$env:KIROCREW_SCRATCH\target-<your-crate>"` (plus the PATH and `CARGO_BUILD_JOBS=2` lines from the common brief).

## Real-network smoke tests (development only)
- Allowed: at most 10 requests in total to YouTube hosts and at most 10 to any other host, one at a time, at least 1 s apart, no loops, no accounts, no cookies. Put them in `#[ignore]` tests or examples. Run each at most twice.
- Never commit responses, URLs with signatures, or video ids you did not choose. Do not log URLs.
- Send `User-Agent: NemoMusic/0.0.1 (+https://github.com/nekoo-moe/NemoMusic)` to non-YouTube hosts.
- Write down what you observed (status codes, codecs, sizes) in your report.

## Runtime-agnostic futures
Service traits return futures that the UI polls on its own executor. If your code needs tokio (reqwest does), hold a `tokio::runtime::Handle`, run the work with `handle.spawn(...)`, and return a future that awaits the `JoinHandle`. Never call `block_on` inside a method that can run on the UI executor.

## Agreed names between crates (build exactly these)
- `nemo_ytm::catalog::YtmCatalog`: implements `Catalog`.
- `nemo_ytm::stream::HttpStreamOpener`: implements `StreamOpener`.
- `nemo_ytm::ytdlp::{YtDlpResolver, locate_yt_dlp}`: the resolver gets `with_allowed_codecs(&[Codec])`.
- `nemo_ytm::sponsorblock::ReqwestHttp`: implements `HttpGet`.
- `nemo_ytm::images::ImageCache`: implements `ImageSource`.
- `nemo_lyrics::service::LyricsHub`: implements `LyricsService`.
- `nemo_audio::player::{Player, PlayerDeps}` and `nemo_audio::decoder::supported_stream_codecs() -> &'static [Codec]`.
- `nemo_ui::run(backend: Backend, options: UiOptions)` and `nemo_ui::mock::mock_backend()` (both behind the `gui` feature).
The parent wires them together in `nemo-app`.

## Report
Same as the common brief, plus: what you observed on the real network, and what you could not check.
