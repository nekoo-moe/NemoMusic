## Tasks

Cross-spec ids written `<spec>#?` point at the task in that spec that adds the named ADR 0004 Decision 4 row; fill in the number when that spec's `tasks.md` lands.

- [ ] 1. Spike: cast a googlevideo stream to a real Chromecast and a real DLNA speaker
  - Crate and files: none merged. A throwaway probe outside the repo; the result goes into `.kiro/specs/cast/design.md` (Risks and Alternatives).
  - Does: Loads (a) a `googlevideo.com` address from `yt-dlp -f bestaudio` (WebM/Opus) and from an m4a (AAC) format, fetched by the device itself, and (b) the same streams through a small HTTP relay on this computer, with and without the CORS headers of the design. Records per device which container, codec, headers and route play, and whether `Range` is used.
  - Pass: the device plays the address it fetches itself, so `StreamRoute::default_for_youtube` is `Direct` with `Relay` as the per-track fallback. Fail (403 or no sound): the default is `Relay`. Either result keeps the relay (task 7), because local files need the same server. A format no device plays becomes a `FormatRejected` case in task 16.
  - Changes on failure: task 16 (default route), task 7 (relay becomes the main path, so its tests grow), task 12 and task 13 (`LOAD` and `SetAVTransportURI` content type).
  - _Requirements: 18, 30_
  - Depends on: none
  - Wave: 1
  - Checks: needs the owner: a Chromecast or Nest device, a DLNA speaker, and a throwaway Google account for `yt-dlp`. No cookies or tokens are written into the design.
  - Steering: none

- [ ] 2. Add the shared remote-playback types to nemo-core
  - Crate and files: `nemo-core`: new `src/remote.rs`, `src/lib.rs`. Touches nemo-core/src/remote.rs and nemo-core/src/lib.rs.
  - Does: Adds `RemoteKind`, `RoomRole`, `RemoteMode`, `RemoteStatus`, `CommandGate`, `RemoteBusy`, `RemoteSlot`, `RemoteLease` and the `RemotePlayback` trait, signatures as in the design (ADR 0004 row 41). `RemoteSlot::claim` fails while another kind holds the slot, and the lease frees it on drop.
  - _Requirements: 27_
  - Depends on: none
  - Wave: 0
  - Checks: unit tests: claim, claim while busy returns `RemoteBusy(kind)`, release on drop, `active()`. No I/O in `nemo-core`.
  - Steering: `structure.md` adds `remote.rs` and `cast.rs` to the `nemo-core` ownership line; ADR 0004 Decision 2 lists `RemotePlayback`, `RemoteSlot` and `CastControl`.

- [ ] 3. Add the Cast control trait and its types to nemo-core
  - Crate and files: `nemo-core`: new `src/cast.rs`, `src/lib.rs`. Touches nemo-core/src/cast.rs and nemo-core/src/lib.rs.
  - Does: Adds `CastDeviceId`, `DeviceKind`, `CastDevice`, `Discovery`, `CastEvent`, `CastEventReceiver`, the `CastControl` trait and `CastError` (`thiserror`, the variants of the design's Error Handling), because the trait returns it.
  - _Requirements: none (groundwork)_
  - Depends on: cast#2
  - Wave: 0
  - Checks: `CastError` `Display` holds no URL or token (unit test).
  - Steering: none

- [x] 4. Add the parked player mode (Park and Unpark)
  - Crate and files: `nemo-core`: `src/player.rs` (`PlayerCommand::Park`, `PlayerCommand::Unpark { at }`, `PlayerEvent::ParkedChanged(bool)`). `nemo-audio`: `src/control_cmd.rs`, `src/control.rs`. Touches nemo-core/src/player.rs.
  - Does: `Park` runs `stop_all` and releases the output device; while parked, `start_track` skips `spawn_prepare` and the pipeline and still emits `TrackChanged`, so Next, Previous and queue edits work with no decode. `Unpark { at }` reopens the device and resumes at `at`. The render callback does not change.
  - _Requirements: 13_
  - Depends on: none. Only one task at a time edits nemo-core/src/player.rs and nemo-audio/src/control_cmd.rs: order it with audio-effects#? (row 1), output-devices#? (row 5) and account-actions#? (row 6).
  - Wave: 0
  - Checks: `cargo test -p nemo-audio --test bit_exact` (Park never used). Counting-allocator test (`tests/rt_no_alloc.rs`) still shows zero allocations in the render callback. New tests: Park releases the device, a parked `Next` emits `TrackChanged` with no decode, `Unpark { at }` resumes at `at`.
  - Steering: `structure.md` notes the parked mode under `nemo-audio`; ADR 0004 Decision 1 gives `nemo-audio` the parked mode.

- [ ] 5. Create the nemo-cast crate skeleton
  - Crate and files: new `crates/nemo-cast/` (`Cargo.toml`, `src/lib.rs`, `src/service.rs` with an empty `CastService`), workspace member entry.
  - Does: Adds the crate with `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]`, `[lints] workspace = true`, and its own tokio runtime holder. Depends on `nemo-core` only.
  - _Requirements: none (groundwork)_
  - Depends on: cast#3
  - Wave: 0
  - Checks: `cargo tree -p nemo-cast --edges normal` shows no other `nemo-*` crate.
  - Steering: `structure.md` adds `nemo-cast` to the layout, the ownership list and the edge `nemo-app -> nemo-cast`.

- [ ] 6. Serve the cast file from a LAN-only HTTP server
  - Crate and files: `nemo-cast`: `src/server.rs`, `src/server/` (route, range, cors).
  - Does: `hyper` server on the one IPv4 address in the device's subnet (`if-addrs`), OS-chosen port, never `0.0.0.0` or any unspecified address. Serves one item at `/s/<32 hex from getrandom>/<name>.<ext>`: `GET`, `HEAD`, `OPTIONS`, file `Range`, the CORS headers of the design; any other path is 404 and any peer but the device is 403. No listing; ends on stop and on drop.
  - _Requirements: 14, 15, 16, 26_
  - Depends on: cast#5
  - Wave: 1
  - Checks: tests: unguessable path, wrong path 404, wrong peer 403, `Range` and `HEAD`, `OPTIONS` and CORS headers, one item only, server closed on stop and on drop, bind address never unspecified.
  - Steering: `tech.md` rows for `hyper` 1.12.0 (features `server`, `http1`), `hyper-util` 0.1.21, `http-body-util` 0.1.5, `if-addrs` =0.15.0, `getrandom` 0.3.4.

- [ ] 7. Relay a playing stream through the LAN server
  - Crate and files: `nemo-cast`: `src/server/relay.rs`.
  - Does: For a `Relay` item the server fetches the resolved URL (a `Secret<String>`) with `reqwest`, forwards the device's `Range` header upstream and streams the bytes to the device with no disk copy. It serves only the chosen device and ends with the session.
  - _Requirements: 18, 19_
  - Depends on: cast#6
  - Wave: 1
  - Checks: tests with a local upstream: bytes and ranges pass through, no file is created in the temp or data folder, no cookie or account header goes upstream or to the device, the URL never appears in logs or errors.
  - Steering: `tech.md` notes `reqwest` 0.13.5 (as locked) for `nemo-cast`.

- [ ] 8. Vendor cast_channel.proto and add the Cast V2 frame codec
  - Crate and files: `nemo-cast`: `proto/cast_channel.proto`, `build.rs`, `src/cast_v2/codec.rs`.
  - Does: Generates `CastMessage` with `prost-build` and `protox` (no `protoc`), and encodes and decodes frames as a 4-byte big-endian length plus a `CastMessage`, with a frame size limit. The vendored proto keeps its Apache-2.0 header and source commit.
  - _Requirements: none (groundwork)_
  - Depends on: cast#5
  - Wave: 1
  - Checks: round-trip and property tests (`proptest`) for the framing, oversize frame rejected.
  - Steering: `tech.md` rows for `prost` =0.14.6, build-dependencies `prost-build` =0.14.6 and `protox` =0.10.0 (shared with `nemo-together`, which reuses them). The `cast_channel.proto` licence notice (Apache-2.0, Chromium) joins the licence notices shipped with the app.

- [ ] 9. Store pinned device certificates
  - Crate and files: `nemo-cast`: `src/pin_store.rs`, migrations for the crate's SQLite file in the app data folder (ADR 0004 Decision 3).
  - Does: Stores the SHA-256 fingerprint of the first certificate seen per Cast device, and reports `Same`, `New` or `Changed` for a later one. The user's confirmation replaces a changed pin.
  - _Requirements: none (groundwork)_
  - Depends on: cast#5
  - Wave: 1
  - Checks: tests: first sight pins, same passes, changed is reported, confirmed change replaces, migration from empty.
  - Steering: `tech.md` notes `rusqlite` 0.41.1 (as locked) for `nemo-cast`.

- [ ] 10. Discover Cast devices over mDNS while the picker is open
  - Crate and files: `nemo-cast`: `src/discovery.rs`, `src/discovery/mdns.rs`.
  - Does: `open_picker` starts an `mdns-sd` daemon browsing `_googlecast._tcp.local.` on every up IPv4 interface and reads `fn`, `md` and `st` (busy); `close_picker` shuts it down and drops the socket. Nothing listens otherwise. "This computer" is always the first entry.
  - _Requirements: 3, 4, 5_
  - Depends on: cast#5
  - Wave: 2
  - Checks: socket-counter test: no socket open with the picker closed, none after it closes; TXT parsing tests.
  - Steering: `tech.md` row for `mdns-sd` =0.21.7 (and what it brings: `flume`, `mio`, `socket-pktinfo`, `fastrand`).

- [ ] 11. Discover DLNA renderers over SSDP while the picker is open
  - Crate and files: `nemo-cast`: `src/discovery/ssdp.rs`, `src/upnp/description.rs`.
  - Does: Sends `M-SEARCH` for `urn:schemas-upnp-org:device:MediaRenderer:1` every 5 s while the picker is open. Fetches a device description only when its host equals the responding IP, with a 3 s timeout and a 256 KiB limit, and parses it with `quick-xml`.
  - _Requirements: 3, 4, 5_
  - Depends on: cast#10
  - Wave: 2
  - Checks: fake SSDP responder: 256 KiB limit, a LOCATION host that differs from the sender is ignored, timeout, no socket with the picker closed.
  - Steering: `tech.md` row for `quick-xml` 0.41.0 (as locked).

- [ ] 12. Run a Cast V2 session with the Default Media Receiver
  - Crate and files: `nemo-cast`: `src/cast_v2/session.rs`, `src/cast_v2/tls.rs`.
  - Does: TLS to `ip:8009` with `tokio-native-tls`, chain not checked, certificate checked against the pin store; a changed certificate stops the connection with a warning event. Sends `CONNECT`, answers `PING` and pings every 5 s, `LAUNCH` `CC1AD845`, `LOAD`, play, pause, seek, volume and stop, and reads `MEDIA_STATUS` and `RECEIVER_STATUS`. A busy device needs `take_over`.
  - _Requirements: 10, 19, 20, 21_
  - Depends on: cast#8, cast#9, cast#1 (content types)
  - Wave: 2
  - Checks: in-process TLS fake device: `CONNECT`, `PING`, `LAUNCH`, `LOAD`, `GET_STATUS`, busy take-over, `IDLE/FINISHED`, dropped socket, `LOAD_FAILED`, changed certificate. A test asserts no cookie, token or account field is in `LOAD`.
  - Steering: `tech.md` row for `tokio-native-tls` 0.3.1 (as locked).

- [ ] 13. Run a UPnP AVTransport session
  - Crate and files: `nemo-cast`: `src/upnp/session.rs`, `src/upnp/didl.rs`.
  - Does: SOAP over `reqwest`: `GetProtocolInfo`, `SetAVTransportURI` with DIDL-Lite metadata, `Play`, `Pause`, `Seek` (REL_TIME), `Stop`, `SetVolume`; polls `GetPositionInfo` and `GetTransportInfo` once a second. A format missing from `GetProtocolInfo` is `FormatRejected { format }`, with no transcoding.
  - _Requirements: 10, 17, 19_
  - Depends on: cast#11, cast#1 (content types)
  - Wave: 2
  - Checks: fake SOAP server: each action, `GetProtocolInfo` rejection, position parsing, polling stops with the session.
  - Steering: none

- [ ] 14. Start and stop a cast session in CastService
  - Crate and files: `nemo-cast`: `src/service.rs`, `src/service/session.rs`.
  - Does: `cast_to` claims `RemoteSlot` (fails with `Remote(RemoteBusy)` while a room is live), sends `Park`, starts the server and loads the track at `at`. `stop_casting` stops the device, ends the server, closes the connection, frees the lease and returns the last device position. Any failure does the same and emits `Failed { reason }`; no `GET` within 8 s of `LOAD` emits `FirewallHint`.
  - _Requirements: 7, 12, 13, 15, 20, 21, 27_
  - Depends on: cast#2, cast#4, cast#6, cast#12, cast#13
  - Wave: 2
  - Checks: tests with the fake devices and a fake `PlayerControl`: slot refused while a room holds it, Park sent before `LOAD`, server gone after stop and after failure, position returned, firewall hint timer.
  - Steering: none

- [ ] 15. Route player commands and the queue to the device
  - Crate and files: `nemo-cast`: `src/service/gate.rs`, `src/service/queue.rs`.
  - Does: `gate` returns `Forward` for Pause, Resume, Seek, SetVolume and Stop and `Allow` for Next, Previous and queue edits; `handle` sends forwarded ones to the device. A parked `TrackChanged` becomes a new `LOAD`; `IDLE/FINISHED` emits `TrackEnded` so `nemo-app` sends `Next`, and the queue end stops casting. Volume goes to the device only; `remote_status` reports the device position, length and volume.
  - _Requirements: 10, 11, 28_
  - Depends on: cast#14
  - Wave: 2
  - Checks: gate table test per command; queue-advance and queue-end tests; a test that no software volume command reaches the local player while casting.
  - Steering: none

- [ ] 16. Pick the stream route per track
  - Crate and files: `nemo-cast`: `src/route.rs`.
  - Does: A local file goes through the server; a YouTube or SoundCloud stream uses `StreamRoute::default_for_youtube` from the spike, and a failed `Direct` load retries that track once as `Relay`. For a DLNA device the stream is resolved with `StreamRequest { prefer_codec: Some(Codec::Aac), .. }`. A format the device rejects shows `FormatRejected` and is never transcoded.
  - _Requirements: 17, 18, 19_
  - Depends on: cast#1 (result), cast#7, cast#14, account-actions#? (row 13: `StreamRequest`, `resolve_with`), soundcloud#? (row 9: `Origin::SoundCloud`, `MediaUrl`)
  - Wave: 2
  - Checks: tests: `Direct` URL handed as `Secret<String>` and never logged, `LOAD_FAILED` falls back to `Relay` once, AAC preference for DLNA, rejected format reported with its name.
  - Steering: none

- [ ] 17. Wire Cast into nemo-app
  - Crate and files: `nemo-app`: `src/backend.rs`, `src/ui_bridge.rs`, a new `src/cast_wiring.rs` if `backend.rs` would pass 500 lines.
  - Does: Builds `CastService` only when the Cast switch is on and `confirmed_version == version`, and hands the UI an `Arc<dyn CastControl>`. Applies `RemotePlayback::gate` to every UI command, refuses a song whose `DownloadService::state` is `SaveState::Saved` ("plays only inside NemoMusic"), and registers the `SignOutWipe` step "cast" that stops casting and ends the server.
  - _Requirements: 1, 13, 16, 22, 27_
  - Depends on: cast#15, cast#16, integrations#? (row 43: `OptInInfo`), account-actions#? (row 26: `SignOutWipe`), offline-cache#? (row 17: `DownloadService`)
  - Wave: 2
  - Checks: test with a counting fake network: zero discovery packets while the switch is off or unconfirmed; saved song refused; sign-out step stops the session and the server.
  - Steering: none

- [ ] 18. Add the Cast entry to settings
  - Crate and files: `nemo-cast` (`OptInInfo` for Cast), `nemo-ui`: `src/gui/views/settings.rs`.
  - Does: Shows the Cast `OptInRow`: what is sent (the playing stream to a device on the local network, through a LAN-only server), to whom (none on the internet), the risk (plain HTTP on the LAN to the chosen device only, certificates pinned on first use, not chain-checked), and that Windows may ask to allow NemoMusic through the firewall ("choose Private networks"). The switch stays off until confirmed.
  - _Requirements: 1, 2_
  - Depends on: cast#17, integrations#? (`OptInRow` in nemo-ui)
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; keyboard order and 200% text scale.
  - Steering: none

- [ ] 19. Build the device picker
  - Crate and files: `nemo-ui`: new `src/gui/views/cast_picker.rs`, `src/keys.rs`, `src/gui/views/overlays.rs`.
  - Does: A shortcut opens the picker (opening starts discovery, closing stops it). Lists "This computer" and each device with name, Cast or DLNA, and busy; arrow keys move, Enter selects, focus is visible. Empty state names the local network and the firewall and offers Retry; a busy device asks before take-over; a live room shows "A room is live" and blocks casting. Under reduced motion the search state is text and a static indicator.
  - _Requirements: 3, 4, 5, 6, 21, 23, 24, 27_
  - Depends on: cast#17
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; headless view tests for the empty, busy and room-live states.
  - Steering: none

- [ ] 20. Show casting on the player bar and in the signal path
  - Crate and files: `nemo-ui`: `src/gui/views/bar.rs`, `src/signal.rs`, `src/state.rs`.
  - Does: Shows "Casting to <device name>", position and length from `remote_status`, play, pause, seek, device volume, next, previous and stop. The signal path shows "Casting", the source codec and bitrate when known and "Decoded on the device"; no bit-perfect, gapless, "Lossless", "HD" or "Hi-Res" badge for a stream, and "Lossless source" for a local lossless file only when the device plays the file untouched, otherwise "Unknown".
  - _Requirements: 7, 8, 9, 10, 28, 29_
  - Depends on: cast#17
  - Wave: 3
  - Checks: unit tests of the badge rules (product.md quality badge rule, code.md bit-perfect rule); `design.md` checklist with a screenshot.
  - Steering: none

- [ ] 21. Show cast failures, hints and the hand-back
  - Crate and files: `nemo-ui`: `src/gui/views/overlays.rs`, `src/copy.rs`.
  - Does: Shows the reason of `Failed`, `Disconnected` and a changed certificate, with "Continue here", which sends `Unpark { at }` at the last device position. Shows `FirewallHint` with the steps to allow the app, `FormatRejected` with the format name, and the offline-cache refusal.
  - _Requirements: 6, 16, 17, 20_
  - Depends on: cast#19, cast#20
  - Wave: 3
  - Checks: headless view tests per message; `design.md` checklist with a screenshot.
  - Steering: none

- [ ] 22. Document Cast and the local network in the README
  - Crate and files: `README.md`.
  - Does: Adds the "Cast and local network" section: mDNS (UDP 5353, `224.0.0.251`), SSDP (UDP 1900, `239.255.255.250`), TCP 8009 and SOAP to the device, the LAN stream server on an OS-chosen port, the firewall prompt, and the `netsh advfirewall` rule of the design. No internet host is added.
  - _Requirements: 25_
  - Depends on: cast#17
  - Wave: 3
  - Checks: the `nemo-app` test that compares `OptInInfo.hosts` with the README egress list still passes (Cast lists none).
  - Steering: README section "Cast and local network" and the egress line "Cast: none on the internet".

- [ ] 23. Manual checks on real devices
  - Crate and files: none (results in the PR description).
  - Does: Casts a YouTube song, a SoundCloud song and a local FLAC to a Chromecast and to a DLNA speaker; checks controls, queue advance, stop and "Continue here". Checks the firewall prompt on Windows 11 for the Private and Public profiles and the picker with a VPN on.
  - _Requirements: 2, 6, 10, 11, 12, 18, 20_
  - Depends on: cast#18, cast#19, cast#20, cast#21, cast#22
  - Wave: 3
  - Checks: needs the owner and the hardware of task 1.
  - Steering: none

## Definition of Done

Every task meets [section 4 of `workflow.md`](../../steering/workflow.md#4-definition-of-done). Also for this spec:
- With the picker closed or the switch off, no discovery packet is sent and no socket is open (socket-counter test).
- The LAN server never binds an unspecified address and serves only the current session's item.
- No stream URL, token or cookie appears in logs, errors or messages to a device.
- `cargo test -p nemo-audio --test bit_exact` and the counting-allocator test pass after task 4 and after any task that touches `nemo-audio`.
