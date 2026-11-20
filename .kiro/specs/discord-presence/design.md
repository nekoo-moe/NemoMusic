## Overview

Presence is a pure pipeline with a thin I/O shell. Play events and window facts become a `PresenceState`; `build` turns it into an `Activity` (all texts, images and the private form); a rate limiter, a sleeping rule and a dedupe decide when to send; one worker thread owns the connection to the local Discord app and does only I/O. The pure parts hold all the rules and have fake clocks, so they are tested without a Discord client.

The IPC client is written in this crate (safe code, `std` only) instead of using `discord-rich-presence` =1.1.0. Reasons: the crate pulls a second `uuid` (0.8) and its blocking reads cannot be bounded on a Windows pipe; `cargo deny` is not installed on the build machine, so the crate could not be checked here; the protocol is small (a handshake, `SET_ACTIVITY`, close). No new crate, so `tech.md` is unchanged.

Rules of the pure parts (names are in `crates/nemo-integrations/src/discord/`):

- `text.rs`: `fit` strips control characters, cuts to 128 bytes on a character boundary with `…`, pads to 2 bytes with spaces.
- `activity.rs`: `Activity` and its JSON, in the field names Discord's IPC takes (`type`, `details`, `state`, `timestamps.start/end` in seconds, `assets.*`).
- `state.rs`: `PresenceState`, `Song`, `Source`, `Place`, `Browsing` and `build(&PresenceState, private) -> Activity`. Active texts, all English:

| State | Type | Line 1 (`details`) | Line 2 (`state`) | Times | Large image (tooltip) | Small image (tooltip) |
|---|---|---|---|---|---|---|
| Listening | Listening | title | artist | start, end if length known | cover URL or `nemo` (`NemoMusic`, or the cast, together and sleep timer facts joined by ` · `) | source key (source name, plus the codec for lossless local files) |
| Paused | Listening | title | `Paused · ` artist | none | as above | `paused` (`Paused`) |
| Video | Watching | title | `Watching a video` (`Watching a video · Paused`) | as Listening, none when paused | `nemo` | none |
| Idle | Playing | `Idle` | `Nothing playing` | start = idle since | `nemo` | none |
| Browsing | Playing | `Browsing Library` or `Searching` | `Nothing playing` | start = idle since | `nemo` | none |
| Sleeping | Playing | `Away` | `Sleeping` | none | `nemo` | none |
| Private (any state with a title) | Listening | `Listening to NemoMusic` | none | none | `nemo` (`NemoMusic`) | none |

- `limiter.rs`: `RateLimiter<T>`: `offer(now, item)` sends at once when the last send is at least `gap` old (or none), else keeps the newest item; `poll(now)` releases it when due; `wait(now)` says how long; `force` sends at once (leaving Away); `reset` makes the next item go at once (after a connect).
- `away.rs`: `AwayRule` (the sleeping rule): quiet starts when the window is not focused AND no song plays (stopped, paused or empty); any focus or play ends it; asleep once quiet lasted 10 minutes. `wake_in` tells the worker when to wake.
- `session.rs`: `Session` folds `PlayHookEvent`s (`TrackStarted`, `StateChanged`, `PositionJumped`, `DurationKnown`) and the app context (`Context`: window focus, section, sleep timer, place, private) into a `PresenceState`. Start time = wall time now minus position, computed when the event arrives, so a held update still shows the right progress when it is sent. `Buffering` keeps the shown state. A cover comes from `pick_thumbnail(.., 512)` of a remote artwork; a local file has none.
- `engine.rs`: `Engine::step(now, wall, Input) -> Vec<Action>` joins session, away rule, limiter, dedupe and the connect schedule. Inputs: `Play`, `Context`, `Switch`, `Connected(bool)`, `Tick`. Actions: `Connect`, `Set(Activity)`, `Clear`, `Disconnect`. No clock, no I/O.

## Affected Crates

- `nemo-integrations`: the module `discord` (all of the above, plus `ipc.rs` the frame codec and the socket or pipe, and `worker.rs` the thread); `service.rs` (switch, private switch, observer fan-out, store); `info.rs` (notice text).
- `nemo-core`: additive default methods on `IntegrationsService` (`discord_private`, `set_discord_private`, `window_context`) and the type `WindowContext`. Implementations that do not know them keep working (default bodies). This is the only change to the public API of `nemo-core`.
- `nemo-app`: builds the real `Integrations` with the integrations folder (replacing `NoIntegrations`), registers it as a play observer, and stops it at exit (one hook in `backend.rs`).
- `nemo-ui`: the Discord row and the `Hide song names` switch in Settings, Privacy (`views/settings/discord.rs`), the messages that flip them, and one call that tells the service about the window (focus, section, sleep timer) next to the existing `Message::Focus` handling.

## Data Flow

```
 PlayHub observer thread --on_event--> Mailbox (bounded 64, drop oldest) --> discord worker thread
 UI thread -- window_context / set_discord / set_discord_private --> Mailbox --^
                                                                          |
        worker: Engine::step(now, wall, input) -> Actions -> ipc (socket or pipe) -> local Discord app
```

- The mailbox is a mutex-guarded `VecDeque` with a condition variable; a producer never waits for I/O. Nothing runs on the audio thread. The thread exists only while the switch is on (started by the first `Switch(true)`, ended by `Switch(false)` or the app's exit; both clear the presence first).
- The worker waits with a timeout of `min(limiter.wait, away.wake_in, reconnect wait, 1 s)`; with the switch on and a song playing it wakes at most once a second and does nothing on most wakes.
- Connect: Unix socket `discord-ipc-0..9` in `$XDG_RUNTIME_DIR`, `$XDG_RUNTIME_DIR/app/com.discordapp.Discord` (Flatpak), `$XDG_RUNTIME_DIR/snap.discord` (Snap), `$TMPDIR`, `/tmp`; Windows named pipe `\\.\pipe\discord-ipc-0..9`. Handshake frame (opcode 0, `{"v":1,"client_id":"<id>"}`), wait for `READY`, then `SET_ACTIVITY` frames (opcode 1). A reader thread per connection forwards incoming frames into the mailbox (answers to `PING` are sent by the worker; a closed pipe becomes `Connected(false)`). A frame longer than 64 KiB closes the connection.

## Interfaces

```rust
// nemo_integrations::discord
pub fn fit(text: &str) -> String;
pub struct Activity { /* kind, details, state, start, end, images */ }
impl Activity { pub fn to_json(&self) -> serde_json::Value; }
pub enum PresenceState { Listening{..}, Paused{..}, Video{..}, Idle{..}, Browsing{..}, Sleeping }
pub fn build(state: &PresenceState, private: bool) -> Activity;
pub struct RateLimiter<T> { /* gap, sent_at, pending */ }
pub struct AwayRule { /* after, quiet_since, active, playing */ }
pub struct Engine; pub enum Input { .. } pub enum Action { .. }

// nemo_core::integrations (additive, default bodies)
pub struct WindowContext { pub focused: bool, pub browsing: Option<BrowsingSection>, pub sleep_timer: bool }
pub trait IntegrationsService { fn discord_private(&self) -> bool { false }
    fn set_discord_private(&self, on: bool) {}  fn window_context(&self, ctx: WindowContext) {} }
```

Portal assets the owner uploads (Discord developer portal, Rich Presence, Art Assets, names exact): `nemo` (the app icon, 1024 px), `youtube-music`, `soundcloud`, `local`, `paused` (small icons, 512 px).

## Error Handling

Connect, write and read errors are silent: the worker marks the connection lost, sets the retry time (30 s) and logs one debug line per change of the connected state. A frame Discord rejects is not retried; the next update is built from the newest state. A poisoned lock is recovered. Nothing here returns an error to the UI or the play hub.

## Testing Strategy

- Pure: the builder for every state (table test) and the private form; `fit` with multi-byte characters at the cut point and under 2 bytes; the rate limiter with a fake clock (first at once, newest wins, a seek, `force`, `reset`); the sleeping rule with a fake clock; the cover rules (host list, https, 256 bytes); the engine (off does nothing, dedupe, reconnect after loss, clear on off).
- I/O: a fake Discord server on a Unix socket in a temp folder (handshake, `SET_ACTIVITY`, clear, `PING`, a server that dies and comes back); the worker end to end with short times; a test that the tracing output holds no title, artist or cover address.
- Not tested here: a real Discord client, the Windows named pipe, Flatpak and Snap socket paths.

## Risks and Alternatives

- Discord may rate limit or change local IPC; the Social SDK is its recommended path but needs a network login, which this app does not do (out of scope).
- The Windows named pipe is opened with `std::fs::OpenOptions` and cannot be unit tested on Linux; the pipe path code is a few lines and is listed as not verified.
- The small art assets are only text keys; with no upload Discord shows no small picture and the tooltips still work.
- Alternative: `discord-rich-presence` =1.1.0 (MIT). Kept as the fallback if the own client proves flaky.
