# Fast start: tasks

## Tasks

- [x] 1. Stage timing at debug level and the measuring example `fast_start_stages`; spec files; BEFORE numbers in the PR. _Requirements: 1, 2, 9_
- [x] 2. Reuse the opened next song on Next, and warm only the next entry; tests with fakes; bit-exact and no-alloc tests. _Requirements: 4, 5, 7_
- [x] 3. Start yt-dlp beside a native request that is slow (1.5 s); the first good answer wins and the other is cancelled; a dropped yt-dlp resolve kills its process. _Requirements: 2, 6_
- [x] 4. Linux onedir build for yt-dlp (`yt-dlp_linux.zip`, unpacked with `unzip`; the one-file build when `unzip` is missing); `ytdlp-auto-install` spec lines updated; the `tech.md` sentence is the owner's to edit. _Requirements: 10_
- [x] 5. A refused video is not a failed request: the native path rests after 5 refusals in a row for 2 minutes (was: 2 in a row for 10 minutes); failed requests still rest it as before. _Requirements: 9_

## Definition of Done

See section 4 of `.kiro/steering/workflow.md`. Per PR here: `cargo fmt --all -- --check`, `cargo clippy -p nemo-ytm -p nemo-audio -p nemo-app --all-targets -- -D warnings`, `cargo test` for the crates touched, and for task 2 `cargo test -p nemo-audio --test bit_exact --test rt_no_alloc`.
