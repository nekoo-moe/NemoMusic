# ADR 0004: Crates and shared rules for the 2026-11-14 feature wave

Status: proposed - owner approves it together with the designs

## Owner decisions recorded
- All 17 features in `product.md` (Scope, "Approved additions") are approved (owner, 2026-11-14).
- The owner accepted the recommended answers to the requirements' open questions (2026-11-14). Each spec's `requirements.md` records the answers that touch it.
- Song recognition stays in, and the owner accepts its patent and Terms of Service risk.

## Context
`structure.md` allows `nemo-app` to depend on every crate, and every other crate to depend on `nemo-core` only. Library, ytm and lyrics crates must not depend on each other. The wave adds services (SoundCloud, Spotify, Last.fm, AI, Cast, rooms, recognition), local data (library, stats, offline cache) and audio stages (effects, crossfade, exclusive output). Seventeen designs written in parallel need one map, or they will put the same thing in different places.

## Decision 1: where the code lives
Existing crates take what is clearly theirs:

| Crate | Takes |
|---|---|
| `nemo-core` | New shared types and traits only (see Decision 2). No I/O. |
| `nemo-audio` | Effects chain (EQ, delay, reverb, ReplayGain gain), crossfade mixer, device switching, WASAPI exclusive sink, a file `ByteSource` for local and cached files, more decoders (ALAC and Vorbis through `symphonia` features, Opus through a decoder crate). |
| `nemo-library` | The SQLite library, folder watching, local playlists and M3U, CUE sheets, tag editing, stored ReplayGain values. |
| `nemo-ytm` | Offline cache (downloads and index), dislike and playlist edits, several accounts, Premium session for `yt-dlp`, audio-language choice, YouTube search, live streams, comments, Return YouTube Dislike, YouTube captions as a `LyricsProvider`, and a generic `yt-dlp` URL resolver behind a `nemo-core` trait. |
| `nemo-lyrics` | Own lyrics, romanization, the translation hook (calls a `nemo-core` trait, never an AI crate). |
| `nemo-ui` | Every screen. Talks to services only through `nemo-core` traits and messages. |
| `nemo-app` | Wiring: builds each service and injects it into the others through `nemo-core` traits. Still no business logic. |

New crates, each depending on `nemo-core` and its own third-party crates only:

| New crate | Holds |
|---|---|
| `nemo-secrets` | The OS credential store. The `SecretStore` trait moves from `nemo-ytm/src/credentials.rs` to `nemo-core`; this crate holds the Windows and Linux implementations. Every crate that needs a secret gets one through the trait. |
| `nemo-soundcloud` | SoundCloud catalog and pages. Stream lookup uses the `nemo-core` URL-resolver trait that `nemo-ytm` implements with `yt-dlp`. |
| `nemo-spotify` | The user's Spotify session, Spotify lyrics (a `LyricsProvider`) and Canvas (an animated-art trait). |
| `nemo-stats` | The local play log (SQLite) and every aggregate: charts, clock, receipt, year in music, recaps. |
| `nemo-integrations` | OS media controls and media keys, Last.fm, Discord Rich Presence. |
| `nemo-ai` | The client for the endpoint the user sets, behind a `nemo-core` trait used by lyrics translation and suggestions. |
| `nemo-recognize` | Microphone and loopback capture, the fingerprint, the recognition request. |
| `nemo-cast` | Cast and DLNA discovery and control, and the LAN-only stream server. |
| `nemo-together` | The Listen Together room client. |
| `nemo-video` | Video decoding. `nemo-ui` shows frames through a `nemo-core` trait, never by depending on this crate. |

A design may merge two of these or add a module elsewhere if it says why. A design that needs a crate-to-crate edge other than "depends on `nemo-core`" must stop and raise it.

## Decision 2: shared traits in `nemo-core`
`nemo-core` files are shared. Each design names the traits and types it adds, signatures only. At implementation time only one task at a time edits a given `nemo-core` file, and the task that adds a trait lands before the tasks that use it. Expected additions: `SecretStore`, a URL resolver for `yt-dlp`, a new `Origin` value for SoundCloud, a translator, an animated-art source, a play-log and stats service, a downloads service, a remote-playback service (Cast and rooms), a recognizer, and video frames.

## Decision 3: rules every design follows
- Storage: one SQLite file per owning crate (as `nemo-lyrics` does now), under the app data folder, with migrations. No shared database.
- Secrets: only through `SecretStore`, wrapped in `Secret<T>` (`nemo-core/src/secret.rs`) so they never print.
- Opt-in: each opt-in feature has one settings entry with the switch, what is sent, to which hosts, and its risk text, shown before the first request. One shared UI pattern for all of them.
- Network: every new host goes on the README egress list in the task that first contacts it.
- Audio: every stage that alters samples registers in the signal path, is off by default and does no arithmetic when off. The real-time rules of `code.md` hold. The bit-exact test passes with every stage off.
- Sign-out: every crate that keeps account data registers a wipe step that sign-out runs.
- Steering: `tech.md` (crates) and `structure.md` (new crates and edges) change in the implementing task, as their change rules say. Designs list the edits.

## Consequences
- `nemo-ytm` loses its credential module to `nemo-secrets` and `nemo-core`; its sign-in flow is otherwise unchanged.
- The crate count grows from 8 to 18. Each new crate is small and testable alone.
- `nemo-app` gets more wiring. If it passes 500 lines per file, it splits as `code.md` asks.
