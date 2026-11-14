## Overview

Effects and crossfade live in `nemo-audio` and add no crate. Two places change samples, and both are off by default.

- The audio callback (`make_callback` in `pipeline.rs`) gets an `EffectsChain` between the ring read and `apply_gain`: equalizer, delay, reverb, then a clamp that counts clipping. One `bool` guards the whole chain, so with every effect off the callback does no extra arithmetic and `bit_exact` still passes.
- The feeder thread (`feeder.rs`) gets a `TransitionMixer`. It holds the last L seconds of the playing song in a tail window, and mixes that tail with the head of the next song before anything reaches the ring. Crossfade is therefore baked into ring samples and costs the callback nothing.

Processing is `f32` (the whole pipeline already is). EQ state is `f64` so the 31 Hz band stays stable at 192 kHz. Preamp is automatic: the control thread evaluates the combined EQ response, lowers the preamp by its peak boost, and reports it. There is no limiter in this release; samples past full scale are clamped, counted, and shown as a warning (criterion 7). A limiter adds latency and a second thing to test, and automatic preamp covers the EQ case.

AutoEq files are plain text (`Preamp: -6.2 dB`, `Filter 1: ON PK Fc 105 Hz Gain 3.0 dB Q 0.70`). One parser reads both the ten-band and the parametric files. The equalizer holds up to 12 biquads of kind peak, low shelf or high shelf; the ten-band equalizer is ten peaks at the ISO octave centres with Q 1.41. A file with other filter types (low-pass, high-pass, notch) or more than 12 filters is refused with the reason (criterion 11). No data is bundled and no host is contacted.

## Affected Crates

- `nemo-core`: new `effects.rs` (settings and status types, `GaplessPolicy`), new `PlayerCommand` and `PlayerEvent` values, new `ProcessingStage` values, `PlayerSnapshot` fields. No I/O.
- `nemo-audio`: new `effects/` module (`chain`, `biquad`, `delay`, `reverb`, `fade`, `profile` parser, `presets` store), new `transition.rs` (`TransitionMixer`), changes to `pipeline.rs`, `feeder.rs`, `control.rs`, `control_tick.rs`.
- `nemo-app`: loads and saves `EffectsSettings` and `TransitionSettings` in `settings.json`; injects a `GaplessPolicy`.
- `nemo-ui`: the effects screen and the signal-path lines. It sends `PlayerCommand` only (the UI task is separate).
- `nemo-library`: implements `GaplessPolicy` (marks an album as gapless). Needed from the local-library design; it also owns the file `ByteSource` of ADR 0004, which this design does not need beyond what `FileSource` does today.

## Data Flow

1. The UI sends `PlayerCommand::SetEffects(EffectsSettings)`. The control thread validates and clamps it, computes biquad coefficients and the auto preamp, and pushes one `Copy` `EffectsUpdate` into a dedicated `rtrb` ring (capacity 8). Nothing is allocated or locked on the audio side. The control thread then sends `PlayerEvent::EffectsChanged(EffectsStatus)`.
2. The callback pops updates at the start of each block. It never allocates: all delay lines, reverb combs and filter state are sized when the pipeline starts (delay up to 2 s, reverb fixed tables at the device rate, `channels` up to 8) and only cleared on reset.
3. Switching an effect on or off ramps a wet gain over 10 ms; coefficient changes ramp linearly over 10 ms. Both finish well inside the 100 ms of criterion 2. When an effect is fully off its stage is skipped.
4. Tails (criterion 15): when the ring is idle the callback today writes silence. With delay or reverb active it instead runs the chain on zeros until `tail_frames` (computed from feedback and size, capped at 10 s) have passed. `Stop` fades out over 150 ms, keeps the pipeline open for the tail, then drops it. Pause freezes state and does not ring out.
5. Crossfade. Control hands the next song to the feeder as today (`FeedCmd::Load`). The prefetch starts when the current song loads (`prefetch_next`), so the next song is normally ready minutes early. **Lead time (settled here):** the transition starts when the feeder's decoder reaches the end of the playing song, which is about L + 1 s (ring) before the audible end. Control therefore attaches a `TransitionPlan` to the `Job` only if the next song is `Ready` and opened by then; the target is L + 5 s before the audible end. Otherwise the songs join gaplessly and the event says why (criterion 20). L is capped at one third of the shorter song.
6. The feeder keeps a tail window of L frames of the playing song. At decoder end it scans the window for trailing silence (below -60 dBFS), scans the first 10 s of the next song for leading silence, and picks per pair: `Crossfade` (equal-power, length L), `TrimSilence` (drop the silences, then a 300 ms overlap), or `Gapless` (no overlap). `Gapless` is chosen when `GaplessPolicy::join_without_fade` is true (criterion 18) or when both ends are already within 50 ms of silence. The plain mode skips the scan and always crossfades.
7. Both songs go through their own `Chain` to the output format first, so differing rates or channel counts still work (criterion 22). `Started` for the incoming song is sent at the middle of the overlap with `base` = L/2, so the player bar and position follow the louder song (criterion 21).
8. Manual skip (criterion 17): control sets `shared.fade_ms` (0 to 1000) before the flush. The callback ramps the gain to zero over that time, then discards the ring and acks. `FLUSH_TIMEOUT` becomes fade + 500 ms. The new song's first frames fade in over the same time in the feeder.

Threads are unchanged: control, feeder, decode `prepare` workers, audio callback. The ring stays the only audio handoff; the tail window and the next song's head buffer belong to the feeder thread.

## Interfaces

```rust
// nemo-core/src/effects.rs (new). Integer units keep PlayerCommand `Eq`.
pub const EQ_MAX_FILTERS: usize = 12;
pub enum FilterKind { Peak, LowShelf, HighShelf }
pub struct EqFilter { pub kind: FilterKind, pub freq_hz: u16, pub gain_tenth_db: i16, pub q_hundredths: u16 }
pub struct EqSettings { pub enabled: bool, pub preamp_tenth_db: i16, pub filters: [Option<EqFilter>; EQ_MAX_FILTERS] }
pub struct DelaySettings { pub enabled: bool, pub time_ms: u16, pub feedback_pct: u8, pub mix_pct: u8 }   // time 1..=2000, feedback <= 85
pub struct ReverbSettings { pub enabled: bool, pub size_pct: u8, pub damping_pct: u8, pub mix_pct: u8 }
pub struct EffectsSettings { pub equalizer: EqSettings, pub delay: DelaySettings, pub reverb: ReverbSettings } // all `enabled: false` by Default
pub enum PreampMode { Manual, Auto { lowered_by_tenth_db: u16 } }
pub struct EffectsStatus { pub settings: EffectsSettings, pub preamp: PreampMode, pub clipped_samples: u64 }
pub enum TransitionMode { Off, Crossfade, Smart }
pub enum SkipFade { Off, Short }  // Short = 200 ms
pub struct TransitionSettings { pub mode: TransitionMode, pub seconds: u8 /* 1..=12 */, pub skip_fade: SkipFade, pub honour_gapless_marks: bool }
pub enum TransitionKind { Crossfade, TrimSilence, Gapless }
pub enum TransitionSkipReason { NextNotReady, FormatChangeInExclusive, TooShort, Analysis }
pub trait GaplessPolicy: Send + Sync { fn join_without_fade(&self, from: &Track, to: &Track) -> bool; }

// PlayerCommand additions
SetEffects(EffectsSettings), SetTransition(TransitionSettings),
// PlayerEvent additions
EffectsChanged(EffectsStatus),
TransitionStarted { kind: TransitionKind, seconds: u8 },
TransitionSkipped(TransitionSkipReason),
// PlayerSnapshot additions: pub effects: EffectsStatus, pub transition: TransitionSettings
// ProcessingStage: replace the unit `Equalizer` (used in nemo-ui/src/signal.rs) with
Equalizer { preamp_tenth_db: i16, auto_preamp: bool },
Delay { time_ms: u16 }, Reverb,
Crossfade { seconds: u8, running: Option<TransitionKind> },   // listed while the mode is on
Clipped { samples: u64 },                                      // listed only when > 0
```

`ProcessingStage` stays `Copy`; names such as "preset Flat" come from the UI's own preset state, not the stage.

```rust
// nemo-audio (public)
pub mod effects {
    pub struct EffectsChain;                       // RT: new(channels, rate, rx), process(&mut [f32]), process_tail(&mut [f32]) -> bool
    pub struct EqProfile { pub name: String, pub preamp_tenth_db: i16, pub filters: Vec<EqFilter> }
    pub fn parse_autoeq(name: &str, text: &str) -> Result<EqProfile, ProfileError>;
    pub fn write_autoeq(profile: &EqProfile) -> String;       // user preset export
    pub fn auto_preamp(settings: &EqSettings, rate: u32) -> u16;  // peak boost, tenth dB
    pub struct PresetStore;                        // list, load, save, rename, delete in a folder; off the RT path
}
pub struct TransitionMixer;                        // feeder only: push(block), at_end(next) -> TransitionKind, mix_into(&mut Vec<f32>)
pub struct TransitionPlan { pub kind_hint: TransitionMode, pub frames: u64 }
// pipeline.rs: Job gains `pub transition: Option<TransitionPlan>`; PipeShared gains `fade_ms: AtomicU32`, `clipped: AtomicU64`
```

Built-in presets (Flat, Bass boost, Treble boost, Vocal) are constants in `effects::presets`. User presets and imports are AutoEq-format text files, so one format covers all of them.

## Error Handling

- `ProfileError` (`thiserror`): `Empty`, `UnsupportedFilter { line, kind }`, `TooManyFilters`, `OutOfRange { line }`, `Syntax { line }`. The UI shows the text; nothing is applied on error (criterion 11).
- `PresetError`: `Io(std::io::Error)`, `NameTaken`, `Profile(ProfileError)`. Files are read with a 64 KiB cap.
- Out-of-range settings are clamped by the control thread, never by the callback; the clamped value is what `EffectsChanged` returns.
- `TransitionSkipped` is an event, not a failure: playback continues gaplessly.
- A full update ring drops the older update; the next `SetEffects` carries the whole state, so none is lost for good.
- Non-finite samples (NaN from a bad filter) are replaced by 0 and counted as clipped; the chain resets its state.

## Testing Strategy

- `cargo test -p nemo-audio --test bit_exact`: unchanged, runs with every effect and the transition mode off, plus a new case that turns an effect on and off and checks the output is identical again once the ramp ends.
- `tests/rt_no_alloc.rs`: new cases run `EffectsChain::process` and `process_tail` with EQ (12 filters), delay and reverb on, during a ramp and after, under the counting allocator, and assert zero allocations.
- Unit tests: biquad response against analytic gain at band centres; `auto_preamp` never lets a swept sine exceed 0 dBFS; AutoEq parser on the ten-band and parametric samples from the requirements sources and on refused files; delay with feedback 85% decays; reverb tail ends inside `tail_frames`.
- Feeder tests with fake decoders: equal-power overlap sums to constant power; `Started` carries `base` = L/2; `NextNotReady` joins gaplessly with no lost or doubled frame; rate-differing songs crossfade and report `Resample`; silence trim removes only silence.
- Callback cost: a `#[ignore]` benchmark asserts the chain stays under 25% of a 5 ms period on the reference machine (criterion 24).
- Manual: listen for clicks while toggling each effect at 100 ms intervals.

## Risks and Alternatives

- **Click on parameter change.** Ramps hide most of it. Not proven by ear until the manual test runs; the 10 ms figure may need to grow.
- **Tail window adds up to L of decode work at the start of a song.** Decoding runs faster than real time, but a slow stream could underrun the first song. Mitigation: the window only fills once the ring is primed; if it is not full at decoder end the transition shrinks to what is there.
- **Exclusive mode and crossfade.** A device open in exclusive mode keeps one format, so a next song in another format cannot overlap. The design reopens the device without overlap and emits `TransitionSkipped(FormatChangeInExclusive)`. This departs from criterion 22 in that one case; the owner should confirm.
- **AutoEq files with unknown filter types are refused**, not approximated. Approximating needs a design of its own and would change the sound the user chose.
- Alternative: a limiter instead of automatic preamp. Rejected for this release (latency, a second state machine). Alternative: effects in the feeder. Rejected, because toggling would lag by the 1 s ring and miss criterion 2.
- Alternative: the `biquad` crate. Rejected; 60 lines of RBJ coefficients are cheaper than a dependency and no new crate is needed. Smart-transition trimming is simple silence detection, not Apple's AutoMix, which Apple does not document.

## Steering and README changes

- `tech.md`: no new crate. Add a row "Effects: own biquad, delay and reverb code in `nemo-audio`, f32 with f64 filter state". Update the Equalizer line if tech.md names a crate.
- `structure.md`: no new crate or edge. Add `GaplessPolicy` and the effects types to the `nemo-core` ownership line; add effects and crossfade to `nemo-audio`.
- README egress list: no new host (the feature contacts none). Add the AutoEq credit to the about screen text and the licence file.
- ADR 0004: add `GaplessPolicy` and the effects types to the expected `nemo-core` additions.

<!-- Sources:
crates/nemo-audio/src/{pipeline.rs (make_callback, PipeShared, Job), feeder.rs (Feeder, Active, push_converted, FeedEvent::Started), render.rs (apply_gain, UNITY_GAIN_BITS), chain.rs, control.rs, control_tick.rs (try_load_following, promote_following), tests/rt_no_alloc.rs, tests/bit_exact.rs}; crates/nemo-core/src/{audio.rs (ProcessingStage), player.rs}; crates/nemo-ui/src/signal.rs
docs/adr/0004-feature-wave-crates.md; .kiro/steering/{code.md, tech.md, structure.md}
https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results/oratory1990/over-ear/1MORE%20SonoFlow/README.md (file syntax, Q 1.41)
https://support.apple.com/en-us/105067 (AutoMix, Crossfade)
-->
