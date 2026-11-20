## Overview

Presence is a pure pipeline with a thin I/O shell. Play events and window facts become a `PresenceState`; `build` turns it into an `Activity` (all texts, images and the private form); a rate limiter, a sleeping rule and a dedupe decide when to send; one worker thread owns the connection to the local Discord app and does only I/O. The pure parts hold all the rules and have fake clocks, so they are tested without a Discord client.

The IPC client is written in this crate (safe code, `std` only) instead of using `discord-rich-presence` =1.1.0. Reasons: the crate pulls a second `uuid` (0.8) and `cargo deny` is not installed on the build machine, so it could not be checked here; the owner's rule is to use the crate only if it passes; the protocol is small (a handshake, `SET_ACTIVITY`, close) and the own client lets the tests run against a fake server with timeouts. No new crate, so `tech.md` is unchanged.

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
- `session.rs`: `Session` folds `PlayHookEvent`s (`TrackStarted`, `StateChanged`, `PositionJumped`, `DurationKnown`) and the facts of the app (section, sleep timer, place) into a `PresenceState`. Start time = wall time now minus position, computed when the event arrives, so a held update still shows the right progress when it is sent. `Buffering` keeps the shown state. A cover comes from `pick_thumbnail(.., 512)` of a remote artwork; a local file has none.
- `engine.rs`: `Engine::step(now, wall, Input) -> Vec<Action>` joins session, away rule, limiter, dedupe and the connect schedule. Inputs: `Play`, `Window`, `Section`, `SleepTimer`, `Place`, `Private`, `Switch`, `Connected(bool)`, `Tick`. Actions: `Connect`, `Set(Activity)`, `Clear`, `Disconnect`. No clock, no I/O.

## Affected Crates

- `nemo-integrations`: the module `discord` (all of the above, plus `ipc.rs` the frame codec and the socket or pipe, and `worker.rs` the thread); `service.rs` (switch, private switch, observer fan-out, store); `info.rs` (notice text).
- `nemo-core`: additive default methods on `IntegrationsService` (`discord_private`, `set_discord_private`, `window_context`) and the types `WindowContext` and `BrowsingSection`. Implementations that do not know them keep working (default bodies). This is the only change to the public API of `nemo-core`.
- `nemo-app`: `Startup::run` builds the real `Integrations` with the integrations folder (replacing `NoIntegrations` in the `Backend`) and registers it as a play observer; the service stops its worker when it is dropped at exit (hooks in `backend.rs`, one dependency line).
- `nemo-ui`: the Discord row and the `Hide song names` switch in Settings, Privacy (`views/settings/discord.rs`), the messages that flip them, and one call that tells the service about the window (focus, section, sleep timer) next to the existing `Message::Focus` handling.

## Data Flow

```
 PlayHub observer thread --on_event--> Mailbox (bounded 64, drop oldest) --> discord worker thread
 UI thread -- window_context / set_discord / set_discord_private --> Mailbox --^
                                                                          |
        worker: Engine::step(now, wall, input) -> Actions -> ipc (socket or pipe) -> local Discord app
```

- The mailbox is a mutex-guarded `VecDeque` with a condition variable; a producer never waits for I/O. When it holds 64 entries, the oldest play event is dropped; a switch, a window fact or the exit message is never dropped. Nothing runs on the audio thread. The thread exists only while the switch is on (started by `set_discord(true)` or at app start when the stored switch is on and its notice confirmed; ended by `set_discord(false)` or the drop of the service, both clear the presence first, the exit waits at most 500 ms).
- The worker waits for the next message with a timeout of `min(limiter.wait, away.wake_in, reconnect wait)`, or for ever when none is due. A song that plays steadily wakes it for nothing.
- Connect: Unix socket `discord-ipc-0..9` in `$XDG_RUNTIME_DIR`, `$XDG_RUNTIME_DIR/app/com.discordapp.Discord` and `.DiscordCanary` (Flatpak), `$XDG_RUNTIME_DIR/snap.discord` (Snap), `$TMPDIR`, `$TMP`, `$TEMP`, `/tmp`; Windows named pipe `\\.\pipe\discord-ipc-0..9`. Handshake frame (opcode 0, `{"v":1,"client_id":"<id>"}`), read until `READY`, then `SET_ACTIVITY` frames (opcode 1). The client asks and Discord answers, one frame at a time on the worker thread; there is no reader thread, because a synchronous Windows pipe handle cannot read and write at once. A Unix socket has a 5 s read and write timeout. A `PING` frame is answered with `PONG`; a `CLOSE` frame or a read or write error ends the connection (`Connected(false)`, retry in 30 s); a frame longer than 64 KiB is an error. A lost connection is noticed at the next update; while nothing changes, nothing is sent and nothing is read.
- A play event while the switch turns on in the middle of a song is not known to the worker: the presence shows the song from the next track or state change. (The play hub sends events only; it has no snapshot to hand over.)

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
pub struct Worker; // Worker::start(Config), send(Input), stop(wait)
pub trait Connector { fn open(&self, client_id: &str) -> io::Result<Box<dyn Link>>; }  // IpcConnector, or a fake in tests
pub trait Link { fn set_activity(&mut self, a: Option<&Activity>) -> io::Result<Incoming>; fn close(self: Box<Self>); }

// nemo_core::integrations (additive, default bodies)
pub struct WindowContext { pub focused: bool, pub section: Option<BrowsingSection>, pub sleep_timer: bool }
pub trait IntegrationsService { fn discord_private(&self) -> bool { false }
    fn set_discord_private(&self, on: bool) {}  fn window_context(&self, ctx: WindowContext) {} }
```

Portal assets the owner uploads (Discord developer portal, Rich Presence, Art Assets, names exact): `nemo` (the app icon, 1024 px), `youtube-music`, `soundcloud`, `local`, `paused` (small icons, 512 px).

## Error Handling

Connect, write and read errors are silent: the worker marks the connection lost, sets the retry time (30 s) and logs one debug line per change of the connected state. A frame Discord rejects is not retried; the next update is built from the newest state. A poisoned lock is recovered. A frame Discord answers with an error is logged as `status=err` without its text. Nothing here returns an error to the UI or the play hub.

## Testing Strategy

- Pure: the builder for every state (table test) and the private form; `fit` with multi-byte characters at the cut point and under 2 bytes; the rate limiter with a fake clock (first at once, newest wins, a seek, `force`, `reset`); the sleeping rule with a fake clock; the cover rules (host list, https, 256 bytes); the engine (off does nothing, dedupe, reconnect after loss, clear on off).
- I/O: a fake Discord server on a Unix socket in a temp folder (handshake, `SET_ACTIVITY`, clear, `PING`, a server that dies and comes back); the worker end to end with short times; a test that the tracing output holds no title, artist or cover address.
- Not tested here: a real Discord client, the Windows named pipe, Flatpak and Snap socket paths.

## Risks and Alternatives

- Discord may rate limit or change local IPC; the Social SDK is its recommended path but needs a network login, which this app does not do (out of scope).
- The Windows named pipe is opened with `std::fs::OpenOptions` and cannot be unit tested on Linux; the pipe path code is a few lines and is listed as not verified.
- The small art assets are only text keys; with no upload Discord shows no small picture and the tooltips still work.
- Alternative: `discord-rich-presence` =1.1.0 (MIT). Kept as the fallback if the own client proves flaky.
