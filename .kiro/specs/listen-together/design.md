## Overview

A new crate `nemo-together` is a Listen Together client written from the published `listentogether.proto` (metroproto, GPL-3.0). It speaks the same protocol as Metrolist, over a WebSocket to a server address the user types. NemoMusic ships with no default server (criterion 3). The crate never copies Metrolist or metroserver code; it reads the `.proto` and the server README only.

The crate drives the local player through `nemo-core` traits. As host it watches `PlayerEvent`s and sends `playback_action` messages. As guest it turns room state into `PlayerCommand`s and keeps the position in line by seeking. A shared `RemotePlayback` trait (defined in the Cast design, `nemo-core/src/remote.rs`) tells the UI and `nemo-app` that a room is live, and which commands a guest may send. Rooms and Cast exclude each other through its `RemoteSlot` (criterion 30).

Launch feature set, sent in `client_capabilities`: `request_id`, `origin_tag`, `drift_report`, `desync_canary`, `server_restarting`, `replace_queue`, `host_sequence`, `track_revision`, `update_track`. Not requested: `suggestion_ack`, because guest suggestions are Later. The client ignores unknown messages and features (criterion 31). Positions and durations are milliseconds; the first task confirms this against a self-hosted metroserver (unverified).

## Affected Crates

- `nemo-together` (new): socket, codec, room state machine, clock sync, drift loop, blocklist database.
- `nemo-core`: new `together.rs` with `RoomControl` and its types; `remote.rs` (Cast design) for `RemotePlayback` and `RemoteSlot`.
- `nemo-ui`: settings entry (shared opt-in pattern, server address field, risk text), create and join dialogs, member list, host approvals, player-bar dot (criterion 27). Uses `RoomControl` only.
- `nemo-app`: builds `nemo-together` with `Arc<dyn PlayerControl>`, applies `RemotePlayback::gate` to UI commands, keeps the user's saved `TransitionSettings` in memory, sends `PlayerCommand::SetTransition` with `mode: TransitionMode::Off` and `skip_fade: SkipFade::Off` when a room starts and the saved value when it ends (audio-effects design; the stored setting is not changed), and registers a `SignOutWipe` step named "together" that leaves the room and drops the session token.
- `nemo-audio`, `nemo-ytm`: no change. A guest resolves its own stream through the existing `StreamResolver` (criterion 17).

## Data Flow

- One tokio task owns the socket (`tokio-tungstenite`, binary frames). Commands from the UI arrive on an `mpsc` channel; room events leave on a `broadcast`-style queue read by `RoomEventReceiver::try_next` (polled per frame, like `EventReceiver`).
- Frame: protobuf `Envelope { type, payload, compressed, request_id }`. Payloads over 100 bytes are gzip-compressed when the server's `supports_compression` is true. Inbound limits: 1 MiB per frame, 4 MiB after decompression, track ids at most 200 bytes, queue at most 500 tracks; anything larger is dropped with `RoomError::BadMessage`.
- Clock: pings carry `client_time`; pongs give `server_receive_time` and `server_send_time`. The crate keeps the best of the last 8 round trips to map server time to local time. A room position is `position + (server_now - last_update)` while playing.
- Guest: `change_track` becomes `Play(track)` followed by `Pause`; when the local player reports the track ready, the client sends `buffer_ready` with the `track_revision`; on `buffer_complete` or `sync_playback{play}` it issues `Seek` and `Resume`. A join starts the current track at the room's current position (criterion 11).
- Drift (criterion 13, 14): pings go out at 250 ms three times on connect, then every 5 s while playing. `pong.drift_ms` (feature `drift_report`) is compared with 500 ms. Above it, the guest sends one `PlayerCommand::Seek` to the room position. The loop ignores drift for 3 s after a seek or track change, and corrects at most once per 10 s. It never changes speed or resamples. A local pause leaves the room running; `Resume` reads the room position and seeks first (criterion 12).
- Host: `TrackChanged`, `StateChanged`, seeks and queue changes become `playback_action` messages with a rising `host_sequence`. A local or SoundCloud track is not sent; the host sees "the room cannot play this" (criterion 18). Only `id` (the video id), title, artist, album, duration and the thumbnail URL leave the machine (criterion 17).
- Reconnect (criterion 24): up to 15 attempts, 1 s doubling to 120 s, with jitter, using `reconnect{session_token}`. The token is a `Secret<String>` in memory only, dropped on leave. Playback continues locally meanwhile. `server_restarting` schedules one attempt after `eta_ms`.
- Threads: the audio thread is untouched. The crate sends commands with `PlayerControl::send`, which never blocks.

## Interfaces

```rust
// nemo-core/src/together.rs  (RoomRole comes from remote.rs)
pub enum LinkState { Off, Connecting, Connected, Reconnecting { attempt: u8, of: u8 }, Failed }
pub struct MemberId(pub String);
pub struct Member { pub id: MemberId, pub name: String, pub is_host: bool, pub connected: bool }
pub struct RoomStatus {
    pub link: LinkState, pub code: Option<String>, pub role: Option<RoomRole>,
    pub members: Vec<Member>, pub pending: Vec<Member>, pub server: Option<String>, pub auto_approve: bool,
}
pub enum RoomEvent {
    StatusChanged, JoinRequested(Member), JoinRejected { reason: String }, Kicked { reason: String },
    HostChanged { new_host: Member }, CannotPlay { title: String }, ServerMessage(String), Left,
}
pub trait RoomEventReceiver: Send { fn try_next(&mut self) -> Option<RoomEvent>; }

pub trait RoomControl: RemotePlayback {
    fn status(&self) -> RoomStatus;
    fn subscribe_room(&self) -> Box<dyn RoomEventReceiver>;
    fn create<'a>(&'a self, server: &'a str, display_name: &'a str) -> BoxFuture<'a, Result<(), RoomError>>;
    fn join<'a>(&'a self, server: &'a str, code: &'a str, display_name: &'a str) -> BoxFuture<'a, Result<(), RoomError>>;
    fn approve(&self, member: &MemberId);
    fn reject(&self, member: &MemberId, reason: &str);
    fn kick(&self, member: &MemberId, reason: &str);
    fn set_auto_approve(&self, on: bool);
    fn block_name(&self, name: &str);
    fn leave(&self) -> BoxFuture<'_, ()>;
}

// nemo-together
pub struct TogetherClient { /* socket task, clock, blocklist */ }
impl TogetherClient {
    pub fn new(data_dir: &Path, player: Arc<dyn PlayerControl>, slot: RemoteSlot, user_agent: String) -> Result<Self, RoomError>;
}
pub struct ServerAddress(/* validated */);
impl ServerAddress { pub fn parse(text: &str) -> Result<Self, RoomError>; }
```

`ServerAddress::parse` accepts `wss://host[:port]/path`, and `ws://` only for loopback, private-range and `.local` hosts (criterion 3).

## Error Handling

`RoomError` (`thiserror`): `BadAddress`, `Unreachable { host }`, `Refused { message }` (server text shown as received, criterion 26), `RoomNotFound`, `RoomFull`, `Kicked`, `NotHost`, `BadMessage`, `Busy(RemoteKind)` (Cast active), `Storage`. Server `error{code,message}` maps to `Refused` unless it is a known code. A failed first connect shows the host and does not retry (criterion 25). Reconnect gives up after 15 attempts and leaves the room with a message. No display name, room code or session token goes to logs.

## Testing Strategy

- Codec: round-trip every message type from the `.proto`, with and without gzip; reject oversize frames and zip bombs; ignore unknown types.
- A fake metroserver (in-process WebSocket) runs scripted rooms: create, join with approve and reject, host handover, kick, server restart, forced disconnect and reconnect, wrong code, full room.
- Drift: a simulated player clock drifts by ±2 s; the test asserts convergence below 500 ms, no seek within 3 s of a seek, at most one seek per 10 s, and never a speed change.
- Privacy: a test asserts the only fields sent are those in criterion 17, and a local file or SoundCloud track is never sent.
- Gate: a guest's `Seek`, `Next` and `Play` are denied; host commands pass (criterion 9).
- Exclusion: a room cannot start while Cast holds `RemoteSlot`, and the reverse.
- Audio is not touched; `cargo test -p nemo-audio --test bit_exact` stays green. No real-time path is added, so no counting-allocator test.
- `#[ignore]` interop test against a self-hosted metroserver in Docker, run by hand, to check units, id format and the User-Agent.

## Risks and Alternatives

- The protocol is open but Metrolist's KMP rewrite is private and the protocol grows. Unknown features are ignored; the pinned `.proto` commit is recorded in the crate.
- A server may block unknown clients (User-Agent policy `block`). We show the server's text and do not rename the client (criterion 26). The owner asks the operator of The Meowery before it is listed.
- Track ids are opaque to the server; Metrolist uses the YouTube video id. A foreign id that fails `VideoId::parse` shows "cannot play". Unverified until the interop test runs.
- Drift of 500 ms is a starting value. Seeking on a compressed stream can click; the owner listens on two machines before the value is fixed.
- A self-hosted address on a LAN uses `ws://` without TLS. The risk text says room data and the IP are visible on that network.
- Alternative rejected: copy the Go server types or Metrolist's Kotlin client. Not needed and mixes in code we do not need to carry.
- Alternative rejected: speed correction (Metrolist does 2%). Criterion 14 forbids it.
- Merging into `nemo-ytm`: rejected, ADR 0004 gives rooms their own crate.

## Steering and README changes

- Opt-in: the Listen Together switch uses the shared `OptInRow` and `OptInInfo` (ADR 0004, "Opt-in settings pattern"), with the server address field. Hosts: "the server you type"; a different server asks again. Risk text includes the `ws://` LAN warning.
- `tech.md` rows (add): `tokio-tungstenite` =0.30.0 (MIT, 2026-07-11, MSRV 1.85, feature `native-tls`, expected to reuse `tokio-native-tls` 0.3.1 from `Cargo.lock`; confirm the feature name); `prost` =0.14.6 (Apache-2.0, 2026-11-12, MSRV 1.85); build-dependencies `prost-build` =0.14.6 (Apache-2.0) and `protox` =0.10.0 (MIT OR Apache-2.0, 2026-10-07, MSRV 1.85), so no `protoc` is needed on Windows; `flate2` 1.1.11, `crc32fast` 1.5.5 and `rusqlite` 0.41.1 as already locked. The vendored `listentogether.proto` is GPL-3.0 and sits in `crates/nemo-together/proto/` with its source commit.
- `structure.md`: add `nemo-together` (depends on `nemo-core` only) to layout, ownership and the graph.
- README egress list: no default host. Text: the user types the room server; the app sends display name, room code, track ids, titles and play state, and the IP address shows to the server. A host is named only when it becomes a listed choice.
- ADR 0004: add `RoomControl` to the Decision 2 list; note that `nemo-together` takes `Arc<dyn PlayerControl>` from `nemo-app`.

<!-- Sources:
listentogether.proto: https://raw.githubusercontent.com/MetrolistGroup/metroproto/master/listentogether.proto
metroserver README and internal/server/protocol.go (message types, features, UA policy, host handover): https://github.com/MetrolistGroup/metroserver
Metrolist client constants (15 reconnect attempts, 1 s to 120 s delays, 25 s ping, 750 ms hard sync, gzip over 100 bytes, User-Agent): https://github.com/MetrolistGroup/Metrolist/tree/main/app/src/main/kotlin/com/metrolist/music/listentogether
crates.io API for tokio-tungstenite 0.30.0, prost 0.14.6, prost-build 0.14.6, protox 0.10.0 (fetched 2026-11-14)
Repo: crates/nemo-core/src/player.rs and services.rs (PlayerCommand, PlayerEvent, PlayerControl), .kiro/steering/structure.md, coc.md
Owner answers 2026-11-14 (task text)
-->
