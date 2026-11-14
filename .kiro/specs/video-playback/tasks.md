## Tasks

Tasks in other specs are cited by spec and the ADR 0004 Decision 4 row they own, until their task numbers are fixed. Criterion 31 puts the libmpv spike before every other video task, including the `nemo-core` groundwork, so task 1 is a Wave 0 spike and tasks 2 to 5 wait for it. Every video task keeps the audio path as it is: the picture follows the audio clock, libmpv plays no audio, and `cargo test -p nemo-audio --test bit_exact` passes.

- [ ] 1. Spike: libmpv through `libmpv2` on Windows and Linux
  - Crate and files: throwaway branch `spike/nemo-video-libmpv`; only the result merges, into the Risks and Alternatives section of `.kiro/specs/video-playback/design.md`.
  - Does: Tries `libmpv2` =7.0.1 first. Plays a 720p VP9 and a 1080p H.264 video into memory with libmpv's software render (`rgb0`, `aid=no`, `ao=null`) and draws them in an `iced` 0.14 window through a `shader` widget with `write_texture`, compared with `image::Handle::from_rgba`, at 100%, 150% and 200% scale, on Windows and on Linux. Records median and worst frame time, CPU, memory and a clean shutdown, plus: an LGPL libmpv build for Windows and its FFmpeg flags (no `--enable-gpl`), where the DLL lives, that the app starts without it, `rgb0` alpha, `hwdec=auto-copy-safe`, the music-video id lookup for three songs, and the hosts contacted.
  - Pass and fail: pass when 720p30 renders in 16 ms median or less with no audio underrun in 10 minutes while the real audio path plays, on both systems. The 1080p cost decides whether task 19 offers 1080p or caps at 720p. If `libmpv2` alone fails, own FFI over `libloading` is tried, and task 6 changes, only after the owner extends the `code.md` unsafe rule to an `ffi` module of `nemo-video`. If either system fails, no further video task starts until the owner picks another decoder (criterion 31).
  - _Requirements: 25, 30, 31_
  - Depends on: none
  - Wave: 0
  - Checks: needs the owner: a Linux machine for the run, and agreement to ship libmpv. The result is written into the design's Risks section.
  - Steering: none.
  - Status (2026-11-14): Windows half stopped at the binding. `libmpv2` 7.0.1 has no software render API and links libmpv at load time, and the only checkable Windows build (shinchiro, `20261111`) is GPL. Waiting on the owner: extend the `code.md` unsafe rule to an `ffi` module of `nemo-video` (own FFI over `libloading`), or pick another route; and accept a GPL build or ask for an LGPL one. Linux half needs the owner's machine. Details in design Risks.

- [ ] 2. Add the video types and traits
  - Crate and files: `nemo-core` (new `src/video.rs`, `src/lib.rs`). Touches nemo-core/src/video.rs and nemo-core/src/lib.rs.
  - Does: Adds `VideoCodec`, `VideoInfo`, `VideoQuality`, `AUTO_MAX_HEIGHT`, `MAX_HEIGHT`, `SubtitleTrack`, `ResolvedVideo`, `VideoError`, `VideoStreamResolver`, `VideoFrame`, `VideoFrameSource`, `VideoState`, `VideoController` and `ScreenWake` (ADR 0004 row 37). No I/O.
  - _Requirements: none (groundwork)_
  - Depends on: video-playback#1
  - Wave: 0
  - Checks: `Debug` of `ResolvedVideo` never prints the URL; `VideoError` texts hold no URL.
  - Steering: none.

- [ ] 3. Add `PlaybackClock` and implement it in `nemo-audio`
  - Crate and files: `nemo-core` (`src/video.rs`); `nemo-audio` (`src/control.rs`). Touches nemo-core/src/video.rs, after video-playback#2.
  - Does: Adds the `PlaybackClock` trait (ADR 0004 row 38) and implements it in `nemo-audio` over the existing played-frames and device-latency atomics (`shared.position()`). Lock-free and allocation-free; nothing in the audio callback changes.
  - _Requirements: 15_
  - Depends on: video-playback#2
  - Wave: 0
  - Checks: `cargo test -p nemo-audio --test bit_exact`; the counting-allocator test `rt_no_alloc` still passes; the position advances with simulated callbacks and stops while paused.
  - Steering: `structure.md`: `nemo-audio` implements `PlaybackClock`.

- [ ] 4. Add `ClipPlayer` and `ClipInfo`
  - Crate and files: `nemo-core` (`src/video.rs`). Touches nemo-core/src/video.rs, after video-playback#3.
  - Does: Adds the silent, looping clip trait that now-playing-styles uses for animated covers (ADR 0004 row 39).
  - _Requirements: none (groundwork)_
  - Depends on: video-playback#3
  - Wave: 0
  - Checks: none beyond the usual.
  - Steering: none.

- [ ] 5. Add the video settings
  - Crate and files: `nemo-core` (`src/services.rs`); `nemo-app` (config). Touches nemo-core/src/services.rs; order it with the other tasks on that file (account-actions#29).
  - Does: Adds `video_quality` (default `Auto`), `subtitle_language` (default `None`, "Off") and `keep_screen_awake` (default false), with their `SettingChange` variants. No setting starts video by itself.
  - _Requirements: 1, 5, 7, 26_
  - Depends on: video-playback#2
  - Wave: 0
  - Checks: settings round trip and defaults.
  - Steering: none.

- [ ] 6. Add the `nemo-video` crate with the libmpv binding
  - Crate and files: new `nemo-video` (`Cargo.toml`, `src/lib.rs`, `src/binding.rs`); workspace `Cargo.toml` members; `deny.toml` when it exists.
  - Does: Loads libmpv at run time through the binding task 1 chose and returns `PlayerMissing` when it is absent, so the app starts without it. Sets fixed options: `aid=no`, `ao=null`, `sid=no`, `ytdl=no`, `hwdec=auto-copy-safe`, in-memory cache only with `demuxer-max-bytes` capped and `cache-on-disk=no`. The binding exposes no screenshot or record command.
  - _Requirements: 15, 22, 30_
  - Depends on: video-playback#1, video-playback#2
  - Wave: 1
  - Checks: a test without libmpv returns `PlayerMissing`; `cargo deny check` with the `libmpv2` exception; `#![forbid(unsafe_code)]` stays unless the owner changed `code.md` (own FFI path). Needs the owner to approve the `deny.toml` exception before merge.
  - Steering: `tech.md`: "Video" row (libmpv LGPL build loaded at run time, `libmpv2` =7.0.1 or `libloading` =0.9.1, version pinned from the spike) and the `ffmpeg` note (libmpv carries its own FFmpeg, loaded as a binary). `structure.md`: add `nemo-video` (depends on `nemo-core` and its binding only), `nemo-app` depends on it. New ADR 0005: libmpv, its licence and the audio-clock rule. `deny.toml`: the `libmpv2` exception, only if that binding is chosen.

- [ ] 7. Render frames into a pool of three
  - Crate and files: `nemo-video` (new `src/frame_pool.rs`, `src/render.rs`).
  - Does: One `nemo-video` thread at below-normal priority owns the libmpv handle, renders with the software backend into three `rgb0` buffers and publishes the newest. `VideoFrameSource::latest` returns an `Arc` and never blocks; older frames are dropped.
  - _Requirements: 25_
  - Depends on: video-playback#6
  - Wave: 1
  - Checks: pool tests (never more than three buffers, the newest wins, the reader never waits). Thread priority checked by hand on Windows and Linux; a platform crate not already in `Cargo.lock` needs the owner (`workflow.md` section 8).
  - Steering: none.

- [ ] 8. Resolve the video stream and the caption list
  - Crate and files: `nemo-ytm` (new `src/video.rs`, `src/ytdlp.rs`); fixture under `crates/nemo-ytm/tests/fixtures/`.
  - Does: Implements `VideoStreamResolver::resolve_video` with one extra `yt-dlp` run when the user first turns video on for a song. Fills `available_heights` with only the heights that exist, at most 1080; `Auto` picks 720p or less; lists caption tracks with `auto_generated`; sets `expires_at`. The answer is cached per track until it expires, so a quality change makes no new request.
  - _Requirements: 5, 6, 29, 32_
  - Depends on: video-playback#2
  - Wave: 1
  - Checks: a recorded `yt-dlp` answer with no cookies: heights never exceed what exists or 1080, `Auto` never above 720, `auto_generated` set, the URL absent from `Debug`; a quality change sends no request.
  - Steering: README egress: `*.googlevideo.com` and `www.youtube.com` also name video and subtitles, in the final set the spike confirmed.

- [ ] 9. Find the music video of a song
  - Crate and files: `nemo-ytm` (`src/video.rs`); fixtures under `crates/nemo-ytm/tests/fixtures/`.
  - Does: `has_video` finds the music-video id from the `next` response as task 1 showed, and returns false for audio-only songs.
  - _Requirements: 2, 19_
  - Depends on: video-playback#8
  - Wave: 1
  - Checks: recorded `next` answers for the spike's three songs, including one without a video.
  - Steering: none.

- [ ] 10. Keep the picture on the audio clock
  - Crate and files: `nemo-video` (new `src/drift.rs`).
  - Does: Every 10 ms compares the audio position with mpv `time-pos`: under 40 ms nothing, 40 to 250 ms a `speed` between 0.95 and 1.05 in proportion, over 250 ms an exact seek. When audio pauses, mpv pauses on the same frame. SponsorBlock skips follow through the clock with no code of their own; the audio is never adjusted.
  - _Requirements: 15, 20, 21, 23_
  - Depends on: video-playback#3, video-playback#6
  - Wave: 2
  - Checks: unit tests with a fake `PlaybackClock` and a fake mpv: each band, pause, and a SponsorBlock jump that settles under 100 ms.
  - Steering: none.

- [ ] 11. Start and stop video at the audio position
  - Crate and files: `nemo-video` (new `src/engine.rs`).
  - Does: `VideoEngine` implements `VideoController`. `start` opens the URL paused, seeks exactly to the audio position, then plays; `stop` drops the file, which stops fetching and decoding, and returns the pool. libmpv errors map to `Decode` or `Network`; a stall over 5 seconds or an early end becomes `Unavailable`; audio is never touched.
  - _Requirements: 3, 4, 17, 18, 22_
  - Depends on: video-playback#7, video-playback#10
  - Wave: 2
  - Checks: fake-mpv tests for every state change; the URL never reaches a log; nothing is written to disk; `cargo test -p nemo-audio --test bit_exact`.
  - Steering: none.

- [ ] 12. Drop the video before the audio under load
  - Crate and files: `nemo-video` (`src/engine.rs`).
  - Does: When `PlayerSnapshot::underruns` rises while video is on, or a frame renders slower than twice its interval for 2 seconds, sets `framedrop=vo`; if it persists, video stops with `Overloaded` and audio continues.
  - _Requirements: 25_
  - Depends on: video-playback#11
  - Wave: 2
  - Checks: tests with fake counters; manual 10 minutes on a slow machine (owner hardware).
  - Steering: none.

- [ ] 13. Play silent looping clips
  - Crate and files: `nemo-video` (new `src/clip.rs`).
  - Does: `VideoEngine` implements `ClipPlayer` with a second libmpv handle (`loop-file=inf`, `aid=no`, `ao=null`, at most 30 fps, a pool of two). `open` returns `Overloaded` while a music video plays; `available` is false without libmpv.
  - _Requirements: none (groundwork for now-playing-styles, ADR 0004 row 39)_
  - Depends on: video-playback#4, video-playback#7
  - Wave: 2
  - Checks: fake-mpv tests for loop, pause, stop and the refusal while a video plays.
  - Steering: none.

- [ ] 14. Wire video into `nemo-app`
  - Crate and files: `nemo-app` (wiring, UI backend).
  - Does: Builds `VideoEngine` when libmpv loads and injects `VideoController`, `VideoFrameSource`, `PlaybackClock` and `VideoStreamResolver`. Video starts only from the switch. On a track change it calls `has_video` and starts the next video or stops so the cover shows; an expired URL is resolved again once at the same position, and two failures in a row stop retrying. A song played from the offline cache gets no video. Output device, sample rate and exclusive mode are not touched.
  - _Requirements: 1, 19, 22, 24_
  - Depends on: video-playback#5, video-playback#9, video-playback#11, offline-cache (ADR 0004 row 17: `downloads.rs`)
  - Wave: 2
  - Checks: fake-engine tests for track change, expiry and the offline-cache case; `cargo test -p nemo-audio --test bit_exact`.
  - Steering: none.

- [ ] 15. Keep the screen awake while video plays
  - Crate and files: `nemo-app` (platform `ScreenWake` for Windows and Linux).
  - Does: Holds the wake lock only while video plays and `keep_screen_awake` is on, and releases it on pause, video off and exit.
  - _Requirements: 26_
  - Depends on: video-playback#5, video-playback#14
  - Wave: 2
  - Checks: fake `ScreenWake` tests; manual on Windows and Linux. A platform crate not already in `Cargo.lock` needs the owner.
  - Steering: none.

- [ ] 16. Pick subtitle cues by the audio clock
  - Crate and files: `nemo-ui` (new subtitle cue module under `src/gui/`); `nemo-app` (fetches cues).
  - Does: When the user picks a language, `nemo-app` fetches cues once through `SubtitleSource::cues`; the UI picks the cue for the `PlaybackClock` position. A video with no captions reports "No subtitles".
  - _Requirements: 8, 9, 32_
  - Depends on: video-playback#3, video-playback#14, youtube-extras (ADR 0004 row 22: `SubtitleSource`, `SubtitleCue`)
  - Wave: 2
  - Checks: cue selection tests, including after a seek and between cues.
  - Steering: none.

- [ ] 17. Draw frames with `VideoSurface`
  - Crate and files: `nemo-ui` (new `src/gui/widgets/video_surface.rs`).
  - Does: An iced `shader` widget uploads the newest frame into one persistent `wgpu` texture with `write_texture` and draws it scaled, with alpha forced to 1. It polls `latest` on the existing `window::frames()` subscription only while video is on, and shows the cover whenever there is no frame, never a black frame. No transition under reduced motion.
  - _Requirements: 19, 27_
  - Depends on: video-playback#2
  - Wave: 3
  - Checks: headless tests; `design.md` checklist at 100%, 150% and 200% scale with a screenshot.
  - Steering: none.

- [ ] 18. Add the Video switch to Now Playing
  - Crate and files: `nemo-ui` (Now Playing view).
  - Does: A "Video" switch, off at start, disabled with "No video" or "Video needs libmpv". It shows "Video unavailable" or "Video paused, audio continues" with a retry. Turning it off keeps the audio playing from the same position. Keyboard alone, visible focus, accessible names.
  - _Requirements: 1, 2, 4, 17, 18, 28_
  - Depends on: video-playback#14, video-playback#17
  - Wave: 3
  - Checks: fake-engine UI tests for every switch state; `design.md` checklist with a screenshot.
  - Steering: none.

- [ ] 19. Add the quality menu and the video line
  - Crate and files: `nemo-ui` (Now Playing view, signal-path view).
  - Does: The quality menu lists "Auto" (default, 720p or less) and only the heights the video has, up to 1080 (or 720 if task 1 set that cap). "Video: <codec>, <resolution>" shows on its own line; the quality badge and signal-path view keep showing only the audio stream. No "lossless", "HD audio" or "Hi-Res" label for YouTube.
  - _Requirements: 5, 6, 16, 28_
  - Depends on: video-playback#8, video-playback#18
  - Wave: 3
  - Checks: headless tests, keyboard only; the badge is identical with video on and off; `design.md` checklist with a screenshot.
  - Steering: none.

- [ ] 20. Add the subtitle menu and overlay
  - Crate and files: `nemo-ui` (Now Playing view, video overlay).
  - Does: A menu with each language, labelled "auto-generated" where so, and "Off" (default); "No subtitles" when there are none. The choice is kept for the next video. Cue text is drawn as iced text over the frame.
  - _Requirements: 7, 8, 9, 28, 32_
  - Depends on: video-playback#16, video-playback#17, video-playback#18
  - Wave: 3
  - Checks: headless tests, keyboard only; readable at 200% text scale; `design.md` checklist with a screenshot.
  - Steering: none.

- [ ] 21. Add full screen
  - Crate and files: `nemo-ui` (full-screen mode, window handling).
  - Does: A key and a button enter full screen. The chrome hides after a few seconds without input and returns on any key or pointer move. Escape leaves and playback continues; play, pause, seek, next, previous and media keys keep working.
  - _Requirements: 10, 11, 12, 28_
  - Depends on: video-playback#18
  - Wave: 3
  - Checks: headless tests for enter, auto-hide, Escape and keyboard transport; manual media-key check once integrations' OS media controls (ADR 0004 row 31) are in.
  - Steering: none.

- [ ] 22. Change the audio language from the video view
  - Crate and files: `nemo-ui` (Now Playing video view).
  - Does: When the video has several audio tracks, the view uses the preferred language from account-actions and offers a language menu. A change sends `PlayerCommand::SwitchAudioTrack` and updates the same setting the audio-only view uses.
  - _Requirements: 13, 14, 28_
  - Depends on: video-playback#18, account-actions#10, account-actions#27, account-actions#36
  - Wave: 3
  - Checks: headless tests: the choice shows in both views; `cargo test -p nemo-audio --test bit_exact`.
  - Steering: none.

- [ ] 23. Add the "Keep screen awake" setting
  - Crate and files: `nemo-ui` (settings view).
  - Does: One switch, off by default, saying that the screen stays on only while video plays.
  - _Requirements: 26, 28_
  - Depends on: video-playback#15
  - Wave: 3
  - Checks: headless test, keyboard only.
  - Steering: none.

- [ ] 24. Ship libmpv with the app
  - Crate and files: Windows installer configuration; `assets/` licence notices; `README.md`.
  - Does: Ships the LGPL libmpv build chosen in task 1 next to `nemo-app.exe`, with its licence text and a source link, and documents the libmpv package to install on Linux. The app still starts and plays audio without it.
  - _Requirements: 30_
  - Depends on: video-playback#6
  - Wave: 3
  - Checks: clean-VM smoke test (`workflow.md` section 7) with and without the DLL; needs the owner for the licence review and the installer signing.
  - Steering: README: the libmpv licence notice and source link.

- [ ] 25. Run the video manual checks
  - Crate and files: none (results in the PR description).
  - Does: On Windows and Linux: 10 minutes of video with exclusive mode on, then the badge and signal-path view must match audio only, with no underrun and no change of device, rate or exclusive mode. Also checks seek sync within 100 ms, a SponsorBlock skip, a network drop and that nothing video-related was written to disk.
  - _Requirements: 15, 16, 21, 23, 24, 25_
  - Depends on: video-playback#19, video-playback#20, video-playback#21, video-playback#22, video-playback#23, video-playback#24
  - Wave: 3
  - Checks: needs the owner (an exclusive-mode device, a Linux machine); `cargo test -p nemo-audio --test bit_exact`.
  - Steering: none.

## Definition of Done

Every task meets [section 4 of workflow.md](../../steering/workflow.md#4-definition-of-done). For this spec also:

- [ ] `cargo test -p nemo-audio --test bit_exact` passes in every video task, and `rt_no_alloc` keeps passing; no video task changes the audio callback.
- [ ] libmpv plays no audio (`aid=no`, `ao=null`) and writes nothing to disk in any task.
- [ ] No video task after task 1 starts before the spike result is in the design's Risks section and has passed on Windows and Linux (criterion 31).
- [ ] The `libmpv2` `deny.toml` exception is approved by the owner before task 6 merges.
- [ ] Stream URLs never reach logs, errors or `Debug` output.
