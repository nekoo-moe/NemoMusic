# Device switch fix: tasks

## Tasks

- [ ] 1. Spec with the findings (this PR). _Requirements: 1 to 7_
- [ ] 2. Read the position after the old output stopped; apply the unread start report; seek on a frame boundary; song-end case. Files: `pipeline.rs`, `feeder.rs`, `control_cmd.rs`. _Requirements: 1, 2, 3, 4_
- [ ] 3. Deterministic switch tests with a frame counter; 200-run loop. File: `tests/player_switch.rs`, `tests/common/fakes.rs`. _Requirements: 2, 3, 6, 7_
- [ ] 4. `auto_preamp` peak search in the warped frequency, fixed cases and a fixed seed for the property test; 200-run loop. Files: `effects/biquad.rs`, `effects/biquad/tests.rs`. _Requirements: 5, 6, 7_

## Definition of Done

See section 4 of `.kiro/steering/workflow.md`. Every PR also states the `bit_exact` and `rt_no_alloc` results.
