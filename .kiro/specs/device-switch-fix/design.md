# Device switch fix: design

## Overview

Keep the design of `output-devices` (carry the decoder, seek it to the played position). Fix the order of the two steps and the rounding of the seek.

## Affected Crates

- `nemo-audio` (control and pipeline code, and tests). No other crate.

## Data Flow

Control thread: open the new sink (unchanged) -> `Pipeline::halt_output()` of the old pipeline (stops the sink; the callback no longer runs, so `played` is final) -> read the position -> `stop_and_take()` (stop flag, join the feeder, collect the jobs) -> seek the first job to the position on a frame boundary -> `Pipeline::start` with the new sink -> load the jobs. The feeder thread keeps decoding into the old ring for the few milliseconds between the two steps; that audio is never played and is read again from the decoder after the seek.

The callback code (`make_callback`, `render`, `apply_gain`) is not touched. `Pipeline::halt_output` runs on the control thread and calls `AudioSink::stop`, which every sink already implements and tolerates being called twice (`stop_and_take` calls it again).

Seek time: `pos` is rounded to the nearest source frame and turned into the earliest time that lands on that frame (`frame_time`, already used for CUE spans). The stored base position is rounded to a microsecond, not cut, so repeated switches do not drift.

Song end: the position belongs to the current song. WHEN the current song has been fully decoded (`Run::finished` is set), the first carried job is the following song, not the current one. Task 3 decides by test what happens today and fixes it with the smallest change: restart the current song at the position through `start_track`, the way `seek` already does for a finished song.

## Interfaces

```rust
impl Pipeline {
    /// Stop the output device; the callback no longer runs afterwards.
    pub fn halt_output(&mut self);
}
```

`feeder.rs` gains `pub(crate) fn snap_to_frame(pos: Duration, rate: u32) -> Duration`.

## Error Handling

Unchanged: a failed open of the new sink leaves the old pipeline running (the old output is halted only after the new sink opened, and only when the new pipeline can start; if `Pipeline::start` fails after the halt, the jobs are lost as today and `OutputOpenFailed` is sent).

## Testing Strategy

- `tests/player_switch.rs`: `switch_device_no_click_step` becomes `switch_device_repeats_and_skips_no_frame`: a stereo 24-bit counter track, the capture of each device read back as frame numbers, assert strictly consecutive numbers across the switch and a silence of at most 500 ms. An unaligned device block (441 frames) covers the rounding. The old amplitude-step check stays as a second assertion on a tone with the same rig.
- `biquad/tests.rs`: the five inputs as fixed cases; the property test gets a fixed seed (the number of cases stays 256 and can be raised with `PROPTEST_CASES` when hunting).
- `cargo test -p nemo-audio --test bit_exact` and `--test rt_no_alloc` in every PR. 200-run loops in the PR text.

## Risks and Alternatives

- A real device may play out its buffer after `stop()`. Then a few milliseconds would be heard twice. Not measurable here; the alternative (fade) is out of scope.
- Carrying the unplayed ring to the new sink would be exact for every case, but needs the callback to give back its consumer (rejected before, the same reasons hold).
- Auto preamp: a denser grid in Hz would also catch the peaks but costs 10 times the probes on the control thread. A grid in the warped frequency `tan(w/2)` follows the width of the features and needs fewer points.
