## Tasks

- [x] 1. Add the effects settings types and `GaplessPolicy` to `nemo-core`
  - Crate and files: `nemo-core`: `src/effects.rs` (new), `src/lib.rs` (one `pub mod` line). Touches nemo-core/src/effects.rs and nemo-core/src/lib.rs.
  - Does: Adds every type of ADR 0004 Decision 4 row 2 (`EffectsSettings`, `EqSettings`, `EqFilter`, `FilterKind`, `DelaySettings`, `ReverbSettings`, `PreampMode`, `EffectsStatus`, `TransitionMode`, `SkipFade`, `TransitionSettings`, `TransitionKind`, `TransitionSkipReason`, `EQ_MAX_FILTERS`) and the `GaplessPolicy` trait (row 4), signatures as in the design. `Default` has every `enabled` false and `TransitionMode::Off`; integer units keep `PlayerCommand` `Eq`. No I/O.
  - _Requirements: 1, 5, 16_
  - Depends on: none
  - Wave: 0
  - Checks: unit tests that `Default` turns every effect and the transition off, and that the documented ranges (gain -12 to +12 dB, delay 1 to 2000 ms, feedback at most 85%, crossfade 1 to 12 s) are the constants the clamp code will use.
  - Steering: `structure.md`, `nemo-core` ownership line: add the effects types and `GaplessPolicy`. ADR 0004: no edit (Decision 4 rows 1 to 4 already list them).

- [x] 2. Add the new `ProcessingStage` values
  - Crate and files: `nemo-core`: `src/audio.rs`; `nemo-ui`: `src/signal.rs` (match arms only). Touches nemo-core/src/audio.rs (soundcloud's `Origin::SoundCloud` task also edits it; one at a time).
  - Does: Replaces the unit `Equalizer` with `Equalizer { preamp_tenth_db, auto_preamp }` and adds `Delay { time_ms }`, `Reverb`, `Crossfade { seconds, running }`, `Clipped { samples }` and `ReplayGain { db }` (row 3; `Gain` stays the volume stage). `ProcessingStage` stays `Copy`. `nemo-ui` gets plain labels so the build stays green; the real lines come in task 22.
  - _Requirements: 4_
  - Depends on: audio-effects#1 (the `Crossfade` stage holds a `TransitionKind`, which that task adds in `effects.rs`)
  - Wave: 0
  - Checks: `is_bit_perfect` is false with each new stage in the path (unit test in `audio.rs`).

- [x] 3. Add the effects and transition commands, events and snapshot fields
  - Crate and files: `nemo-core`: `src/player.rs`; `nemo-audio`: `src/control_cmd.rs`; exhaustive matches in `nemo-ui` and `nemo-app`. Touches nemo-core/src/player.rs.
  - Does: Adds `PlayerCommand::{SetEffects, SetTransition}`, `PlayerEvent::{EffectsChanged, TransitionStarted, TransitionSkipped}` and `PlayerSnapshot.{effects, transition}` (row 1). `control_cmd.rs` stores the settings and answers with `EffectsChanged`; nothing reaches the audio path yet, and no UI sends the commands until task 19.
  - _Requirements: 1_
  - Depends on: audio-effects#1
  - Wave: 0
  - Checks: `cargo test -p nemo-audio --test bit_exact`. Only one task at a time edits `nemo-core/src/player.rs` (ADR 0004): output-devices#3 waits for this one; account-actions (`SwitchAudioTrack`) and cast (`Park`) tasks must not land at the same time.

- [ ] 4. Save the effects and transition settings in `settings.json`
  - Crate and files: `nemo-app`: `src/config.rs`, `src/backend.rs` (send the saved settings to the player at start).
  - Does: Stores `EffectsSettings` and `TransitionSettings` beside the other settings and sends `SetEffects` and `SetTransition` after the player starts. A file without the new fields loads with everything off, and an app update never turns an effect on. Values are clamped by `sanitized()`.
  - _Requirements: 1, 27_
  - Depends on: audio-effects#1, audio-effects#3
  - Wave: 1
  - Checks: round-trip test; an old `settings.json` without the fields gives all-off; out-of-range values are clamped. No network host (criterion 27).

- [x] 5. Parse and write AutoEq profile files
  - Crate and files: `nemo-audio`: `src/effects.rs` (module root, new), `src/effects/profile.rs` (new); fixtures in `crates/nemo-audio/tests/fixtures/autoeq/`.
  - Does: `parse_autoeq(name, text)` reads the fixed-band and the parametric text (`Preamp:` and `Filter N: ON PK|LSC|HSC Fc .. Gain .. Q ..` lines) into `EqProfile`; `write_autoeq` writes the same format for user presets. Other filter types or more than 12 filters return `ProfileError` with the line, and nothing is applied (criterion 11). Pure functions, off the real-time path; no network.
  - _Requirements: 10, 11, 12_
  - Depends on: audio-effects#1
  - Wave: 1
  - Checks: `proptest` for parse/write round trip and for no panic on any input; refused-file cases for every `ProfileError` variant; `cargo test -p nemo-audio --test bit_exact`. Fixtures are written by hand in the AutoEq format and are not copied from AutoEq results (criterion 12; a measurer asked that its data not be published), licence noted in `fixtures/README.md`.
  - Steering: `structure.md`, `nemo-audio` ownership line: add effects (EQ, delay, reverb) and crossfade.

- [ ] 6. Add the built-in presets and the preset store
  - Crate and files: `nemo-audio`: `src/effects/presets.rs` (new).
  - Does: Built-in Flat, Bass boost, Treble boost and Vocal as constants; Flat sets every band and the preamp to 0 dB. `PresetStore` lists, loads, saves, renames, deletes, imports and exports AutoEq-format text files in a folder that `nemo-app` passes in, reads at most 64 KiB per file, and returns `PresetError` (`Io`, `NameTaken`, `Profile`).
  - _Requirements: 8, 9, 12_
  - Depends on: audio-effects#5
  - Wave: 1
  - Checks: unit tests in a temp folder for each store action, `NameTaken`, the 64 KiB cap and Flat at 0 dB; `cargo test -p nemo-audio --test bit_exact`.

- [x] 7. Add the biquad filters and the automatic preamp
  - Crate and files: `nemo-audio`: `src/effects/biquad.rs` (new).
  - Does: RBJ peak, low-shelf and high-shelf coefficients, `f64` state, `f32` samples; the ten-band equalizer is ten peaks at the ISO centres 31 Hz to 16 kHz, Q 1.41. `auto_preamp(settings, rate)` returns the peak boost of the combined response in tenths of a dB. The per-sample `process` is `/// RT-SAFE` (no allocation, lock or logging, code.md).
  - _Requirements: 5, 6_
  - Depends on: audio-effects#1
  - Wave: 2
  - Checks: gain at each band centre matches the analytic value at 44.1, 48, 96 and 192 kHz; the 31 Hz band stays stable at 192 kHz; `proptest`: a swept sine through any settings with the auto preamp never exceeds 0 dBFS; a counting-allocator case for `process` in `cargo test -p nemo-audio --test rt_no_alloc`; `cargo test -p nemo-audio --test bit_exact`.
  - Steering: `tech.md`: add the row "Effects: own biquad, delay and reverb code in `nemo-audio`, `f32` with `f64` filter state" (no new crate).

- [x] 8. Add the delay stage
  - Crate and files: `nemo-audio`: `src/effects/delay.rs` (new).
  - Does: Time, feedback and wet/dry mix; the delay line holds 2 s per channel for up to 8 channels and is sized when the pipeline starts, never in the callback. Feedback is limited to 85% so the echo always decays. Reports `tail_frames` from time and feedback, capped at 10 s.
  - _Requirements: 14, 15_
  - Depends on: audio-effects#1
  - Wave: 2
  - Checks: an impulse with feedback 85% decays below -60 dBFS inside `tail_frames`; counting-allocator case in `cargo test -p nemo-audio --test rt_no_alloc`; `cargo test -p nemo-audio --test bit_exact`.

- [x] 9. Add the reverb stage
  - Crate and files: `nemo-audio`: `src/effects/reverb.rs` (new).
  - Does: Size, damping and wet/dry mix on fixed comb and all-pass tables at the device rate, allocated when the pipeline starts. Size and damping are bounded so the tail cannot grow; `tail_frames` follows size, capped at 10 s.
  - _Requirements: 14, 15_
  - Depends on: audio-effects#1
  - Wave: 2
  - Checks: the tail ends inside `tail_frames` at the largest size; output stays bounded for any settings (`proptest`); counting-allocator case in `cargo test -p nemo-audio --test rt_no_alloc`; `cargo test -p nemo-audio --test bit_exact`.

- [ ] 10. Build the `EffectsChain` with ramps and the clip counter
  - Crate and files: `nemo-audio`: `src/effects/chain.rs`, `src/effects/fade.rs` (new).
  - Does: Runs equalizer, delay, reverb, then a clamp that counts samples past full scale and never wraps; NaN or infinite samples become 0, count as clipped and reset the filter state. Pops one `Copy` `EffectsUpdate` per block from an `rtrb` ring of 8, ramps on/off and coefficient changes over 10 ms, and skips a fully-off stage. One `bool` guards the chain so all-off does no arithmetic. Every function is `/// RT-SAFE`.
  - _Requirements: 2, 3, 7, 23_
  - Depends on: audio-effects#7, audio-effects#8, audio-effects#9
  - Wave: 2
  - Checks: `cargo test -p nemo-audio --test rt_no_alloc` new cases: `process` and `process_tail` with 12 filters, delay and reverb on, during a ramp and after, 10 s of simulated callbacks, zero allocations; a ramp reaches its target inside 100 ms; `cargo test -p nemo-audio --test bit_exact`.

- [ ] 11. Run the chain in the audio callback and apply `SetEffects`
  - Crate and files: `nemo-audio`: `src/pipeline.rs` (`make_callback`, `PipeShared.clipped`), `src/control_cmd.rs`, the signal-path report.
  - Does: Puts the chain between the ring read and `apply_gain`, guarded by its `bool`. The control thread validates and clamps `SetEffects`, computes coefficients and the auto preamp, pushes the update (a full ring drops the older one) and sends `EffectsChanged`; the signal path lists `Equalizer`, `Delay`, `Reverb` and `Clipped` (only when above 0). Every new pipeline, including one built by a device switch (output-devices#9), starts with the current settings.
  - _Requirements: 2, 3, 4, 6, 7, 23_
  - Depends on: audio-effects#2, audio-effects#3, audio-effects#10
  - Wave: 2
  - Checks: `cargo test -p nemo-audio --test bit_exact` with every effect off, plus a new case that turns an effect on and off and checks the output is identical again once the ramp ends; `cargo test -p nemo-audio --test rt_no_alloc` on the whole callback with the chain on; a control test that a toggle reaches the output within 100 ms.

- [ ] 12. Let delay and reverb tails ring out
  - Crate and files: `nemo-audio`: `src/pipeline.rs`, `src/control.rs`, `src/control_tick.rs`.
  - Does: When the ring is idle and delay or reverb is on, the callback runs `process_tail` on zeros until `tail_frames` have passed instead of writing silence. `Stop` fades out over 150 ms and keeps the pipeline open for the tail, then drops it; `Pause` freezes the state and does not ring out.
  - _Requirements: 15_
  - Depends on: audio-effects#11
  - Wave: 2
  - Checks: no sample-to-sample step above the click threshold at a stop or song end; the tail ends inside `tail_frames`; `process_tail` case in `cargo test -p nemo-audio --test rt_no_alloc`; `cargo test -p nemo-audio --test bit_exact` (all off: no tail, no arithmetic).

- [ ] 13. Fade on a manual skip
  - Crate and files: `nemo-audio`: `src/pipeline.rs` (`PipeShared.fade_ms`), `src/control_cmd.rs` (`FLUSH_TIMEOUT`), `src/feeder.rs` (fade-in).
  - Does: Before the flush, control sets `fade_ms` (0 to 1000; `SkipFade::Short` is 200 ms); the callback ramps the gain to zero over that time, then discards the ring and acks. `FLUSH_TIMEOUT` becomes fade + 500 ms, and the feeder fades the new song in over the same time. A skip never waits for the crossfade length.
  - _Requirements: 17_
  - Depends on: audio-effects#3, audio-effects#12
  - Wave: 2
  - Checks: `cargo test -p nemo-audio --test bit_exact` with `SkipFade::Off` (no arithmetic); fade path in `cargo test -p nemo-audio --test rt_no_alloc`; a skip completes within fade + 500 ms (`tests/player_skip.rs`).

- [x] 14. Build the `TransitionMixer`
  - Crate and files: `nemo-audio`: `src/transition.rs` (new).
  - Does: Feeder-only type with a tail window of L frames, allocated when the song starts. At decoder end it scans the tail for trailing silence and the first 10 s of the next song for leading silence (below -60 dBFS) and picks `Crossfade` (equal-power, length L), `TrimSilence` (drop silences, 300 ms overlap) or `Gapless` (both ends within 50 ms of silence, or the policy says so). It never changes tempo or aligns beats.
  - _Requirements: 16, 19_
  - Depends on: audio-effects#1
  - Wave: 2
  - Checks: equal-power overlap sums to constant power; silence trim removes only samples below -60 dBFS; `cargo test -p nemo-audio --test bit_exact`.

- [ ] 15. Mix transitions in the feeder
  - Crate and files: `nemo-audio`: `src/feeder.rs`, `src/pipeline.rs` (`Job.transition: Option<TransitionPlan>`).
  - Does: The feeder fills the tail window once the ring is primed, runs each song through its own `Chain` to the output format first (so differing rates and channel counts still mix), and mixes the tail with the next song's head before the ring. `Started` for the incoming song goes out at the middle of the overlap with `base` = L/2; a window that is not full shrinks the transition. The seam between each song's `Chain` and the mixer is where local-library's ReplayGain stage goes (design, Data Flow 9).
  - _Requirements: 16, 21, 22_
  - Depends on: audio-effects#13, audio-effects#14
  - Wave: 2
  - Checks: feeder tests with fake decoders: no lost or doubled frame, `base` = L/2, songs of different rates crossfade and report `Resample`; `cargo test -p nemo-audio --test bit_exact` with the mode `Off`; `cargo test -p nemo-audio --test rt_no_alloc` (the callback is unchanged and must stay allocation-free).

- [ ] 16. Plan transitions in control and apply `SetTransition`
  - Crate and files: `nemo-audio`: `src/control.rs`, `src/control_tick.rs` (`try_load_following`, `promote_following`), `src/control_cmd.rs`.
  - Does: Attaches a `TransitionPlan` only when the next song is `Ready` and opened L + 5 s before the audible end; otherwise the songs join gaplessly with `TransitionSkipped(NextNotReady)`. L is capped at one third of the shorter song (`TooShort`). Sends `TransitionStarted`, lists `Crossfade` in the signal path while the mode is on, and fills `PlayerSnapshot.transition`.
  - _Requirements: 16, 20, 21_
  - Depends on: audio-effects#15
  - Wave: 2
  - Checks: control tests with fakes for each skip reason and for a plan on a ready song; `cargo test -p nemo-audio --test bit_exact`.

- [ ] 17. Choose the smart transition per song pair and honour gapless marks
  - Crate and files: `nemo-audio`: `src/transition.rs`, `src/control_tick.rs`, `src/engine.rs` (player builder takes `Arc<dyn GaplessPolicy>`); `nemo-app`: `src/backend.rs` (injects the default).
  - Does: In `Smart` mode the mixer's scan picks the kind per pair and `TransitionStarted` reports it; `Crossfade` mode skips the scan. When the scan cannot run (a stream with no lead time) it falls back to the plain crossfade or a gapless join (`Analysis`). `join_without_fade` true means gapless unless `honour_gapless_marks` is off; the default policy `NoGaplessMarks` returns false until local-library's `GaplessPolicy` task (which depends on this one) swaps it in.
  - _Requirements: 18, 19, 20_
  - Depends on: audio-effects#16
  - Wave: 2
  - Checks: tests with a fake `GaplessPolicy` for marked and unmarked pairs and with the exception off; `cargo test -p nemo-audio --test bit_exact`.

- [ ] 18. Skip the crossfade on a format change in exclusive mode
  - Crate and files: `nemo-audio`: `src/control_tick.rs`.
  - Does: When the sink is in exclusive mode and the next song has another rate or channel count, no plan is attached: the device reopens through output-devices' format-change path and `TransitionSkipped(FormatChangeInExclusive)` says why (owner, 2026-11-14). Same-format songs in exclusive mode still crossfade.
  - _Requirements: 22_
  - Depends on: audio-effects#16, output-devices#18
  - Wave: 2
  - Checks: control test with a fake exclusive sink; `cargo test -p nemo-audio --test bit_exact`. Manual: covered in output-devices#22 on the owner's reference hardware.

- [ ] 19. Build the equalizer screen
  - Crate and files: `nemo-ui`: `src/gui/views/effects.rs` (new), `src/gui/views/settings.rs` (entry), `src/copy.rs`.
  - Does: Ten sliders named by frequency for screen readers, the preamp, the auto-preamp line ("lowered by 3.0 dB"), Flat/reset and the built-in presets; sends `SetEffects` only. The curve is drawn without animation under reduced motion. Every control is reachable by keyboard with a visible focus.
  - _Requirements: 5, 6, 9, 25, 26_
  - Depends on: audio-effects#11
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; headless snapshot with reduced motion on and off.

- [ ] 20. Import, export and manage presets and AutoEq profiles in the screen
  - Crate and files: `nemo-ui`: `src/gui/views/effects.rs`; `nemo-app`: `src/ui_bridge.rs` (calls `PresetStore` and `parse_autoeq`).
  - Does: Save, rename, delete, export and import presets; import an AutoEq file the user picks, show the name from the file name and a source note the user may type, and show the `ProfileError` text instead of applying a refused file. `nemo-ui` never depends on `nemo-audio` (structure.md); if the bridge cannot carry this without a new `nemo-core` trait, stop and raise it (ADR 0004 Decision 2).
  - _Requirements: 8, 10, 11, 12_
  - Depends on: audio-effects#6, audio-effects#19; the local-library task that adds `rfd` to `nemo-ui` (its id was not known when this list was written)
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot; bridge tests with a temp folder; no network request (criterion 12).

- [ ] 21. Add the delay, reverb and crossfade controls
  - Crate and files: `nemo-ui`: `src/gui/views/effects.rs`, `src/copy.rs`.
  - Does: Time, feedback and mix for delay; size, damping and mix for reverb, each within its safe limit. Transition mode (Off, Crossfade, Smart), seconds 1 to 12, skip fade (off or short) and the gapless-marks exception. Keyboard reachable, named for screen readers.
  - _Requirements: 14, 16, 17, 18, 19, 25_
  - Depends on: audio-effects#17, audio-effects#19
  - Wave: 3
  - Checks: `design.md` checklist with a screenshot.

- [ ] 22. Show effects and transitions in the signal path
  - Crate and files: `nemo-ui`: `src/signal.rs`, `src/gui/views/bar.rs` (check only).
  - Does: One line per active effect with its settings ("Equalizer, preset Flat, preamp -3 dB"), a clipping warning with the count, the running transition kind and any resampling it needed, and the `TransitionSkipped` reason. While any effect is on the path is never shown as bit-perfect and the file never as true-lossless. The player bar shows the song that `Started` names.
  - _Requirements: 4, 7, 19, 21, 22_
  - Depends on: audio-effects#2, audio-effects#11, audio-effects#16
  - Wave: 3
  - Checks: `signal.rs` unit tests for each stage and for the badge with an effect on; headless snapshot.

- [ ] 23. Credit the AutoEq project
  - Crate and files: `nemo-ui`: `src/copy.rs` (`ABOUT_CREDITS`); `README.md` (Credits); the licence notices shipped with the app.
  - Does: Credits AutoEq for its file format (MIT-licensed code) on the about screen and in the licence notices, and says that profiles come from the user's own files.
  - _Requirements: 13_
  - Depends on: audio-effects#20
  - Wave: 3
  - Checks: none beyond the usual.
  - Steering: README Credits line for AutoEq. No egress line (no new host).

- [ ] 24. Measure the callback cost on the reference machine
  - Crate and files: `nemo-audio`: `benches/effects.rs` (new, `criterion`), an `#[ignore]` budget test.
  - Does: Benchmarks the callback with 12 filters, delay and reverb on, during a ramp and in a tail, and asserts it stays under 25% of a 5 ms period. Documents the worst-case callback time in the benchmark (code.md proof).
  - _Requirements: 24_
  - Depends on: audio-effects#12, audio-effects#15
  - Wave: 3
  - Checks: needs the owner's reference machine (product.md Open decision 7): the `#[ignore]` test and a 1-hour playback with every effect on and no new underrun.

- [ ] 25. Listen for clicks and check transitions by ear
  - Crate and files: none (manual); results go into the design's Risks section.
  - Does: Toggle each effect at 100 ms intervals and move every slider during playback; stop and end songs with tails; skip with each skip fade; play an album with gapless marks and a playlist in each transition mode; play a format change on an exclusive device. If clicks remain, the 10 ms ramp grows in a follow-up task.
  - _Requirements: 2, 15, 17, 18, 19_
  - Depends on: audio-effects#18, audio-effects#21, audio-effects#22
  - Wave: 3
  - Checks: needs the owner (reference machine and headphones).

## Definition of Done

See [section 4 of workflow.md](../../steering/workflow.md#4-definition-of-done). For this spec also:

- `cargo test -p nemo-audio --test bit_exact` passes with every effect and the transition mode off, on every task that changes `nemo-audio`.
- `cargo test -p nemo-audio --test rt_no_alloc` covers every stage that runs in the callback (code.md, Real-time audio thread), and every such function is marked `/// RT-SAFE`.
- A fresh install and an updated install start with every effect and the transition off (criterion 1), and no task adds a network host (criteria 12, 27).
