## Goal

The user can watch the music video of a song inside NemoMusic, in up to 1080p, with subtitles, and switch between audio only and video without losing their place. Video can go full screen. Audio only stays the default and keeps the calm, bit-perfect path. The audio language chosen in `account-actions` applies to video too.

## User Stories

- As a listener, I want to watch the music video, so that I get the picture that goes with the song.
- As a listener, I want subtitles, so that I can follow songs in other languages.
- As a listener, I want to switch between audio and video at any time, so that I can save bandwidth and attention.
- As a listener, I want full screen, so that the video fills the display.
- As a listener, I want my preferred audio language used, so that dubbed videos play right.

## Acceptance Criteria

1. THE SYSTEM SHALL play audio only by default. Video is never started by itself.
2. WHEN a song has a music video, THE SYSTEM SHALL show a "Video" switch in Now Playing. IF it has none, THEN THE SYSTEM SHALL disable the switch and say "No video".
3. WHEN the user turns video on, THE SYSTEM SHALL start the picture at the current playback position and SHALL NOT restart the song.
4. WHEN the user turns video off, THE SYSTEM SHALL stop fetching and decoding video and keep playing the audio from the same position.
5. WHEN video is on, THE SYSTEM SHALL offer a quality choice from the resolutions the video really has, up to 1080p. The default is "Auto", capped at 720p.
6. THE SYSTEM SHALL NOT offer a resolution the video does not have, and SHALL NOT label YouTube video "lossless", "HD audio" or "Hi-Res".
7. WHEN the video has subtitles, THE SYSTEM SHALL offer a subtitle menu with each language and "Off". The default is "Off".
8. WHEN the user picks a subtitle language, THE SYSTEM SHALL show the text in sync with the picture, and keep the choice for the next video.
9. IF a video has no subtitles, THEN THE SYSTEM SHALL show "No subtitles".
10. WHEN the user presses the full-screen key or button, THE SYSTEM SHALL fill the display with the video, hide the chrome after a few seconds without input, and show controls again on any key or pointer move.
11. WHEN the user presses Escape in full screen, THE SYSTEM SHALL leave full screen. Playback continues.
12. WHILE in full screen, THE SYSTEM SHALL keep the media keys, play, pause, seek, next and previous working.
13. WHEN a video has several audio tracks, THE SYSTEM SHALL use the audio language set in `account-actions`. IF none matches, THEN THE SYSTEM SHALL use the default track.
14. WHEN the user changes the audio language in the video view, THE SYSTEM SHALL change it for the audio-only view as well.
15. WHILE video is on, THE SYSTEM SHALL keep the audio on the same output path as audio only. The picture follows the audio clock, never the reverse.
16. WHILE video is on, THE SYSTEM SHALL keep the signal-path view and the quality badge based on the audio stream actually playing, and show "Video: <codec>, <resolution>" as a separate line. The badge SHALL NOT merge video facts into the audio facts.
17. IF the video stalls, drops frames, or cannot be decoded, THEN THE SYSTEM SHALL keep the audio playing, show "Video unavailable" and offer a retry.
18. IF the network drops while video is on, THEN THE SYSTEM SHALL fall back to audio only as far as the buffered audio allows, and say so in one line.
19. WHEN the track changes while video is on, THE SYSTEM SHALL show the next video if it has one, otherwise show the cover art. It SHALL NOT leave a black frame.
20. WHEN playback is paused, THE SYSTEM SHALL pause the video on the same frame.
21. WHEN seeking, THE SYSTEM SHALL keep audio and picture within 100 ms of each other after the seek settles.
22. THE SYSTEM SHALL NOT save, cache to disk, or export video. WHEN the offline cache plays a song, THE SYSTEM SHALL NOT use the video switch.
23. WHEN `SponsorBlock` is on, THE SYSTEM SHALL apply the same skips to the picture and the audio.
24. WHEN video starts, THE SYSTEM SHALL NOT change the output device, sample rate or exclusive mode that audio only was using.
25. THE SYSTEM SHALL keep the audio free of underruns while video decodes. IF the machine cannot do both, THEN THE SYSTEM SHALL drop the video first.
26. WHILE video is on, THE SYSTEM SHALL keep the screen awake only if the user allows it, and release it when video is off or paused.
27. WHERE the user has reduced motion on, THE SYSTEM SHALL switch between cover and video with no transition, and SHALL NOT autoplay animated covers.
28. WHEN any video control is used, THE SYSTEM SHALL be operable by keyboard alone, with a visible focus and an accessible name.
29. WHEN a new network host is used for video or subtitles, THE SYSTEM SHALL list it in the README egress list in the same change. Expected: the YouTube video hosts that `yt-dlp` returns (`*.googlevideo.com`), which music playback already uses, and `www.youtube.com`. The design confirms the final set.
30. WHEN the app builds, THE SYSTEM SHALL add no dependency that makes the project's licence (GPL-3.0) or the Windows or Linux build unusable. The video decoder is libmpv. Its licence is LGPL-2.1-or-later or GPL-2.0-or-later, depending on the build, and it needs a `deny.toml` exception that the owner approves before it merges.

31. THE SYSTEM SHALL prove libmpv in a spike task on Windows and on Linux (frames decoded to memory, shown in an `iced` window, no audio output from libmpv) before any other video task starts. IF the spike fails on either system, THEN no further video task SHALL start until the owner picks another decoder.
32. THE SYSTEM SHALL take subtitles from YouTube captions, the same source as the captions lyrics provider in `nemo-ytm`. WHEN a caption track is auto-generated, THE SYSTEM SHALL label it "auto-generated" in the menu.

## Out of Scope

- Downloading, recording or exporting video.
- Video from local files (`local-library` may add it later).
- Picture-in-picture and casting video (`cast` is a separate spec).
- Video effects, filters, upscaling and HDR or 4K.
- Video on Cast or DLNA devices.
- Moving the audio path to the video library. Audio goes through `nemo-audio`, with the audio clock as master (owner, 2026-11-14).

## Open Questions

Decided on 2026-11-14 (owner): libmpv first, proven by a spike on Windows and Linux. Audio stays on `nemo-audio` with the audio clock as master, and video frames follow it. "Auto" is capped at 720p. Subtitles come from YouTube captions.

1. Frame output: how are frames shown in an `iced` 0.14 window (texture upload or shader widget), and what does that cost at 150% and 200% display scale? The spike measures it.
2. A video stream for a song needs a second `yt-dlp` run (its answer lists every format and the captions). The design asks for it once, when the user first turns video on for a song.

<!-- Sources:
- Repo: .kiro/steering/product.md, tech.md (ffmpeg rejected, line 21), coc.md; crates/nemo-audio/src/decoder.rs
- libmpv licensing and LGPL builds: https://github.com/zhongfly/mpv-winbuild , https://github.com/loustack17/AnimePortable/blob/main/docs/LIBMPV_RUNTIME_NOTICE.txt , https://github.com/jason-yau/mpv-prebuild
- GStreamer licensing: https://github.com/GStreamer/gst-docs/blob/master/markdown/legal-information.md
-->
