## Overview

A new crate `nemo-recognize` captures a few seconds of sound, turns it into a Shazam-style signature on this computer, and posts only that signature to `amp.shazam.com`. The user then picks a match in NemoMusic's own catalog. Nothing runs until the user taps "Recognize". The input stream is closed when the run ends or is cancelled, so the playback path is never touched (criteria 3, 4, 22).

The algorithm is ported, not imported. SongRec 0.8.2 is GPL-3.0+ (this repo is GPL-3.0, so a port with attribution is compatible), but it is an application crate that pulls GTK, libsoup and gettext, and a GPL crate dependency would need a `deny.toml` exception. We port three files (`algorithm.rs`, `hanning.rs`, `signature_format.rs` under `src/core/fingerprinting/`) into `nemo-recognize/src/fingerprint/` with an SPDX header and a `NOTICE` that names SongRec and its author. We do not port `communication.rs`: it sends a fixed location (45, 2), the time zone "Europe/Paris" and a random User-Agent, which criterion 7 forbids.

Capture is up to 12 seconds, in at most 3 requests (criterion 24). The audio is mono 16 kHz in memory only.

First task, before any other code: a `#[ignore]` live test that posts one signature of a short clip the owner supplies, with no `geolocation` and no `timezone` field, and records the answer in this file. If the service refuses, the design stops and raises it to the owner (criterion 7). Nothing false is sent to get an answer.

## Affected Crates

- `nemo-recognize` (new): capture, signature, request, history database.
- `nemo-core`: new `recognize.rs` with the `Recognizer` trait and its types.
- `nemo-ui`: Recognize button, Listening state with Cancel, result card, history list, settings entry, keyboard shortcut, reduced-motion indicator. Talks only to `Recognizer`.
- `nemo-app`: builds `nemo-recognize`, injects it, runs the cover lookup and "Find in NemoMusic" by calling `Catalog::search` and the library search. It registers no sign-out step: the history holds no account data (criterion 19; ADR 0004, Decision 3).
- `nemo-audio`: no change. `nemo-ytm`, `nemo-library`, `nemo-lyrics`: no change.

## Data Flow

- UI thread calls `Recognizer::recognize`. It returns a future that runs on the crate's own tokio runtime.
- A capture thread owns the `cpal` input stream (cpal 0.18.2, already in `Cargo.lock`). The input callback only converts to f32 mono and pushes into an `rtrb` ring (no allocation, lock or log in the callback, as in `nemo-audio`). The ring holds 12 s at the device rate.
- A worker thread drains the ring, resamples to 16 kHz with `rubato`, and keeps one `Vec<f32>` of at most 12 s. That vector is the only copy of the audio. It is dropped when the run ends. Nothing is written to disk or logs (criterion 5).
- At 5 s, 9 s and 12 s of audio the worker builds a signature from the whole window (`SignatureGenerator::make_signature_from_buffer`) and the async side posts it. A match, "No match" after the third request, a cancel or an error ends the run: the stream is dropped first, then the result is sent.
- The request is built from the signature only: `signature.uri`, `signature.samplems`, `signature.timestamp`, and `timestamp`, all real. The path has fresh random UUIDs per request, as the route requires. Headers: `User-Agent: NemoMusic/<version>`, `Content-Type: application/json`. No cookies. The cookie store of `reqwest` stays off.
- Loopback: on Windows the same cpal call opens the default output device as an input (WASAPI loopback). On Linux it opens the monitor source. If cpal 0.18.2 lacks loopback on a platform, `wasapi` 0.27.0 (already approved) covers Windows. The cpal part is unverified; the first capture task checks it.
- Cover: after a match, `nemo-app` runs one `Catalog::search("<title> <artist>")`, the same request "Find in NemoMusic" makes, and takes the first song whose normalized title and first artist equal the match. The result card shows that song's YouTube Music cover through `ImageSource`; with no equal song it shows a placeholder. No image is fetched from the recognition service, so no Shazam cover host exists. The search goes to `music.youtube.com`, which is listed already; the settings text names it (criterion 2).
- History: `recognition.sqlite3` in the app data folder, owned by `nemo-recognize` (`rusqlite` 0.41.1, WAL, migrations). One table `history(id, title, artist, recognized_at, source)`, capped at 500 rows.

## Interfaces

```rust
// nemo-core/src/recognize.rs  (no I/O, no runtime)
pub enum InputSource { Microphone { id: String, name: String }, ComputerSound }
/// No cover field: the cover comes from the YouTube Music search result (criteria 8, 26).
pub struct RecognizedSong { pub title: String, pub artist: String }
pub enum RecognitionState { Idle, Listening { seconds: u8 }, Matching, Done }
pub struct CancelFlag(/* Arc<AtomicBool> */);
impl CancelFlag { pub fn new() -> Self; pub fn cancel(&self); pub fn is_cancelled(&self) -> bool; }
pub struct HistoryEntry { pub id: i64, pub title: String, pub artist: String, pub at: SystemTime, pub source: HistorySource }
pub enum HistorySource { Microphone, Computer }

pub trait Recognizer: Send + Sync {
    fn sources(&self) -> Vec<InputSource>;
    fn state(&self) -> RecognitionState;
    fn recognize<'a>(&'a self, source: &'a InputSource, cancel: CancelFlag)
        -> BoxFuture<'a, Result<Option<RecognizedSong>, RecognizeError>>; // None = "No match"
    fn history(&self) -> Vec<HistoryEntry>;
    fn delete_entry(&self, id: i64);
    fn clear_history(&self);
}

// nemo-recognize
pub struct ShazamRecognizer { /* capture, http client, history db */ }
impl ShazamRecognizer { pub fn new(data_dir: &Path, user_agent: String) -> Result<Self, RecognizeError>; }
pub fn make_signature(mono_16khz: &[f32]) -> DecodedSignature;
pub fn request_body(sig: &DecodedSignature, now_ms: u64) -> String;
```

## Error Handling

`RecognizeError` (`thiserror`, in `nemo-core`): `NoInputDevice`, `PermissionDenied` (UI points to Windows "Microphone privacy" or the Linux sound settings, criterion 13), `Offline`, `Unreachable`, `RateLimited` (HTTP 429), `ServiceRefused { status }`, `BadAnswer`, `Cancelled`, `Storage`. No retry loop: a 429 or a network error ends the run. A new run waits 10 s after the last one (criterion 24). Error text holds no URL, fingerprint or cookie. The response body is parsed with `serde_json` into the three fields we use; an unknown shape is `BadAnswer`.

## Testing Strategy

- Signature: a golden test feeds a fixed synthetic 12 s buffer and compares the peak list with values produced by SongRec's own code at the ported commit (recorded in `NOTICE`).
- Request: tests assert the body holds no `geolocation` and no `timezone` key, and the User-Agent is `NemoMusic/<version>` (criteria 6, 7).
- Capture: a fake input source feeds the worker. Tests check at most 12 s kept, at most 3 requests, 4 s spacing, the stream dropped before the result, and the 10 s cool-down. Another test checks no file is created in the data folder during a run.
- Real-time path: a counting-allocator test on the capture callback proves no allocation.
- HTTP: a local test server returns a match, no match, 429, and a broken body.
- History: cap at 500, newest first, delete one, clear all; sign-out leaves it (criterion 19).
- Audio is not touched. `cargo test -p nemo-audio --test bit_exact` must still pass.
- `#[ignore]` live test against the service, run by hand only (criterion 25).

## Risks and Alternatives

- The service is unofficial. It can change, block clients or refuse requests without a location and time zone. Then the feature stays unavailable (criterion 7).
- Patent and Terms of Service risk. The owner accepted it (`coc.md`). The settings text names both before the first request.
- The real User-Agent may be blocked where a browser-like one is not. We do not impersonate; we raise it.
- The path segment `en/US/android` belongs to the service's route. The first task checks whether other values work; if not, it stays a constant and the owner is told.
- `rustfft` 6.4.1 (pulled by `realfft`) was released 2025-09-18, which is inside the 2 years `code.md` allows (ADR 0004, Decision 5). It is pure Rust and widely used. It needs no exception and no hand-written FFT.
- Alternative rejected: depend on the `songrec` crate. GPL crate, GUI dependencies, and it would need a `deny.toml` exception.
- Alternative rejected: `chromaprint` bindings. They fingerprint for AcoustID, not for this service.

## Steering and README changes

- Opt-in: the recognition switch uses the shared `OptInRow` and `OptInInfo` (ADR 0004, "Opt-in settings pattern"). Sends: an audio fingerprint, its length, the time, the app name and version; then the matched title and artist as a YouTube Music search. Hosts: `amp.shazam.com`, `music.youtube.com`. Risk: unofficial service, terms, software patents outside the EU (criterion 2).
- `tech.md` rows (add): `realfft` =3.5.1 (MIT, released 2026-10-26, MSRV 1.61) with `rustfft` =6.4.1 (MIT OR Apache-2.0, released 2025-09-18, MSRV 1.61, no exception); `uuid` 1.29.2 with feature `v4` (for the random path ids; already locked), `crc32fast` =1.5.5 and `base64` at the versions already in `Cargo.lock`; `rubato` and `cpal` at the versions `nemo-audio` pins. The ported code is GPL-3.0+; the row says so. Little-endian packing uses `u32::from_le_bytes`, so `byteorder` (last release 2023) is not added.
- `structure.md`: add `nemo-recognize` (depends on `nemo-core` only) to the layout, ownership list and the dependency graph under `nemo-app`.
- README egress list: `amp.shazam.com` (fingerprint, only on user action). No cover host: covers come from YouTube Music (criterion 26). The settings text says that the matched title and artist are searched on `music.youtube.com`, which is listed already.
- ADR 0004: no change; the `Recognizer` trait is Decision 4 row 40.

<!-- Sources:
SongRec repo and files (algorithm.rs, communication.rs, Cargo.toml: GPL-3.0+, rust-version 1.92): https://github.com/marin-m/SongRec
crates.io API for songrec 0.8.2, realfft 3.5.1, rustfft 6.4.1, rubato 5.1.0, base64 0.23.1, byteorder 1.5.0, microfft 0.6.0, chromaprint 0.2.0 (fetched 2026-11-14)
Repo: crates/nemo-core/src/services.rs (BoxFuture trait style), crates/nemo-audio (rtrb ring, real-time rules), .kiro/steering/code.md, coc.md, tech.md
Owner answers 2026-11-14 (task text)
-->
