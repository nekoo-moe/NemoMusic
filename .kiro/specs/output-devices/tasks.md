## Tasks

- [ ] 1. Add the output device types to `nemo-core`
  - Crate and files: `nemo-core`: `src/output.rs` (new), `src/lib.rs` (one `pub mod` line). Touches nemo-core/src/output.rs and nemo-core/src/lib.rs.
  - Does: Adds `DeviceId`, `OutputDevice`, `SavedDevice`, `OutputOptions`, `OutputStatus` and `FallbackReason` (ADR 0004 Decision 4 row 5), signatures as in the design. `OutputOptions::default()` has exclusive off, `idle_release_secs` 5 (1 to 60), `exclusive_buffer_ms` 20 (5 to 100) and `auto_resume` off. `Debug` for `DeviceId` prints a hash, never the ID. No I/O.
  - _Requirements: 7, 13, 20, 23, 26_
  - Depends on: audio-effects#1 (both add a line to nemo-core/src/lib.rs; one at a time)
  - Wave: 0
  - Checks: unit tests for the defaults, the range clamps and that `format!("{:?}", id)` does not contain the ID.
  - Steering: `structure.md`, `nemo-core` ownership line: add `output.rs`. ADR 0004: no edit (Decision 4 row 5 already lists the types).

- [ ] 2. Extend the sink traits for health, reporting and device lists
  - Crate and files: `nemo-audio`: `src/sink.rs`, `src/lib.rs`.
  - Does: Adds `SinkHealth { Ok, Lost, Taken }`, `AudioSink::health()` (default `Ok`), `OpenedSink`, `OutputSelection`, and `SinkFactory::{open_reported, select, list_devices}` with defaults that keep `CpalSinkFactory` and the test fakes unchanged. `SinkError` gains `DeviceBusy`, `ExclusiveRefused(String)` and `FormatNotAccepted`.
  - _Requirements: none (groundwork)_
  - Depends on: output-devices#1
  - Wave: 0
  - Checks: `cargo test -p nemo-audio --test bit_exact`; `cargo test -p nemo-audio --test rt_no_alloc` (the callback is unchanged and must stay allocation-free).

- [ ] 3. Add the output commands, events and snapshot field
  - Crate and files: `nemo-core`: `src/player.rs`; `nemo-audio`: `src/control_cmd.rs`; exhaustive matches in `nemo-ui` and `nemo-app`. Touches nemo-core/src/player.rs.
  - Does: Adds `PlayerCommand::{ListOutputDevices, SetOutputDevice, SetOutputOptions, RetryOutput, ResumeOnReturnedDevice}`, `PlayerEvent::{OutputDevicesChanged, OutputChanged, OutputOpenFailed, DeviceLost, SavedDeviceReturned, OutputTaken, ExclusiveFellBack}` and `PlayerSnapshot.output` (row 5). `ListOutputDevices` answers from `SinkFactory::list_devices`; the other commands are accepted and do nothing until their tasks land, and no UI sends them before output-devices#20.
  - _Requirements: none (groundwork)_
  - Depends on: output-devices#1, output-devices#2, audio-effects#3 (only one task at a time edits nemo-core/src/player.rs, ADR 0004)
  - Wave: 0
  - Checks: `cargo test -p nemo-audio --test bit_exact`. Must not land at the same time as account-actions (`SwitchAudioTrack`) or cast (`Park`) tasks that edit `player.rs`.

- [ ] 4. Spike: match `cpal` device IDs to WASAPI endpoint IDs
  - Crate and files: workspace `Cargo.toml` (`wasapi` =0.27.0 and `windows` ^0.62.2 pinned in `[workspace.dependencies]`); `nemo-audio`: `Cargo.toml` (`[target.'cfg(windows)'.dependencies]`), `src/wasapi_sink.rs` (module root, new; `foo.rs` plus `foo/` as `structure.md` asks, not the design's `mod.rs`), `tests/device_id_windows.rs` (new, `#[ignore]`).
  - Does: Must prove three things. (a) `wasapi` builds on the pinned toolchain with no C toolchain. (b) For every output device, the endpoint ID inside `cpal`'s `DeviceId` string equals the ID that `wasapi`'s own enumeration gives, and two devices with the same name get two IDs. (c) The `cpal` 0.18.2 tag (registry source in `Cargo.lock`) opens WASAPI in shared mode only. Pass: output-devices#15 finds the endpoint by the ID inside the `cpal` string. Fail on (b): output-devices#15 adds a map built from `wasapi`'s enumeration in `device_factory.rs` (design, Risks), about 60 more lines. Fail on (a) or (c): stop and ask the owner. The result goes into the design's Risks section.
  - _Requirements: 7_
  - Depends on: output-devices#2
  - Wave: 1
  - Checks: the `#[ignore]` test on a Windows machine with at least one output device; two identical devices if the owner has them. `cargo deny check` for the new crates (MIT). `cargo test -p nemo-audio --test bit_exact`.
  - Steering: `tech.md`: `wasapi` =0.27.0 row (MIT, MSRV 1.85, released 2026-11-06, Windows only, needs `windows` ^0.62.2, C toolchain result); change the "Exclusive mode" status to "verified: `cpal` 0.18.2 is shared-only".

- [ ] 5. Save the chosen device by ID and name
  - Crate and files: `nemo-app`: `src/config.rs`, `src/ui_bridge.rs`.
  - Does: `output_device` becomes `{ id, name }` and `OutputOptions` is saved beside it, clamped by `sanitized()`. An old file with a plain name still loads: it is matched by name once, then saved with the ID. The name is only the label shown when the device is absent.
  - _Requirements: 6, 7_
  - Depends on: output-devices#1
  - Wave: 1
  - Checks: round trip; an old plain-name file loads and upgrades; out-of-range options are clamped.

- [ ] 6. List devices with IDs and report health in the `cpal` sink
  - Crate and files: `nemo-audio`: `src/cpal_sink.rs`.
  - Does: Lists every output device with its `DeviceId`, name and the default mark, and opens a device by `DeviceId` instead of by name. The `cpal` error callback sets `SinkHealth::Lost` in an atomic that `health()` reads; the data callback is unchanged and stays allocation-free, lock-free and silent (code.md). Device names and IDs are logged at `debug` only.
  - _Requirements: 1, 7, 9, 22, 26_
  - Depends on: output-devices#2
  - Wave: 1
  - Checks: `cargo test -p nemo-audio --test bit_exact`; `cargo test -p nemo-audio --test rt_no_alloc`; an `#[ignore]` test that lists real devices.

- [ ] 7. Open the chosen device through a new `DeviceSinkFactory` (shared mode)
  - Crate and files: `nemo-audio`: `src/device_factory.rs` (new), `src/lib.rs`; `nemo-app`: `src/backend.rs` (replaces `CpalSinkFactory`).
  - Does: Holds the `OutputSelection` that `select()` sets and opens that device in shared mode, labelled shared and never bit-perfect. No choice means the system default; a saved device that is absent opens the default and sets `saved_missing`. It only opens its own stream, so the system default and other apps' volume and routing stay as they were.
  - _Requirements: 2, 3, 8, 12, 26_
  - Depends on: output-devices#5, output-devices#6
  - Wave: 2
  - Checks: `cargo test -p nemo-audio --test bit_exact`; `cargo test -p nemo-audio --test rt_no_alloc`; a test with a capturing `tracing` layer that no device name or ID appears at `info` or above (criterion 26).
  - Steering: `structure.md`, `nemo-audio` ownership line: add the device factory, device switching and the WASAPI exclusive sink.

- [ ] 8. Carry the decoders out of a stopped pipeline
  - Crate and files: `nemo-audio`: `src/pipeline.rs` (`Pipeline::stop_and_take`), `src/feeder.rs`.
  - Does: `stop_and_take()` joins the feeder and returns the active and queued `Job`s with their open decoders, so a new pipeline can continue them without a network re-resolve. The ring discipline is unchanged.
  - _Requirements: 4_
  - Depends on: output-devices#2
  - Wave: 2
  - Checks: a test that the returned jobs keep their decoder position; `cargo test -p nemo-audio --test bit_exact`; `cargo test -p nemo-audio --test rt_no_alloc`.

- [ ] 9. Switch device during playback
  - Crate and files: `nemo-audio`: `src/control.rs`, `src/control_cmd.rs` (`SetOutputDevice`); `nemo-app`: `src/ui_bridge.rs` (saves the choice on `OutputChanged`).
  - Does: Opens and tests the new sink first; on failure the old device keeps playing, `OutputOpenFailed` says why and the saved choice stays. On success control reads the position (played, start, base, minus latency), calls `stop_and_take()`, starts a new pipeline and seeks the carried decoder; queue and `Run` state are untouched. The choice is saved only after a successful open.
  - _Requirements: 4, 5, 6_
  - Depends on: output-devices#3, output-devices#7, output-devices#8
  - Wave: 2
  - Checks: control tests with fake sinks (`tests/common/fakes.rs`): position and queue kept, a failed open keeps the old device and the saved choice; no sample step above the click threshold across the switch; `cargo test -p nemo-audio --test bit_exact`; `cargo test -p nemo-audio --test rt_no_alloc`. The 500 ms gap is measured in output-devices#22.

- [ ] 10. Watch for device changes
  - Crate and files: `nemo-audio`: `src/device_watch.rs` (new).
  - Does: A `nemo-devices` thread polls `cpal` once a second and sends `OutputDevicesChanged` on the player's event path. With no device chosen, a new default triggers the switch of output-devices#9; the saved device seen again triggers the return handling of output-devices#11. If the thread fails, updates stop and `ListOutputDevices` still refreshes by hand.
  - _Requirements: 3, 8, 10, 11_
  - Depends on: output-devices#9
  - Wave: 2
  - Checks: unit tests with a fake device enumerator for add, remove, default change and return; no `unwrap`.

- [ ] 11. Pause on device loss and offer to resume on return
  - Crate and files: `nemo-audio`: `src/control_tick.rs`, `src/control_cmd.rs` (`ResumeOnReturnedDevice`).
  - Does: The 5 ms tick reads `health()`; `Lost` pauses within one tick, keeps the ring and decoder, and sends `DeviceLost { name }`. No other device is chosen for the user. When the device returns, `SavedDeviceReturned` offers the resume and nothing resumes by itself (owner, 2026-11-14); a saved device that appears while stopped or paused is switched to at once. `auto_resume` stays off and hidden.
  - _Requirements: 8, 9, 10_
  - Depends on: output-devices#6, output-devices#10
  - Wave: 2
  - Checks: control tests with fakes: loss pauses within one tick, return sends the offer and does not resume; `cargo test -p nemo-audio --test bit_exact`.

- [ ] 12. Pack `f32` samples into device bytes
  - Crate and files: `nemo-audio`: `src/wasapi_sink/pack.rs` (new; builds on every OS so the tests run on Linux CI too).
  - Does: `pack_f32(samples, out, layout)` for `I16`, `I24Packed`, `I24In32`, `I32` and `F32`: the exact inverse of the decoder's integer-to-`f32` scaling, with rounding and clamping and no dither. Pure and `/// RT-SAFE`.
  - _Requirements: 17, 22_
  - Depends on: output-devices#4
  - Wave: 2
  - Checks: `cargo test -p nemo-audio --test bit_exact` new cases: 16-bit and 24-bit WAV fixtures through `I16`, `I24Packed` and `I24In32` give bytes equal to the source integers; `proptest` round trip for each layout; counting-allocator case in `cargo test -p nemo-audio --test rt_no_alloc`.

- [ ] 13. Negotiate the exclusive format
  - Crate and files: `nemo-audio`: `src/wasapi_sink/negotiate.rs` (new; every OS).
  - Does: Builds the format ladder at the song's rate and channel count: known depth `d` gives `d` packed, `d` in 32, 32-bit integer, `f32`; unknown depth gives 24 in 32, 32-bit integer, 24 packed, 16, `f32`. If nothing fits, tries the device's other rates nearest first, integer multiples first. Reports valid bits, not the container; a source above 24 bits reports 24. Maps `ExclusiveError` to `FallbackReason`.
  - _Requirements: 14, 15, 17_
  - Depends on: output-devices#1, output-devices#4
  - Wave: 2
  - Checks: unit tests against a fake "device accepts" predicate for 16, 24, 32 and unknown depth, nearest-rate choice and every `FallbackReason` mapping; `cargo test -p nemo-audio --test bit_exact`.

- [ ] 14. Run the exclusive render loop against a backend trait
  - Crate and files: `nemo-audio`: `src/wasapi_sink/thread.rs` (new; every OS), `tests/common/fakes.rs` (memory backend), `benches/` (render loop).
  - Does: The `nemo-wasapi` thread waits on the backend event, asks for the free frames, calls the same `RenderCallback` on a preallocated `f32` scratch in fixed chunks, packs into a preallocated byte buffer, and writes it. Device errors set `SinkHealth::Lost` or `Taken`; underruns are counted, never blocked on. The loop body is `/// RT-SAFE`: no allocation, lock, blocking call other than the event wait, or logging (code.md).
  - _Requirements: 9, 21, 22, 23_
  - Depends on: output-devices#2, output-devices#12
  - Wave: 2
  - Checks: `cargo test -p nemo-audio --test bit_exact` runs the whole loop on the memory backend; `cargo test -p nemo-audio --test rt_no_alloc`: zero allocations over 10 s, normal and with underruns; the benchmark documents the worst-case loop time. Manual: on the owner's reference hardware in output-devices#22.

- [ ] 15. Open WASAPI exclusive mode on Windows
  - Crate and files: `nemo-audio`: `src/wasapi_sink.rs` (`#[cfg(windows)] WasapiExclusiveSink` and the `wasapi` backend).
  - Does: Finds the endpoint as the spike decided, negotiates through output-devices#13, and opens event-driven exclusive mode with a buffer of `exclusive_buffer_ms` or the device's minimum period if larger, and reports the size used. Implements `AudioSink` (mode `Exclusive`, the negotiated format, `can_follow_source` true) and maps busy, refused, control disabled and no format to `ExclusiveError`. Uses the safe `wasapi` API only; if `unsafe` seems needed, stop and raise it (code.md, Unsafe: only an `ffi` module, with `// SAFETY:` comments and its own review).
  - _Requirements: 14, 15, 16, 22, 23_
  - Depends on: output-devices#4, output-devices#13, output-devices#14
  - Wave: 2
  - Checks: `cargo test -p nemo-audio --test bit_exact`; `cargo test -p nemo-audio --test rt_no_alloc`; an `#[ignore]` Windows test that opens a real device. Manual: on the owner's reference hardware in output-devices#22.

- [ ] 16. Choose exclusive or shared in the factory and fall back
  - Crate and files: `nemo-audio`: `src/device_factory.rs`, `src/control.rs`.
  - Does: `open_reported` tries exclusive mode when the option is on and the OS is Windows; on `ExclusiveError` it opens shared mode at once, keeps playing and sends `ExclusiveFellBack(reason)` (owner, 2026-11-14: no question each time). On Linux and other OSes `exclusive_available` is false and the reason is `NotSupportedHere`.
  - _Requirements: 13, 16, 27_
  - Depends on: output-devices#9, output-devices#15
  - Wave: 2
  - Checks: factory tests with a fake exclusive backend for each failure; `cargo test -p nemo-audio --test bit_exact` on the exclusive path through the fake; `cargo test -p nemo-audio --test rt_no_alloc`. Manual: on the owner's reference hardware in output-devices#22 (device in use, "allow exclusive control" off).

- [ ] 17. Keep the exclusive path bit-perfect and apply the output options
  - Crate and files: `nemo-audio`: `src/control.rs`, `src/control_cmd.rs` (`SetOutputOptions`), the signal-path report; `nemo-app`: `src/ui_bridge.rs` (saves the options).
  - Does: In exclusive mode control keeps the stored volume but forces unity gain, so no `Gain` stage exists, and the snapshot says the volume is left to the device and OS. The path is labelled bit-perfect only when the device is exclusive, no stage is in the path (including audio-effects' stages) and the device format equals the decoded format by valid bits. `SetOutputOptions` reopens through the switch path, so the user can turn exclusive mode on or off and raise the buffer.
  - _Requirements: 12, 17, 19, 23_
  - Depends on: output-devices#16, audio-effects#2
  - Wave: 2
  - Checks: control tests: no gain stage in exclusive mode; `is_bit_perfect` false with each effect stage, with a format mismatch and in shared mode; `cargo test -p nemo-audio --test bit_exact` on the exclusive path through the fake; `cargo test -p nemo-audio --test rt_no_alloc`. Manual: on the owner's reference hardware in output-devices#22.

- [ ] 18. Reopen the exclusive device when the song format changes
  - Crate and files: `nemo-audio`: `src/control_tick.rs`.
  - Does: With an exclusive sink, a next song in another format waits for the audible end of the current one, then reopens the device in the new format without a click; songs in the same format stay gapless with no reopen. audio-effects#18 builds on this to skip the crossfade in that case.
  - _Requirements: 18_
  - Depends on: output-devices#16
  - Wave: 2
  - Checks: control tests with a fake exclusive sink: same format means no reopen and a gapless join, a new format means one reopen and no step above the click threshold; `cargo test -p nemo-audio --test bit_exact`. Manual: on the owner's reference hardware in output-devices#22 (44.1 kHz to 96 kHz and back).

- [ ] 19. Release an idle exclusive device and recover when another app takes it
  - Crate and files: `nemo-audio`: `src/control.rs`, `src/control_tick.rs`, `src/control_cmd.rs` (`RetryOutput`).
  - Does: On pause or stop with an exclusive sink, a timer of `idle_release_secs` (default 5) runs `stop_and_take()` and keeps the jobs; `Resume` or `Play` reopens through the same path. Shared mode is never released. `SinkHealth::Taken` pauses and sends `OutputTaken`; `RetryOutput` reopens. Repeated underruns raise the hint to increase `exclusive_buffer_ms`.
  - _Requirements: 20, 21, 23_
  - Depends on: output-devices#11, output-devices#17
  - Wave: 2
  - Checks: control tests with fakes: release at 5 s and reopen on resume, taken pauses and retry reopens; `cargo test -p nemo-audio --test bit_exact`; `cargo test -p nemo-audio --test rt_no_alloc`. Manual: on the owner's reference hardware in output-devices#22.

- [ ] 20. Build the output device screen
  - Crate and files: `nemo-ui`: `src/gui/views/settings_output.rs` (new), `src/gui/views/settings.rs` (Audio output section), `src/copy.rs`.
  - Does: Lists every device with the default marked and the saved choice shown, "not connected" when it is absent; picking sends `SetOutputDevice`. On Windows, the exclusive switch (off by default) with the text that other apps cannot use the device while it is on, plus the buffer and the release delay; on Linux no switch and a line that the exclusive path is planned. Keyboard reachable with a visible focus; the list follows reduced motion.
  - _Requirements: 1, 8, 13, 23, 24, 25, 27_
  - Depends on: output-devices#10, output-devices#17
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; headless snapshots on the Windows and Linux layouts.

- [ ] 21. Show output messages and the output line of the signal path
  - Crate and files: `nemo-ui`: `src/gui/views/overlays.rs`, `src/signal.rs`, `src/copy.rs`.
  - Does: Plain messages for `DeviceLost` (names the device), `OutputOpenFailed`, `ExclusiveFellBack` (one sentence per `FallbackReason`), `OutputTaken` with a Retry action, `SavedDeviceReturned` with a Resume action, and the underrun hint. The signal path shows shared or exclusive mode, the negotiated format, any resampling or conversion, and "volume left to the device" in exclusive mode; shared mode is never shown as bit-perfect. Actions are reachable by keyboard and messages follow reduced motion.
  - _Requirements: 5, 8, 9, 10, 12, 14, 15, 16, 21, 23, 24, 25_
  - Depends on: output-devices#19, output-devices#20
  - Wave: 3
  - Checks: `signal.rs` unit tests for each output state; `design.md` checklist with a screenshot.

- [ ] 22. Test the output path on the owner's reference hardware
  - Crate and files: none (manual); results go into the design's Risks section.
  - Does: On the reference machine and USB DAC: exclusive mode at 44.1/16, 96/24 and 192/24 with the bit-perfect label checked against the DAC's rate display; a format change between songs; unplug during playback (pause within 500 ms, no other device chosen); another app taking the device; the Windows "allow exclusive control" setting off; a device switch gap under 500 ms for a local file and a stream; one hour at the 20 ms buffer with no underrun; shared mode leaves the system default and other apps alone. If underruns need MMCSS priority, a follow-up task adds `nemo-audio/src/ffi/mmcss.rs` with `// SAFETY:` comments and its own review (code.md, Unsafe); a stream switch over 500 ms goes to the owner.
  - _Requirements: 2, 4, 9, 14, 16, 17, 18, 21, 23_
  - Depends on: output-devices#21, audio-effects#18
  - Wave: 3
  - Checks: needs the owner: the reference hardware is product.md Open decision 7 and requirements Open question 4.

## Definition of Done

See [section 4 of workflow.md](../../steering/workflow.md#4-definition-of-done). For this spec also:

- `cargo test -p nemo-audio --test bit_exact` passes on every task that changes `nemo-audio`, and covers the exclusive path through the memory backend (criterion 22).
- `cargo test -p nemo-audio --test rt_no_alloc` covers the shared callback and the exclusive render loop, and every function they reach is marked `/// RT-SAFE` (code.md).
- No device name or ID is logged at `info` or above, and no task contacts a network host (criterion 26).
- The release that first shows the exclusive switch needs output-devices#22 passed.
