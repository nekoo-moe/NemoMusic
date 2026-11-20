# Developer tools: design

## Overview

Three parts, in this order of data flow.

1. Capture (`nemo-app`, module `dev_capture`): a `tracing` layer next to the existing file logger. It stores every event that passes its own per-layer filter in a bounded ring (2000 events, oldest dropped). It keeps a fixed allow-list of fields and redacts all text at capture. The ring lives in memory only.
2. Trace (`nemo-core` macro and field names, calls in the provider crates): one event convention, target `nemo_trace`, level `debug`, so the log file at the default `info` level does not change while the ring still receives it.
3. Panel (`nemo-ui`, `gui/views/dev/` and `gui/app/dev.rs`): an overlay that reads one `DevSnapshot` from the `DevProbe` the app supplies. The probe is implemented in `nemo-app` and reads the ring and the public status calls of the crates.

The unlock is a click counter on the About version text. The panel looks like the other sheets.

## Affected Crates

- `nemo-core`: new `dev` module (`DevProbe`, `DevSnapshot` and its row types, the trace field names, the `nemo_trace!` macro) and the `Backend.dev` field.
- `nemo-app`: `dev_capture` (ring, redaction, layer, filter), `logging::init` builds the subscriber from layers, the `DevProbe` implementation.
- `nemo-ytm`, `nemo-soundcloud`, `nemo-lyrics`: small mechanical calls of `nemo_trace!` at their choke points, each crate with its own `trace.rs` for the helper functions. `nemo-spotify` has no network code yet (an empty crate), so it has nothing to trace; its HTTP layer gets the same call when it is written.
- `nemo-ui`: the panel, the click counter, the toast, the `Ctrl+Shift+D` shortcut, the headless render of each tab.

## Data Flow

- Any thread calls `tracing::debug!(target: "nemo_trace", ...)` (through the macro). The capture layer visits the fields, drops every field not on the allow-list, redacts the text, and pushes into the ring under one short `Mutex` lock. No audio thread calls it (`code.md`).
- The subscriber is `registry` plus two layers, each with its own filter:
  - the file layer keeps the `NEMO_LOG` level exactly as before (default `info`);
  - the capture layer takes every event of target `nemo_trace` at `debug` or above, and every other event up to the larger of `info` and the `NEMO_LOG` level (but never below `info`). Its max-level hint is `debug` at most, so `trace`-level callsites of other crates stay off unless `NEMO_LOG=trace`.
- The panel asks `DevProbe::snapshot()` when it opens, after each data change message and on the 1 s tick while it is open and focused. A snapshot is a plain value: the ring copied and ordered, per-provider aggregates computed from the last 50 trace rows of each provider, and the facts gathered from existing status calls.
- `Copy report` formats the snapshot as plain text in the UI crate, passes it through a `Clipboard` trait (the real one in `nemo-app`, a fake in tests) and shows `Copied`. The snapshot is redacted when the probe builds it, and the finished report text goes once more through `DevProbe::redact` (the same function as the capture pass), so a late-added field cannot slip through.
- The click counter is pure state in `gui/app/dev.rs`: `count`, `last_click_ms`; the clock is the app clock (injected, testable).

## Interfaces

Signatures only.

```rust
// nemo-core/src/dev.rs
pub const TRACE_TARGET: &str = "nemo_trace";
pub const TRACE_FIELDS: &[&str];            // provider, op, host, status, ms, bytes, tier, attempt, rest
#[macro_export] macro_rules! nemo_trace { ... }   // expands to tracing::debug!(target: "nemo_trace", ...)

pub trait DevProbe: Send + Sync {
    fn snapshot(&self) -> DevSnapshot;
    fn clear(&self);                         // empties the in-memory ring, nothing else
    fn redact(&self, text: &str) -> String;  // the capture-time redaction pass, for the report text
}
pub struct DevSnapshot { /* plain strings, numbers, enums only */ }
pub struct DevTraceRow { at_ms, provider, op, host, status, ms, tier, bytes, attempt, rest, is_error }
pub struct DevLogLine { at_ms, level: DevLevel, target, text }
pub struct DevProviderCard { name, last_ok_ms, last_error_kind, errors, p50_ms, p95_ms, rest }
pub struct DevAccountFacts { site, signed_in, token_present, token_len_class, cookie_names, last_stage, last_outcome }
pub struct DevSystem { version, os, profile, config_keys, egress_hosts, uptime_s, audio, frame_stats }
pub enum DevLevel { Error, Warn, Info, Debug, Trace }
// Backend: pub dev: Option<Arc<dyn DevProbe>>

// nemo-app/src/dev_capture.rs
pub struct DevRing;                           // cheap clone, Arc inside
impl DevRing { pub fn new(capacity: usize) -> Self; pub fn push(&self, e: CapturedEvent);
               pub fn snapshot(&self) -> Vec<CapturedEvent>; pub fn clear(&self); pub fn len(&self) -> usize; }
pub struct Redactor;
impl Redactor { pub fn new() -> Self; pub fn with_home(home: &str) -> Self; pub fn redact(&self, text: &str) -> String; }
pub struct CaptureLayer<S>;                   // tracing_subscriber::Layer
pub fn capture_filter(log_level: Level) -> impl Filter<S>;
```

Allow-listed fields (anything else is dropped at capture): `provider`, `op`, `host`, `status`, `ms`, `bytes`, `tier`, `attempt`, `rest`, `kind`, `stage`, `outcome`, `error`, `reason`, `count`, `version`. Values are redacted, cut to 160 characters, and the message to 300 characters.

Redaction rules, in order, applied to messages and field values:

1. URL (`scheme://...`): replaced by its host without user info and port; `file://` becomes `[path]`.
2. `Authorization`, `Cookie`, `Set-Cookie` headers: the rest of the line becomes `[redacted]`. `Bearer <token>` becomes `Bearer [redacted]`.
3. `key=value` where the key is a known secret or id key (`token`, `oauth_token`, `sapisid`, `sp_dc`, `cookie`, `password`, `secret`, `key`, `client_id`, `id`, `video_id`, `list` and the `__Secure-` and `__Host-` families): the value becomes `[redacted]`.
4. E-mail addresses become `[email]`.
5. The home folder (`$HOME`, `%USERPROFILE%`) and the generic `/home/<name>`, `/Users/<name>`, `X:\Users\<name>` prefixes become `~`.
6. Id-like runs of `[A-Za-z0-9_-]`: 32 or more characters, or 20 or more with a digit and a letter and either a capital letter or no `-` and `_`, or the UUID shape, become `[id]`.
7. Control characters become spaces.

The function is idempotent (a test checks it) and never panics on any input.

## Error Handling

- A poisoned ring lock is recovered (the ring holds plain data); capture never panics and never logs from inside the layer.
- If the log file cannot be opened the file layer writes to stderr as today; the ring works either way.
- If no `DevProbe` is supplied (tests, a host without it) the panel shows `Unknown` for every fact and an empty trace.
- A facts gatherer that fails gives `Unknown`, never an error toast.

## Testing Strategy

- Click counter: 10 clicks open once, a pause of 3 s resets, a click elsewhere in between does not matter, the 11th click does not reopen.
- Redaction: a table of nasty strings (URLs with userinfo, ports, queries and fragments, bearer and cookie lines, e-mails, home paths on Linux and Windows, long ids, UUIDs, JWT, Unicode, empty and very long input) and `proptest` for no panic, idempotence and "planted secret never appears".
- Ring: bound, order, clear.
- Layer: a subscriber with the file layer at `info` and the capture layer, an event of target `nemo_trace` at `debug` reaches the ring and not the file buffer; the same callsite is first used under an `info`-only subscriber to prove that cached callsite interest does not hide it; non-allow-listed fields never appear.
- `DevSnapshot` holds no free-text type that a secret could fill, checked by a test that builds a snapshot from events carrying secret-shaped fields.
- Headless render (PNG) of each tab with sample data, dark and light, 960 x 600 and 1366 x 697.
- Copy: a fake `Clipboard`; the test checks that Copy calls it once and that the file system and the log are untouched.
- No audio code changes, so no bit-exact test.

## Risks and Alternatives

- Per-layer filters and callsite interest: a callsite first seen by a subscriber that disabled it can stay cached as `never`. Mitigation: the capture filter is a pure function of the metadata, the registry combines both layers' interests, and a test covers the sequence. Alternative (rejected): emit trace events at `info` and filter them out of the file; it changes the file content.
- Cost: always-on capture adds one filter check per callsite and one lock plus a few small allocations per kept event. Provider calls run at human pace, so this is small; nothing is captured below `debug` from other crates by default.
- A second OS window (rejected): needs `iced::daemon`; out of scope.
- A disk trace file (rejected): conflicts with the in-memory, local-only decision.
- Redaction can miss a secret shape. The allow-list of fields, the fixed vocabulary of trace values and the second redaction pass at report time are the other layers; the copied report is also read by the owner before it is pasted.
