## Overview

A new crate `nemo-cast` finds Google Cast devices (mDNS) and UPnP/DLNA renderers (SSDP) while the picker is open, loads the current song on the chosen device, and reports the device's own position and state. The device decodes the stream, so the path is never bit-perfect; the UI shows "Casting", the source codec and bitrate, and "Decoded on the device" (criteria 7 to 9).

First task, before any other code: a spike on a real Chromecast and a real DLNA speaker (criterion 30). It loads (a) a `googlevideo.com` address from `yt-dlp -f bestaudio` (WebM/Opus) and from an m4a (AAC) format, fetched by the device itself, and (b) the same stream passed through a small HTTP relay on this computer, with and without CORS headers. It records which combinations play, and writes the result into this file. The result sets `StreamRoute::default_for_youtube`. The relay is built in either case, because local files need the same server and `coc.md` allows passing the playing stream through this computer.

Two shared pieces are defined here and used by `nemo-together` too: `RemotePlayback` with `RemoteSlot` (one active remote at a time) and a "parked" player. A parked player keeps its queue and moves through it, but decodes nothing and holds no output device. This is how the local output stays idle (criterion 13) while queue navigation still works.

## Affected Crates

- `nemo-cast` (new): discovery, Cast V2 client, UPnP client, LAN stream server.
- `nemo-core`: new `remote.rs` (`RemotePlayback`, `RemoteSlot`, `RemoteStatus`, `CommandGate`), `cast.rs` (`CastControl`). New `PlayerCommand::Park` and `PlayerCommand::Unpark { at }`, new `PlayerEvent::ParkedChanged(bool)`.
- `nemo-audio`: `control_cmd.rs` handles `Park` and `Unpark`; `control.rs::start_track` skips `spawn_prepare` and the pipeline while parked and still emits `TrackChanged`; `stop_all` and the device release run on `Park`. No change to the render callback.
- `nemo-ui`: picker (keyboard, list, busy flag), "Casting to <name>" on the player bar, remote position and volume, firewall hint, reduced-motion text state. Uses `CastControl` only.
- `nemo-app`: builds `nemo-cast`, applies `RemotePlayback::gate` to every UI command, refuses saved songs (`DownloadService::state(&VideoId) == SaveState::Saved`, offline-cache design), and registers a `SignOutWipe` step named "cast" that stops casting and ends the server.
- `nemo-ytm`: no change. `nemo-app` calls `StreamResolver::resolve_with` with `StreamRequest { prefer_codec: Some(Codec::Aac), .. }` for a DLNA device (account-actions design, ADR 0004 row 13). `nemo-library`, `nemo-lyrics`: no change.

## Data Flow

- Picker opens: `CastControl::open_picker` starts a `mdns-sd` daemon browsing `_googlecast._tcp.local.` (TXT `fn` name, `md` model, `st` busy flag) on every up IPv4 interface, and a tokio task sending SSDP `M-SEARCH` for `urn:schemas-upnp-org:device:MediaRenderer:1` every 5 s. Picker closes: the daemon is shut down and the socket dropped (criteria 3, 4). Nothing listens otherwise. The device description is fetched only when its host equals the responding IP, over the LAN, with a 3 s timeout and a 256 KiB limit, and parsed with `quick-xml`.
- Cast session: TLS to `ip:8009` through `tokio-native-tls`. Devices use self-signed certificates, so the chain is not checked; instead the first certificate seen from each device is pinned (its SHA-256 fingerprint, stored with the device) and a changed certificate stops the connection with a warning the user must confirm (owner, 2026-11-14). Frames are a 4-byte big-endian length and a `CastMessage` (prost). Sender `sender-0` sends `CONNECT`, answers `PING` and pings every 5 s, sends `LAUNCH` for the Default Media Receiver (`CC1AD845`), then `LOAD` on the media namespace. `MEDIA_STATUS` gives position, state and idle reason; `RECEIVER_STATUS` gives volume. A busy device (another app or session) needs an explicit take-over from the user (criterion 21).
- UPnP session: SOAP over `reqwest`: `GetProtocolInfo`, `SetAVTransportURI` with DIDL-Lite metadata, `Play`, `Pause`, `Seek` (REL_TIME), `Stop`, `SetVolume`. State is polled once a second with `GetPositionInfo` and `GetTransportInfo`.
- Casting starts: nemo-app claims `RemoteSlot` (fails while a room is live), sends `Park`, and `nemo-cast` loads the current track at the current position. `RemotePlayback::gate` returns `Forward` for Pause, Resume, Seek, SetVolume and Stop (handled by the device session) and `Allow` for Next, Previous and queue edits (the parked player moves the index and emits `TrackChanged`, which `nemo-cast` turns into a new `LOAD`). When the device reports `IDLE/FINISHED`, `nemo-cast` emits `TrackEnded` and nemo-app sends `Next`; at the queue end it stops (criterion 11).
- Stream route per track: a local file is served by the LAN server. A YouTube stream goes `Direct` (the resolved URL, as a `Secret<String>`, handed to the device) or `Relay` (the server fetches it with `reqwest` and streams the bytes to the device with no disk copy). If a `Direct` load fails on the device, `nemo-cast` retries that track once as `Relay`. Songs saved by the offline cache are refused (criterion 16; `DownloadService::state`).
- LAN server: `hyper` 1.12.0 on one IPv4 address, the interface in the same subnet as the device (via `if-addrs`), port chosen by the OS. Never `0.0.0.0`. The path is `/s/<32 hex from the OS random source>/<name>.<ext>`; any other path is 404, a peer IP other than the device's is 403. It answers `GET`, `HEAD` and `OPTIONS`, supports `Range` (local files: file ranges; relay: the `Range` header is forwarded upstream), and serves only the current session's one item. CORS headers on every answer: `Access-Control-Allow-Origin: *`, `Allow-Headers: Range, Content-Type`, `Expose-Headers: Content-Length, Content-Range, Accept-Ranges` (criterion 26). `*` is acceptable because the path is unguessable and the peer is pinned. The server ends on stop, error, sign-out or exit.
- Windows firewall: the first bind of a non-loopback listener makes Windows ask. The turn-on text and the picker say so before it happens (criterion 2), and say "choose Private networks". If the device never requests the file within 8 s of `LOAD`, `nemo-cast` emits `FirewallHint` and the UI shows how to allow it (criterion 6). The README gives the one-line `netsh` rule.
- Handover back (criterion 20): `nemo-cast` reads the device position, nemo-app sends `Unpark { at }`, and the local player resumes there.
- Threads: audio thread untouched. All network work runs on the crate's own tokio runtime.

## Interfaces

```rust
// nemo-core/src/remote.rs
pub enum RemoteKind { Cast, Room }
pub enum RoomRole { Host, Guest }
pub enum RemoteMode { Local, Cast { device: String }, Room { role: RoomRole } }
pub struct RemoteStatus {
    pub mode: RemoteMode, pub state: PlayState, pub position: Option<Duration>,
    pub length: Option<Duration>, pub volume: Option<u8>, pub source: Option<StreamInfo>,
}
pub enum CommandGate { Allow, Forward, Deny { reason: &'static str } }
pub struct RemoteBusy(pub RemoteKind);
#[derive(Clone)] pub struct RemoteSlot(/* Arc<Mutex<Option<RemoteKind>>> */);
pub struct RemoteLease(/* releases the slot on drop */);
impl RemoteSlot {
    pub fn new() -> Self;
    pub fn claim(&self, kind: RemoteKind) -> Result<RemoteLease, RemoteBusy>;
    pub fn active(&self) -> Option<RemoteKind>;
}
pub trait RemotePlayback: Send + Sync {
    fn remote_status(&self) -> RemoteStatus;
    fn gate(&self, command: &PlayerCommand) -> CommandGate;
    fn handle(&self, command: PlayerCommand); // for CommandGate::Forward
}

// nemo-core/src/cast.rs
/// Named `CastDeviceId` so it does not clash with the output-devices `DeviceId` (ADR 0004, row 5).
pub struct CastDeviceId(pub String);
pub enum DeviceKind { ThisComputer, Cast, Dlna }
pub struct CastDevice { pub id: CastDeviceId, pub name: String, pub kind: DeviceKind, pub busy: bool }
pub enum Discovery { Closed, Searching, Finished }
pub enum CastEvent { DevicesChanged, Connected, FirewallHint, FormatRejected { format: String }, TrackEnded, Failed { reason: String }, Disconnected }
pub trait CastEventReceiver: Send { fn try_next(&mut self) -> Option<CastEvent>; }
pub trait CastControl: RemotePlayback {
    fn open_picker(&self);
    fn close_picker(&self);
    fn discovery(&self) -> Discovery;
    fn devices(&self) -> Vec<CastDevice>;
    fn cast_to<'a>(&'a self, device: &'a CastDeviceId, take_over: bool, track: &'a Track, at: Duration) -> BoxFuture<'a, Result<(), CastError>>;
    fn stop_casting(&self) -> BoxFuture<'_, Option<Duration>>; // last device position
    fn subscribe_cast(&self) -> Box<dyn CastEventReceiver>;
}

// nemo-core/src/player.rs (additions)
// PlayerCommand::Park, PlayerCommand::Unpark { at: Duration }, PlayerEvent::ParkedChanged(bool)

// nemo-cast
pub enum StreamRoute { Direct, Relay }
pub struct CastService { /* discovery, session, server */ }
impl CastService {
    pub fn new(player: Arc<dyn PlayerControl>, resolver: Arc<dyn StreamResolver>, slot: RemoteSlot) -> Result<Self, CastError>;
}
```

## Error Handling

`CastError` (`thiserror`): `NoDevices`, `Busy { by_other_sender }`, `Unreachable`, `TlsFailed`, `FormatRejected { format }` (names the codec or container, no transcoding, criterion 17), `CachedSong`, `NotAvailable`, `ServerBind`, `DeviceError { code }`, `Remote(RemoteBusy)`. Any failure stops casting, ends the server, shows the reason and offers "Continue here" at the last known position (criterion 20). Stream URLs and tokens never appear in errors or logs. No cookies or account data are sent to a device (criterion 19).

## Testing Strategy

- Cast: an in-process TLS fake device answers `CONNECT`, `PING`, `LAUNCH`, `LOAD`, `GET_STATUS`; tests cover busy takeover, `IDLE/FINISHED` advancing the queue, a dropped socket, and a `LOAD_FAILED` that falls back from `Direct` to `Relay`.
- UPnP: a fake SSDP responder and SOAP server; tests cover the 256 KiB limit, a LOCATION host that differs from the sender (ignored), `GetProtocolInfo` rejection, and polling.
- Server: unguessable path, wrong peer is 403, `Range`, `HEAD`, `OPTIONS` and CORS headers, one item only, no listing, relay streams with no file created in the temp or data folder, server closed on stop and on drop, bind address is never unspecified.
- Quiet: with the picker closed, no socket is open (assert with a socket counter in the test harness).
- Park: `nemo-audio` tests check that `Park` releases the device, a parked `Next` emits `TrackChanged` with no decode, and `Unpark { at }` resumes at `at`. `cargo test -p nemo-audio --test bit_exact` stays green with Park never used. A counting-allocator test proves the render callback is unchanged by the new code paths.
- Gate: the `RemotePlayback` table of allow, forward and deny per command, for Cast and for rooms.
- Manual, by the owner: Chromecast, a DLNA speaker, firewall prompt on Windows 11 for Private and Public profiles, picker with VPN on.

## Risks and Alternatives

- `googlevideo.com` addresses are tied to the requesting IP and headers; the device may get 403. The spike decides, and `Relay` is the fallback. A relay doubles the bandwidth on this computer and keeps the app running while casting; `coc.md` allows it and forbids storing the stream.
- Windows firewall: a denied prompt or a Public profile blocks the device from reaching the server. Discovery can also fail with VPNs, several adapters or Wi-Fi client isolation. Messages name these causes (criterion 6).
- The device's certificate chain is not checked (self-signed). Pinning the first-seen certificate stops a later impostor, but not one present at the very first connection; it then sees only the session URL, which is unguessable and pinned to the device IP. Chrome itself checks the device chain; we do not, and the settings text says so.
- DLNA renderers differ (DIDL-Lite metadata, supported containers, position format). The spike covers one speaker; more are added as users report them.
- Crates rejected: `rust-cast` 0.21.0 (MIT, 2025-12-30) is synchronous, pins `protobuf` =3.7.2 as a second protobuf stack next to `prost`, and its certificate handling is unverified. `rupnp` 3.0.0 and `ssdp-client` 2.1.0 (both 2025-04-25, MIT OR Apache-2.0) pass the two-year rule of `code.md` (ADR 0004, Decision 5), so age does not reject them. They are still not used: `rupnp` brings `roxmltree`, `genawaiter` and `if-addrs` 0.13 (a second copy next to the 0.15 above) and its own hyper client beside `reqwest`, to replace one `M-SEARCH` and seven SOAP calls (about 300 lines on `quick-xml`, already locked).
- Alternative rejected: transcoding on this computer to fit a device. Criterion 17 forbids it unless the user turns it on, which is not planned.

## Steering and README changes

- Opt-in: the Cast switch uses the shared `OptInRow` and `OptInInfo` (ADR 0004, "Opt-in settings pattern"). Sends: the playing stream to a device on the local network, through a LAN-only server on this computer. Hosts: none on the internet. Risk: the stream is served over plain HTTP on the local network, to the chosen device only; each device's certificate is pinned on first use rather than checked against a chain (see Risks); Windows asks for a firewall rule.
- `tech.md` rows (add): `mdns-sd` =0.21.7 (Apache-2.0 OR MIT, 2026-11-02, MSRV 1.71, pure Rust; brings `flume`, `mio`, `socket-pktinfo`, `fastrand`); `if-addrs` =0.15.0 (MIT OR BSD-3-Clause, 2026-02-08); `prost` =0.14.6 (Apache-2.0, 2026-11-12, MSRV 1.85) with build-dependencies `prost-build` =0.14.6 and `protox` =0.10.0 (MIT OR Apache-2.0, 2026-10-07), shared with `nemo-together`; `getrandom` 0.3.4 (for the 32-hex session path; locked), `hyper` 1.12.0, `hyper-util` 0.1.21, `http-body-util` 0.1.5, `quick-xml` 0.41.0, `tokio-native-tls` 0.3.1, `reqwest` 0.13.5 as already locked (hyper gets its `server` and `http1` features). None needs a C toolchain on Windows. The vendored `cast_channel.proto` is Apache-2.0 (Chromium) and is recorded with its source.
- `structure.md`: add `nemo-cast` (depends on `nemo-core` only) and the edge `nemo-app -> nemo-cast`; add `remote.rs` and `cast.rs` to the `nemo-core` ownership line; note that `nemo-audio` gets `Park`.
- README: no internet host. A "Cast and local network" section lists mDNS (UDP 5353, `224.0.0.251`), SSDP (UDP 1900, `239.255.255.250`), TCP 8009 to the device, SOAP to the device's HTTP port, and the LAN stream server on an OS-chosen port. It gives the firewall rule: `netsh advfirewall firewall add rule name="NemoMusic Cast" dir=in action=allow program="<path>\nemo-app.exe" profile=private enable=yes`.
- ADR 0004: Decision 2 lists `RemotePlayback`, `RemoteSlot` and `CastControl`; Decision 1 gives `nemo-audio` the parked mode.

<!-- Sources:
Cast supported media and CORS: https://developers.google.com/cast/docs/media
Cast Web Receiver and Default Media Receiver: https://developers.google.com/cast/docs/web_receiver/core_features
UPnP AVTransport flow: https://github.com/sonydevworld/audio_control_api_examples/blob/master/DLNA/AVTransport/play_file.adoc
yt-dlp FAQ (same IP and headers): https://github.com/yt-dlp/yt-dlp/wiki/FAQ
crates.io API for mdns-sd 0.21.7, if-addrs 0.15.0, rust-cast 0.21.0 (and its dependencies), rupnp 3.0.0, ssdp-client 2.1.0, prost 0.14.6, protox 0.10.0 (fetched 2026-11-14)
Repo: crates/nemo-audio/src/control.rs and control_cmd.rs (start_track, command), crates/nemo-core/src/player.rs and stream.rs, .kiro/steering/coc.md, code.md, structure.md
Owner answers 2026-11-14 (task text)
-->
