## Tasks

Cross-spec ids written `<spec>#?` point at the task in that spec that adds the named ADR 0004 Decision 4 row; fill in the number when that spec's `tasks.md` lands. `RemotePlayback`, `RemoteSlot` and `RoomRole` come from cast (row 41).

- [ ] 1. Spike: talk to a self-hosted metroserver
  - Crate and files: none merged. A throwaway probe outside the repo, built from the published `listentogether.proto`; the result goes into `.kiro/specs/listen-together/design.md` (Risks and Alternatives), with the pinned `.proto` and server commits.
  - Does: Runs metroserver in Docker on this computer and connects two probe clients with `User-Agent: NemoMusic/<version>`. Creates and joins a room, plays, pauses and seeks, and checks the units of positions and durations, whether a track id is a YouTube video id, which `client_capabilities` the server accepts, `supports_compression`, and the host handover after 30 s.
  - Pass: milliseconds, video ids, and the client name accepted under the default User-Agent policy, so the design stands. Fail: other units change the codec (task 4) and clock math (task 7); non-video ids make task 11 show "cannot play" for them; a refused User-Agent stops the feature and goes to the owner (criterion 26: the client is never renamed). A feature the server rejects is dropped from the launch set in task 6.
  - _Requirements: 26, 31_
  - Depends on: none
  - Wave: 1
  - Checks: needs Docker on the owner's machine; Metrolist on a phone as a third member if the owner has one. No public server is contacted.
  - Steering: none

- [x] 2. Add the room control trait and its types to nemo-core
  - Crate and files: `nemo-core`: new `src/together.rs`, `src/lib.rs`. Touches nemo-core/src/together.rs and nemo-core/src/lib.rs.
  - Does: Adds `LinkState`, `MemberId`, `Member`, `RoomStatus`, `RoomEvent`, `RoomEventReceiver`, the `RoomControl` trait and `RoomError` (`thiserror`, the variants of the design's Error Handling), because the trait returns it (ADR 0004 row 42).
  - _Requirements: none (groundwork)_
  - Depends on: cast#2
  - Wave: 0
  - Checks: `RoomError` `Display` holds no display name, room code or token.
  - Steering: ADR 0004 Decision 2 adds `RoomControl`, and notes that `nemo-together` takes `Arc<dyn PlayerControl>` from `nemo-app`.

- [ ] 3. Create the nemo-together crate and validate server addresses
  - Crate and files: new `crates/nemo-together/` (`Cargo.toml`, `src/lib.rs`, `src/address.rs`), workspace member entry.
  - Does: Adds the crate with `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]` and `[lints] workspace = true`, depending on `nemo-core` only. `ServerAddress::parse` accepts `wss://host[:port]/path`, and `ws://` only for loopback, private-range and `.local` hosts. No default address exists anywhere in the crate.
  - _Requirements: 3_
  - Depends on: listen-together#2
  - Wave: 0
  - Checks: `ServerAddress` tests for each accepted and refused form (property test on the parser); `cargo tree -p nemo-together --edges normal` shows no other `nemo-*` crate.
  - Steering: `structure.md` adds `nemo-together` to the layout, the ownership list and the graph.

- [ ] 4. Vendor listentogether.proto and add the envelope codec
  - Crate and files: `nemo-together`: `proto/listentogether.proto`, `build.rs`, `src/codec.rs`.
  - Does: Generates the messages with `prost-build` and `protox` (no `protoc`). Encodes and decodes `Envelope { type, payload, compressed, request_id }`, gzips payloads over 100 bytes when the server supports it, and drops frames over 1 MiB, over 4 MiB after decompression, track ids over 200 bytes and queues over 500 tracks as `BadMessage`. Unknown types and fields are ignored.
  - _Requirements: 31_
  - Depends on: listen-together#1 (units), listen-together#3, cast#8 (`prost`, `prost-build`, `protox` rows)
  - Wave: 1
  - Checks: round-trip every message type with and without gzip; oversize frame and zip-bomb tests; unknown type ignored; `proptest` on the decoder.
  - Steering: `tech.md` row for `flate2` 1.1.11 (as locked) in `nemo-together`. The `listentogether.proto` licence notice (GPL-3.0, source commit) joins the licence notices shipped with the app.

- [ ] 5. Store the display-name blocklist
  - Crate and files: `nemo-together`: `src/blocklist.rs`, migrations for the crate's SQLite file in the app data folder (ADR 0004 Decision 3).
  - Does: Stores display names the host blocked on this computer, and checks a join request against them. It is a convenience, not security.
  - _Requirements: 20_
  - Depends on: listen-together#3
  - Wave: 1
  - Checks: tests: add, check, migration from empty.
  - Steering: `tech.md` notes `rusqlite` 0.41.1 (as locked) for `nemo-together`.

- [ ] 6. Open the room socket
  - Crate and files: `nemo-together`: `src/socket.rs`.
  - Does: One tokio task owns a `tokio-tungstenite` WebSocket (binary frames) with `User-Agent: NemoMusic/<version>`, takes commands on an `mpsc` channel, and puts events on the queue read by `RoomEventReceiver::try_next`. Sends `client_capabilities` with the launch set of the design. A failed first connect returns `Unreachable { host }` and does not retry.
  - _Requirements: 25, 26_
  - Depends on: listen-together#4
  - Wave: 1
  - Checks: test against an in-process WebSocket server: User-Agent header, capabilities sent, unreachable host reported once with no retry.
  - Steering: `tech.md` row for `tokio-tungstenite` =0.30.0 (feature `native-tls`, reusing `tokio-native-tls` 0.3.1; confirm the feature name).

- [ ] 7. Map server time to local time
  - Crate and files: `nemo-together`: `src/clock.rs`.
  - Does: Pings with `client_time` at 250 ms three times after connect, then every 5 s while playing. Keeps the best of the last 8 round trips to map server time to local time, and computes the room position as `position + (server_now - last_update)` while playing.
  - _Requirements: 13_
  - Depends on: listen-together#1 (units), listen-together#6
  - Wave: 2
  - Checks: unit tests with scripted round trips, asymmetric delays and a paused room.
  - Steering: none

- [ ] 8. Build a fake metroserver for tests
  - Crate and files: `nemo-together`: `tests/fake_server.rs` (test support only).
  - Does: An in-process WebSocket server that speaks the codec and runs scripted rooms: create, join with approve and reject, host handover, kick, server restart, forced disconnect, wrong code and full room.
  - _Requirements: none (groundwork)_
  - Depends on: listen-together#6
  - Wave: 2
  - Checks: it is the harness for tasks 9 to 15.
  - Steering: none

- [ ] 9. Create, join, approve and leave a room
  - Crate and files: `nemo-together`: `src/room.rs`, `src/room/membership.rs`.
  - Does: Creates a room (the user becomes host and gets the code), joins with a code and a display name, waits for approval with Cancel, and lets the host approve or reject, or auto-approve (off by default). A wrong code or full room returns the server's reason and stays out. Leave closes the socket, drops the session token and stops following the room; the local queue stays.
  - _Requirements: 4, 6, 7, 8, 19, 22_
  - Depends on: listen-together#5, listen-together#8
  - Wave: 2
  - Checks: fake-server tests for each path; members carry only a display name, a host mark and a connected state.
  - Steering: none

- [ ] 10. Handle host handover, kicks and blocks
  - Crate and files: `nemo-together`: `src/room/moderation.rs`.
  - Does: On a host change every member gets `HostChanged { new_host }` and the control rights follow. The host can kick a member with a reason, which ends that member's session with `Kicked`, and can block the display name on this computer.
  - _Requirements: 20, 21_
  - Depends on: listen-together#9
  - Wave: 2
  - Checks: fake-server tests: handover after the host is gone, kick with reason, blocked name refused on the next request.
  - Steering: none

- [ ] 11. Follow the room as a guest
  - Crate and files: `nemo-together`: `src/guest.rs`.
  - Does: `change_track` becomes `Play(track)` then `Pause`; when the track is ready it sends `buffer_ready` with the `track_revision`, and on `buffer_complete` or `sync_playback{play}` it issues `Seek` and `Resume`. A join starts the room's track at the room's position. A local pause leaves the room running; Resume seeks to the room position first. A track id that fails `VideoId::parse` gives `CannotPlay`.
  - _Requirements: 10, 11, 12_
  - Depends on: listen-together#7, listen-together#9
  - Wave: 2
  - Checks: fake-server and fake `PlayerControl` tests: join mid-track, local pause and rejoin, foreign id.
  - Steering: none

- [ ] 12. Correct guest drift by seeking
  - Crate and files: `nemo-together`: `src/drift.rs`.
  - Does: Compares `pong.drift_ms` with 500 ms (a starting value) and above it sends one `PlayerCommand::Seek` to the room position. Ignores drift for 3 s after a seek or track change and corrects at most once per 10 s. It never changes speed or resamples.
  - _Requirements: 13, 14_
  - Depends on: listen-together#11
  - Wave: 2
  - Checks: simulated player clock drifting by ±2 s: converges below 500 ms, no seek within 3 s of a seek, at most one seek per 10 s, no speed or rate command ever sent.
  - Steering: none

- [ ] 13. Send the host's playback to the room
  - Crate and files: `nemo-together`: `src/host.rs`.
  - Does: `TrackChanged`, `StateChanged`, seeks and queue changes become `playback_action` messages with a rising `host_sequence`. Only the video id, title, artist, album, duration and thumbnail URL leave the machine. A local file or SoundCloud track is not sent and the host gets `CannotPlay`.
  - _Requirements: 10, 17, 18_
  - Depends on: listen-together#9, soundcloud#? (row 9: `Origin::SoundCloud`)
  - Wave: 2
  - Checks: privacy test: every sent message holds only the fields of criterion 17, never lyrics, file names, tags, account data, cookies or stream URLs; local and SoundCloud tracks never sent.
  - Steering: none

- [ ] 14. Reconnect after a dropped connection
  - Crate and files: `nemo-together`: `src/reconnect.rs`.
  - Does: Up to 15 attempts, 1 s doubling to 120 s with jitter, using `reconnect{session_token}`; the token is a `Secret<String>` in memory only. Playback continues locally meanwhile and `LinkState::Reconnecting { attempt, of }` shows the state. `server_restarting` schedules one attempt after `eta_ms`. After the last attempt the client leaves the room with a message.
  - _Requirements: 24_
  - Depends on: listen-together#9
  - Wave: 2
  - Checks: fake-server tests: forced disconnect and reconnect, server restart, give-up after 15 attempts (with a test clock), token never logged.
  - Steering: none

- [ ] 15. Implement TogetherClient with the command gate
  - Crate and files: `nemo-together`: `src/client.rs`, `src/gate.rs`.
  - Does: `TogetherClient` implements `RoomControl` and `RemotePlayback`. Creating or joining claims `RemoteSlot` as `Room` and fails with `Busy(RemoteKind::Cast)` while casting. As guest, `gate` denies track changes, play, pause, seek and queue order with a reason; as host it allows them.
  - _Requirements: 9, 30_
  - Depends on: listen-together#10, listen-together#11, listen-together#12, listen-together#13, listen-together#14, cast#2
  - Wave: 2
  - Checks: gate table test for host and guest; exclusion test both ways with Cast holding the slot.
  - Steering: none

- [ ] 16. Wire Listen Together into nemo-app
  - Crate and files: `nemo-app`: `src/backend.rs`, `src/ui_bridge.rs`, a new `src/together_wiring.rs` if `backend.rs` would pass 500 lines.
  - Does: Builds `TogetherClient` only when the switch is on and confirmed for the typed host (another host asks again), and applies `RemotePlayback::gate` to UI commands. On room start sends `SetTransition` with `TransitionMode::Off` and `SkipFade::Off`, and on leave restores the saved `TransitionSettings` unchanged. Turning the feature off or the `SignOutWipe` step "together" leaves the room and drops the token.
  - _Requirements: 1, 2, 15, 16, 23, 30_
  - Depends on: listen-together#15, integrations#? (row 43: `OptInInfo`), audio-effects#? (row 1: `SetTransition`), account-actions#? (row 26: `SignOutWipe`)
  - Wave: 2
  - Checks: counting fake host: zero connections while off or unconfirmed; transition restored exactly after leave; the signal path and quality badge show what plays on this computer while in a room. `nemo-audio` is untouched.
  - Steering: none

- [ ] 17. Add the Listen Together entry to settings
  - Crate and files: `nemo-together` (`OptInInfo`), `nemo-ui`: `src/gui/views/settings.rs`.
  - Does: Shows the `OptInRow` with a server address field and no default, labelled "the server you set". The panel shows the typed host, what it receives (display name, room code, track ids and titles, play state and position, IP address) and the risk: unofficial, no known service terms, a leave may not remove data on the server, and a `ws://` LAN address shows room data and the IP to that network. Settings shows the address in use.
  - _Requirements: 1, 2, 3, 29_
  - Depends on: listen-together#16, integrations#? (`OptInRow` in nemo-ui)
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; a different host reopens the panel.
  - Steering: none

- [ ] 18. Add the create and join dialogs
  - Crate and files: `nemo-ui`: new `src/gui/views/together.rs`, `src/keys.rs`.
  - Does: Create shows the room code, Copy and Leave. Join asks for a code and a display name with no default from the Google account. "Waiting for the host" with Cancel; wrong code, full room and refusals show the server's reason. Every action works with the keyboard with visible focus.
  - _Requirements: 4, 5, 6, 7, 27_
  - Depends on: listen-together#16
  - Wave: 3
  - Checks: headless view tests; `design.md` checklist with a screenshot.
  - Steering: none

- [ ] 19. Add the room panel
  - Crate and files: `nemo-ui`: new `src/gui/views/together_room.rs`.
  - Does: Lists members by display name with a host mark and connected state; the host sees join requests with Approve and Reject, the auto-approve switch, and Kick with a reason and Block. Shows host changes, "the room cannot play this", the link state while reconnecting, and server messages as received. Under reduced motion joins and leaves appear without movement.
  - _Requirements: 8, 18, 19, 20, 21, 24, 26, 27, 28_
  - Depends on: listen-together#18
  - Wave: 3
  - Checks: headless view tests; `design.md` checklist with a screenshot.
  - Steering: none

- [ ] 20. Show the live room on the player bar
  - Crate and files: `nemo-ui`: `src/gui/views/bar.rs`, `src/copy.rs`.
  - Does: Shows a dot and label while a room is live. Trying to cast in a room, or to start or join a room while casting, says which one is active.
  - _Requirements: 16, 27, 30_
  - Depends on: listen-together#16, cast#20 (same file, `bar.rs`)
  - Wave: 3
  - Checks: headless view test; `design.md` checklist with a screenshot.
  - Steering: none

- [ ] 21. Describe the room server in the README
  - Crate and files: `README.md`.
  - Does: The egress list says the room server is the one the user types, that no default exists, and what it receives, including the IP address. No host is named until one becomes a listed choice.
  - _Requirements: 29_
  - Depends on: listen-together#17
  - Wave: 3
  - Checks: the `nemo-app` test comparing `OptInInfo.hosts` with the README egress list passes.
  - Steering: README egress line for Listen Together ("the server you type, only when Listen Together is on").

- [ ] 22. Interop and listening checks with metroserver
  - Crate and files: `nemo-together`: `tests/interop_metroserver.rs` (`#[ignore]`, run by hand).
  - Does: Runs the client against a self-hosted metroserver in Docker to check units, id format and the User-Agent. On two machines the owner listens in a room, checks that drift seeks do not click badly, and sets the 500 ms threshold.
  - _Requirements: 13, 26, 31_
  - Depends on: listen-together#19, listen-together#20
  - Wave: 3
  - Checks: needs the owner: Docker and two machines. Listing The Meowery as a choice needs its operator's consent first (owner action, ADR 0004).
  - Steering: none

## Definition of Done

Every task meets [section 4 of `workflow.md`](../../steering/workflow.md#4-definition-of-done). Also for this spec:
- No connection opens while the switch is off, unconfirmed, or confirmed for another host (counting fake host).
- Only the fields of criterion 17 leave the machine (privacy test), and no display name, room code or token is logged.
- Drift is corrected only by seeking; `nemo-audio` is untouched and `cargo test -p nemo-audio --test bit_exact` passes.
