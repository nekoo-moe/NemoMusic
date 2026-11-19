# Device switch fix: requirements

## Goal

Switching the output device during playback repeats no audio and skips none, and the two audio tests that fail on about every second CI run (`switch_device_no_click_step`, `with_the_auto_preamp_no_steady_tone_exceeds_full_scale`) pass every time because the code they test is right, not because the tests were loosened. Follows `output-devices` criterion 4 (owner approval to touch the audio path: 2026-11-19, "you decide").

## Findings

1. `switch_device_no_click_step` finds a real defect. With a counter track (frame `n` holds the value `n + 1`) the capture of the old device ends at frame 5279 while the new device starts at frame 4320 or 4800: 480 to 960 frames (10 to 20 ms) are played twice. A repeat of that size is a phase jump of a 440 Hz tone, so the step across the boundary was 0.33 to 0.91 (limit 0.1). It fails in about 20 of 100 runs when the four tests of `player_switch.rs` run in parallel and in none of 60 solo runs. In 25 diagnostic runs the gap between the last old and the first new frame number was 0, -480 or -960 and never positive (no frame lost).
2. Cause: `migrate_pipeline` (`control_cmd.rs`) reads the position, then calls `stop_and_take()`, which sets the stop flag, joins the feeder thread (a sleep of up to 2 ms plus the join) and only then stops the sink. The old sink keeps pulling blocks from the ring during that time and `played` keeps growing, so the carried decoder is seeked to a position that is already behind what the old device played. The window is a few milliseconds on a loaded machine, about one callback on a quiet one.
3. A smaller defect in the same code: the seek time is `Duration` of `base + played / rate`, which a decoder rounds down to a frame, so an unaligned position repeats one frame.
4. `effects::biquad::tests::with_the_auto_preamp_no_steady_tone_exceeds_full_scale` finds a real defect in `auto_preamp`, not in the test. The three failing inputs in CI logs of 2026-11-18 (all at 44.1 kHz) leave the response 0.085, 0.036 and 3e-5 dB above 0 dB after the automatic preamp. The probe grid is logarithmic in Hz from 5 Hz and each local peak is refined by a golden-section search. (a) A peak between 0 Hz and 5 Hz is never refined, because the first grid point is never the middle of a triple (a low shelf at 10 Hz, +9.5 dB, Q 0.72 peaks at 0.72 Hz). (b) Near Nyquist the bilinear transform squeezes a resonance by `1 + tan^2(w/2)`, about 12 at 18 kHz, so a log-Hz step of 263 Hz skips a feature that is 50 Hz wide, and two bumps inside one refine window defeat the unimodal search (a low shelf at 16.5 kHz with Q 6.6 and a peak at 18.2 kHz with Q 8.5 reach 9.009 dB where `auto_preamp` stopped at 8.9 dB). The property test draws random settings from a fresh seed on every run, so each run has a small chance to hit such a case.

## Acceptance Criteria

1. WHEN the output device is switched during playback, THE SYSTEM SHALL stop the old device before it reads the position, so that no frame the old device played is played again.
2. WHEN the new device starts, THE SYSTEM SHALL continue at the exact next decoded frame: the decoded frame numbers across the switch have no hole and no repeat, and the silence between the two devices is at most 500 ms. No fade is added.
3. WHEN the switch happens in the last second of a song (the song's end is in the old ring and the decoder is gone), THE SYSTEM SHALL NOT move the next song's decoder to the old song's position and SHALL NOT leave the song undrained. (Confirmed or dropped by the test of task 3; see design.)
4. THE SYSTEM SHALL keep the default path bit-identical and the audio callback free of allocation, locks and logging (`bit_exact`, `rt_no_alloc`).
5. `auto_preamp` SHALL return a preamp so that the combined response of the preamp and all filters is at most 0 dB at every frequency, for every `EqSettings`, within the tolerance of one rounding step of the test (1e-6 dB), at every supported rate.
6. The switch test and the auto-preamp test SHALL give the same result on every run: the switch test reads decoded frame numbers instead of measuring a tone; the property test runs with a fixed seed, and the three failing inputs plus the two other shapes of finding 4 are fixed cases.
7. Each fixed test SHALL pass 200 of 200 runs in a local loop (the loop is not in CI).

## Out of Scope

Real devices (cpal, WASAPI). A fade at the switch. Carrying the old ring to the new sink (rejected in `output-devices/design.md`). The exclusive-mode reopen between songs.

## Open Questions

- The latency a real device reports (`latency_frames`) is subtracted from the position. Whether a real device drops or plays out the audio in its own buffer when its stream is stopped is not measurable here.
