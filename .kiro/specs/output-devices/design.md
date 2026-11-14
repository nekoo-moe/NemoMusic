## Overview

The device picker stays on `cpal` (shared mode, as `tech.md` decided). Exclusive mode is a second sink, `WasapiExclusiveSink`, built on the `wasapi` crate 0.27.0 and used on Windows only. `cpal` 0.18.2 cannot do exclusive mode: its WASAPI backend creates every stream with `AUDCLNT_SHAREMODE_SHARED` (checked in `src/host/wasapi/device.rs` on the master branch; the 0.18.2 tag itself was not read). So `wasapi` is needed for the exclusive path only, and `cpal` keeps listing devices and shared output on every platform.

A new `DeviceSinkFactory` replaces `CpalSinkFactory` in `nemo-app`. It reads a small `OutputSelection` (device, exclusive on or off, buffer, idle delay) and returns either sink. If exclusive fails it returns a shared sink and a `FallbackReason`, so the player can say why.

Decisions:
- **Device identifier:** `cpal`'s `DeviceId` string (`Device::id()`, `Display` and `FromStr`, host-prefixed). It is the endpoint ID on Windows and an ALSA name on Linux, so two identical headphones are two devices. Saved with the display name; the name is only the label shown when the device is absent. The `wasapi` side finds the same endpoint by the ID inside that string. That the two IDs agree on Windows is unverified and has a test (below).
- **No unsafe code.** `wasapi` has a safe API (event handles, buffers, notifications). If the hardware test shows underruns that need MMCSS thread priority, `nemo-audio/src/ffi/mmcss.rs` is added later, with `// SAFETY:` comments and its own review (`code.md`, Unsafe). Nothing in this design needs it.
- **Device-list changes** come from a 1 s poll of `cpal` on a small watcher thread, on every platform. Device loss does not wait for the poll: the sink reports it from its own error path.
- **Bit depth rule (criterion 17):** the sink reports the valid bits, not the container. A 24-bit song in a 32-bit container counts as 24-bit. A source above 24 bits is never reported bit-perfect, because the `f32` pipeline cannot hold it exactly; the sink reports 24 bits for it, so `is_bit_perfect` sees a mismatch.

## Affected Crates

- `nemo-core`: new `output.rs` (device and selection types), `PlayerCommand` and `PlayerEvent` values, `PlayerSnapshot.output`. No I/O.
- `nemo-audio`: new `device_factory.rs`, `device_watch.rs`, `wasapi_sink/` (Windows only: `mod.rs`, `negotiate.rs`, `pack.rs`, `thread.rs`), `cpal_sink.rs` (adds `DeviceId` listing and `health()`), `sink.rs` (trait additions), `pipeline.rs` (`stop_and_take`), `control.rs` and `control_tick.rs` (switch, loss, idle release). New dependency: `wasapi` under `[target.'cfg(windows)'.dependencies]`.
- `nemo-app`: `config.rs` saves `output_device` as `{ id, name }` (old plain-name files still load: matched by name once, then upgraded); builds `DeviceSinkFactory`.
- `nemo-ui`: the output screen. It uses `PlayerCommand` and `PlayerEvent` only (separate UI task).
- Other crates: no change.

## Data Flow

- Threads: control (owns the pipeline), feeder, audio callback or, in exclusive mode, the `nemo-wasapi` render thread, and the `nemo-devices` watcher. The watcher sends `PlayerEvent::OutputDevicesChanged` through the player's existing event path.
- **Shared mode:** unchanged. The `cpal` callback pulls from the `rtrb` ring. Buffers belong to the ring; the sink only borrows the device buffer.
- **Exclusive mode:** the `nemo-wasapi` thread waits on the WASAPI event, asks for the free frames, calls the same `RenderCallback` on a preallocated `f32` scratch (fixed chunks, like `render_converted`), and packs the result into a preallocated byte buffer with `pack.rs`. Packing is the exact inverse of the decoder's integer-to-`f32` scaling, with rounding and clamping, no dither. All buffers are sized when the sink opens.
- **Negotiation** (on `open`, off the audio thread): ask the device for the song's rate and channel count with these sample formats, in order. If the source has a known depth `d`: `d`-bit packed, `d` valid bits in a 32-bit container, then 32-bit integer, then `f32`. If the depth is unknown (lossy source): 24 in 32, 32-bit integer, 24 packed, 16, `f32`. If no format fits at the song's rate, try the device's other rates, nearest first, with integer multiples before others; the existing `Chain` resamples and the signal path shows it (criterion 15). If nothing fits, or the device is busy, the factory opens shared mode and returns `FallbackReason`.
- **Switch during playback** (criterion 4): open and test the new sink first. If that fails, keep playing and emit `OutputOpenFailed`; the saved choice stays. If it works, control reads the position (`played`, `start`, `base`, minus latency), calls `Pipeline::stop_and_take()`, which joins the feeder and returns its active and queued `Job`s with their open decoders, then starts a new pipeline and seeks the carried decoder to the position. No network re-resolve happens, so the gap is the device open plus one seek, about 250 ms. The 200 ms prefill is the largest part. Queue and `Run` state are untouched.
- **Loss** (criterion 9): the `cpal` error callback or the WASAPI wait returns an error and sets `SinkHealth::Lost` (an atomic). The control tick (5 ms) reads it through `AudioSink::health()`, pauses, and emits `DeviceLost { name }`; the ring and decoder stay. No device is chosen for the user. When the watcher sees the same `DeviceId` again it emits `SavedDeviceReturned`; the UI offers to resume, and `PlayerCommand::ResumeOnReturnedDevice` does it. A setting `auto_resume` exists but is off and not shown until the owner asks for it.
- **Start with a missing device** (criterion 8): play on the default, snapshot shows `saved_missing`. When it appears: if the player is stopped or paused, switch at once; if it is playing, emit `SavedDeviceReturned` and let the user choose (a surprise switch mid-song is worse than a late one; the owner may prefer otherwise).
- **No device chosen** (criterion 3): the watcher compares the default `DeviceId`; a change triggers the same switch.
- **Idle release** (criterion 20): control starts a timer on pause or stop with an exclusive sink. At the delay (default 5 s) it runs `stop_and_take()` and keeps the jobs; `Resume` or `Play` reopens through the same path. Shared mode is never released.
- **Taken by another app:** the render thread gets `AUDCLNT_E_DEVICE_INVALIDATED` or `E_RESOURCES_INVALIDATED`, sets `SinkHealth::Taken`, and control pauses and emits `OutputTaken`. `PlayerCommand::RetryOutput` reopens.
- **Volume** (criterion 19): in exclusive mode control keeps the stored volume but forces the gain to unity, so no `Gain` stage exists and the path can be bit-perfect. The snapshot says the volume is not applied.
- Format change between songs in exclusive mode: `can_follow_source()` returns true, and the existing path waits for the audible end, then reopens (criterion 18). A transition from the audio-effects design is skipped in that case.

## Interfaces

```rust
// nemo-core/src/output.rs (new)
pub struct DeviceId(pub String);                       // cpal DeviceId, Display form
pub struct OutputDevice { pub id: DeviceId, pub name: String, pub is_default: bool }
pub struct SavedDevice { pub id: DeviceId, pub name: String }
pub struct OutputOptions { pub exclusive: bool, pub idle_release_secs: u8 /* 1..=60, default 5 */, pub exclusive_buffer_ms: u8 /* 5..=100, default 20 */, pub auto_resume: bool }
pub enum FallbackReason { DeviceBusy, DriverRefused, ExclusiveControlDisabled, FormatNotAccepted, NotSupportedHere }
pub struct OutputStatus { pub devices: Vec<OutputDevice>, pub chosen: Option<SavedDevice>, pub saved_missing: bool, pub active: Option<OutputDevice>,
                          pub exclusive_available: bool, pub options: OutputOptions, pub fallback: Option<FallbackReason>, pub lost: Option<String> }
// PlayerCommand additions
ListOutputDevices, SetOutputDevice(Option<SavedDevice>), SetOutputOptions(OutputOptions), RetryOutput, ResumeOnReturnedDevice,
// PlayerEvent additions
OutputDevicesChanged(Vec<OutputDevice>), OutputChanged(OutputStatus), OutputOpenFailed { reason: String },
DeviceLost { name: String }, SavedDeviceReturned, OutputTaken, ExclusiveFellBack(FallbackReason),
// PlayerSnapshot addition: pub output: OutputStatus

// nemo-audio
pub enum SinkHealth { Ok, Lost, Taken }
pub trait AudioSink { /* existing */ fn health(&self) -> SinkHealth; /* default: Ok */ }
pub struct OpenedSink { pub sink: Box<dyn AudioSink>, pub fallback: Option<FallbackReason> }
pub trait SinkFactory: Send + Sync { /* existing */
    fn open_reported(&self, source: AudioFormat) -> Result<OpenedSink, SinkError>;   // default: wraps `open`
    fn select(&self, selection: OutputSelection);                                    // default: ignore
    fn list_devices(&self) -> Result<Vec<OutputDevice>, SinkError>;                  // default: empty
}
pub struct OutputSelection { pub device: Option<DeviceId>, pub options: OutputOptions }
pub struct DeviceSinkFactory;                           // owns the selection; opens exclusive or shared
#[cfg(windows)] pub struct WasapiExclusiveSink;         // AudioSink; open(device, source, opts) -> Result<Self, ExclusiveError>
impl Pipeline { pub fn stop_and_take(self) -> Vec<Job>; }
pub fn pack_f32(samples: &[f32], out: &mut [u8], layout: PackLayout);   // pure, RT-safe, bit-exact tested
pub enum PackLayout { I16, I24Packed, I24In32, I32, F32 }
```

`SinkError` gains `DeviceBusy`, `ExclusiveRefused(String)` and `FormatNotAccepted`. The byte source for local and cached files is not needed here: the switch reuses the open decoder.

## Error Handling

- `ExclusiveError` (`thiserror`): `Busy`, `Refused`, `ControlDisabled`, `NoFormat`, `Backend(String)`. Each maps to a `FallbackReason`, and each `FallbackReason` has one plain sentence in `nemo-ui` copy.
- Open failure on a switch is `OutputOpenFailed` and changes nothing else (criterion 5).
- Device names and IDs are never logged at info level or above (criterion 26); `tracing::debug` may carry them. `Debug` for `DeviceId` prints a hash.
- No `unwrap`. A poisoned or failed watcher thread ends the list updates, and `OutputDevicesChanged` stops; the picker offers a manual refresh through `ListOutputDevices`.
- Underruns in exclusive mode surface as `UnderrunsCounted`; the UI offers a larger `exclusive_buffer_ms` after repeated ones (criterion 23).

## Testing Strategy

- `cargo test -p nemo-audio --test bit_exact`: new cases pass 16-bit and 24-bit WAV through `pack_f32` (I16, I24Packed, I24In32) and compare the bytes with the source integers. A fake exclusive backend (a trait the render thread uses, implemented by `wasapi` on Windows and by a memory buffer in tests) runs the whole thread loop on every OS.
- `tests/rt_no_alloc.rs`: the exclusive render loop (`event wait` replaced by the fake) and `pack_f32` allocate nothing over ten seconds, normal and underrun.
- Unit tests: negotiation order for 16, 24, 32 and unknown depth; nearest-rate choice; `FallbackReason` mapping; the `cpal` ID to endpoint-ID match (`#[ignore]`, Windows, needs a device).
- Control tests with fake sinks (`tests/common/fakes.rs`): switch keeps position and queue; failed open keeps the old device and the saved choice; loss pauses within one tick; return emits the offer and does not resume; idle release at 5 s and reopen on resume; no gain stage in exclusive mode.
- Manual on the reference hardware (open owner question 4): a USB DAC in exclusive mode at 44.1, 96 and 192 kHz; unplug during playback; another app taking the device; the Windows "allow exclusive control" setting off.

## Risks and Alternatives

- **The `cpal` ID and the WASAPI endpoint ID may differ in form.** If so, `DeviceSinkFactory` keeps a map built from `wasapi`'s own enumeration. This is the first thing the Windows test checks.
- **Exclusive buffers.** Event-driven exclusive mode needs a buffer aligned to the hardware period. A 20 ms default can be below a device's minimum period; the sink then uses the minimum and reports it. Some drivers refuse 24-in-32 until 24 packed is tried, which the ladder handles. Real behavior is unverified until the hardware test.
- **Switch gap.** About 250 ms is an estimate. A stream seek is one range request and may exceed 500 ms on a slow link; criterion 4 is then missed for streams, and the event says nothing. Measure on the reference machine.
- Alternative: use `wasapi` for all Windows output. Rejected: it duplicates the shared path that works and drops `cpal` ID parity with Linux.
- Alternative: a 3rd identifier, the device name plus index. Rejected: indexes change when devices are plugged.
- Alternative: `AudioSink::stop` hands back the ring consumer so the same pipeline moves to the new sink. Rejected: the `cpal` closure owns it; carrying the decoder is simpler and keeps the ring discipline.
- ASIO stays out (`tech.md`, optional feature). Linux exclusive (`hw:`) is a later spec; Linux shows shared mode only (criterion 27).

## Steering and README changes

- `tech.md`: add row `wasapi` =0.27.0, MIT, MSRV 1.85 (edition 2024), released 2026-11-06, Windows only; builds on the `windows` 0.62.2 bindings already in `Cargo.lock`; no C toolchain expected (unverified). Change the "Exclusive mode" status from "unverified" to "verified: `cpal` 0.18.2 is shared-only". Note `wasapi` needs `windows` `^0.62.2`; pin both in `[workspace.dependencies]`.
- `structure.md`: no new crate or edge. Add `output.rs` to the `nemo-core` ownership line and the device factory to `nemo-audio`.
- README egress list: no new host (criterion 26).
- ADR 0004: add the output types to the expected `nemo-core` additions.

<!-- Sources:
crates/nemo-audio/src/{cpal_sink.rs (list_output_devices, CpalSink::open_named, CpalSinkFactory), sink.rs (AudioSink, SinkFactory, SinkError, can_follow_source), pipeline.rs (Pipeline, make_callback, PipeShared), feeder.rs, control.rs (TICK 5 ms, ensure_pipeline), control_tick.rs}; crates/nemo-core/src/{audio.rs (OutputMode, is_bit_perfect), player.rs}; crates/nemo-app/src/config.rs (output_device); Cargo.lock (windows 0.62.2)
.kiro/steering/{tech.md, code.md, structure.md}; docs/adr/0004-feature-wave-crates.md
https://crates.io/crates/wasapi (0.27.0, MIT, rust-version 1.85, 2026-11-06; deps windows ^0.62.2, thiserror, log, num-integer)
https://crates.io/crates/cpal (0.18.2, Apache-2.0, rust-version 1.85)
https://raw.githubusercontent.com/RustAudio/cpal/master/src/host/wasapi/device.rs (shared mode only)
https://raw.githubusercontent.com/HEnquist/wasapi-rs/master/README.md (exclusive mode, probing, notifications)
https://docs.rs/cpal/0.18.2/cpal/struct.DeviceId.html (DeviceId exists in 0.18.2)
-->
