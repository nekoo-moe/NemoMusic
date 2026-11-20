# Device switch fix: design

## Overview

Keep the design of `output-devices` (carry the decoder, seek it to the played position). Fix the order of the two steps and the rounding of the seek.

## Affected Crates

- `nemo-audio` (control and pipeline code, and tests). No other crate.

## Data Flow

Control thread: open the new sink (unchanged) -> `Pipeline::halt_output()` of the old pipeline (stops the sink; the callback no longer runs, so `played` is final) -> `stop_and_take_with_events()` (stop flag, join the feeder, collect the jobs and the feeder reports nobody has read) -> apply the unread `Started` reports, so the position is known -> read the position -> seek the first job to the position on a frame boundary -> `Pipeline::start` with the new sink -> load the jobs. The other unread reports (`Finished`, `Skipped`, `Failed`) were lost with the old pipeline before and still are; their frame numbers count the old pipeline. The feeder thread keeps decoding into the old ring for the few milliseconds between the two steps; that audio is never played and is read again from the decoder after the seek.

The callback code (`make_callback`, `render`, `apply_gain`) is not touched. `Pipeline::halt_output` runs on the control thread and calls `AudioSink::stop`, which every sink already implements and tolerates being called twice (`stop_and_take` calls it again).

Seek time: `pos` is rounded to the nearest source frame and turned into the earliest time that lands on that frame (`frame_time`, already used for CUE spans). Every switch rounds again to the nearest frame, so the microsecond cut of the stored base does not add up over repeated switches.

Song end: the position belongs to the current song. WHEN the current song has been fully decoded (`Run::finished` is set), the first carried job is the following song, not the current one. The test showed the player stuck, so the fix is the smallest change: the current song is not among the carried jobs (compared by token), so it is opened again at the position through `start_track`, the way `seek` already does for a finished song (a pause is kept, as in `switch_audio_track`). That resolves the song again, so the gap here can pass 500 ms on a slow link; it affects the last second of a song only.

## Interfaces

```rust
impl Pipeline {
    /// Stop the output device; the callback no longer runs afterwards.
    pub fn halt_output(&mut self);
    /// `stop_and_take`, and the feeder reports nobody has read.
    pub fn stop_and_take_with_events(self) -> (Vec<Job>, Vec<FeedEvent>);
}
```

`feeder.rs` gains `pub(crate) fn snap_to_frame(pos: Duration, rate: u32) -> Duration`.

## Error Handling

Unchanged: a failed open of the new sink leaves the old pipeline running (the old output is halted only after the new sink opened, and only when the new pipeline can start; if `Pipeline::start` fails after the halt, the jobs are lost as today and `OutputOpenFailed` is sent).

## Testing Strategy

- `tests/player_switch_frames.rs` replaces `switch_device_no_click_step`: a stereo 24-bit counter track, the capture of the devices read back as runs of frame numbers, assert that each join continues at the next frame. The silence is bounded to 500 ms only on a clock that runs at real time (a fast test clock turns a few milliseconds of start-up into hundreds of milliseconds of audio). Cases: a real-time clock, an odd block size (441 frames), a switch right after the start, a fast old device, a switch in the last second (song end and next song from its start), and the same while paused.
- `biquad/tests.rs`: the three inputs of the CI logs as a fixed test; the property test gets a fixed seed, so every run sees the same 256 inputs. Hunting for more means a random seed and `PROPTEST_CASES=...` by hand.
- `cargo test -p nemo-audio --test bit_exact` and `--test rt_no_alloc` in every PR. 200-run loops in the PR text.

## Risks and Alternatives

- A real device may play out its buffer after `stop()`. Then a few milliseconds would be heard twice. Not measurable here; the alternative (fade) is out of scope.
- Carrying the unplayed ring to the new sink would be exact for every case, but needs the callback to give back its consumer (rejected before, the same reasons hold).
- Auto preamp: a denser grid in Hz would also catch the peaks but costs 10 times the probes on the control thread. A grid in the warped frequency `tan(w/2)` follows the width of the features and needs fewer points.
