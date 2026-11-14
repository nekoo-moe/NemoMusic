## Overview

Video is a second, optional picture that follows the audio. Audio keeps its whole path in `nemo-audio` (decode, `rtrb` ring, `cpal` or WASAPI output). A new crate, `nemo-video`, runs libmpv with audio switched off and hands finished frames to the UI as plain pixels. The audio clock is the master: `nemo-video` reads the position the listener hears, and it speeds up, slows down or seeks the picture to match. Nothing in the audio thread knows video exists, so the bit-exact path and the signal-path view are unchanged (criteria 15, 16, 24).

Decoder: libmpv, chosen by the owner on 2026-11-14 and proven by a spike task on Windows and Linux before any other video task starts (criterion 31).

- Licence: mpv is GPL-2.0-or-later by default and LGPL-2.1-or-later when built without its GPL parts. The repository is GPL-3.0, so either build can be linked. The shipped build should be the LGPL one, loaded as a shared library at run time, so the licence stays easy to explain. Windows LGPL builds exist from third parties (zhongfly/mpv-winbuild); which one we ship, and its FFmpeg flags (no `--enable-gpl`), is verified in the spike.
- The crate `libmpv2` 7.0.1 (released 2026-11-07, LGPL-2.1) was tried first and failed the spike (Risks). It is not used.
- The binding is our own, over `libloading` (ISC, allowed): about ten libmpv functions, loaded at run time, no link-time import library on Windows, and the app starts without the DLL. Its `unsafe` stays in the `ffi` module of `nemo-video`, which the owner allowed on 2026-11-14 (`code.md`, Unsafe; ADR 0004). No crate exception is needed; the owner approved shipping libmpv itself.

## Affected Crates

- `nemo-video` (new): `MpvBinding` (own FFI over `libloading` in its `ffi` module), `VideoEngine`, drift controller, frame pool. Depends on `nemo-core` and `libloading` only.
- `nemo-core`: new `video.rs` with `VideoFrame`, `VideoFrameSource`, `PlaybackClock`, `VideoController`, `VideoStreamResolver` and the video types. `PlayerSnapshot` is untouched.
- `nemo-audio`: implements `PlaybackClock` over the existing played-frames and device-latency atomics (`control.rs` `shared.position()`). No decoder, ring or sink change.
- `nemo-ytm`: implements `VideoStreamResolver` with one extra `yt-dlp` run, finds the music-video id of a song, and implements `SubtitleSource` (youtube-extras design, `nemo-core/src/captions.rs`), the same captions code the "YouTube captions" lyrics provider uses.
- `nemo-ui`: Video switch and quality and subtitle menus in Now Playing, a `VideoSurface` shader widget, full-screen mode, subtitle overlay.
- `nemo-app`: builds `nemo-video` (when libmpv loads), injects `VideoController` and `PlaybackClock`, owns the wake lock.

## Data Flow

Threads: UI (iced), tokio runtime (network), audio threads (untouched), and one new `nemo-video` thread that owns the libmpv handle. libmpv has its own decode and demux threads.

1. The user turns Video on. `nemo-app` asks `VideoStreamResolver::resolve_video` (tokio). One `yt-dlp` run returns every format and the caption list; the answer is cached for the track until its URL expires, so a quality change makes no new request.
2. `VideoController::open` sends the video URL and the current audio position to the `nemo-video` thread. libmpv starts with `aid=no`, `ao=null`, `sid=no`, `ytdl=no`, `hwdec=auto-copy-safe`, paused, and seeks exactly to that position. The URL is a `Secret` and never logged.
3. The render context is libmpv's software backend. The thread renders into a buffer from a pool of three (`rgb0`, tightly packed) and publishes the newest one. The UI never waits: `VideoFrameSource::latest` returns an `Arc` of the newest frame, older ones are dropped.
4. The drift loop runs every 10 ms: `drift = audio_position - mpv time-pos`. Under 40 ms: nothing. 40 to 250 ms: set mpv `speed` between 0.95 and 1.05 in proportion to the drift. Over 250 ms (a seek, a SponsorBlock skip, a stall): exact seek to the audio position. Audio paused: mpv paused on the same frame. After a seek settles the drift is under 100 ms (criterion 21). SponsorBlock needs no code of its own: the audio engine seeks, the clock jumps, the picture follows (criterion 23).
5. The UI polls `latest` on its existing `window::frames()` subscription, only while video is on. `VideoSurface` uploads the frame into one persistent `wgpu` texture with `write_texture` and draws it scaled (iced `shader` widget, `wgpu` feature already enabled; alpha forced to 1 in the shader because `rgb0` leaves it undefined, unverified). The GPU does the scaling at 150% and 200% display scale. The fallback, `image::Handle::from_rgba` per frame, allocates a texture per frame and is only for the spike's comparison.
6. Subtitles: the UI draws cue text over the frame as ordinary iced text (crisp at any scale, accessible, reduced-motion safe). Cues come from `nemo-ytm` YouTube captions, the same source as the captions lyrics provider, and are picked by the audio clock. mpv renders no subtitles.
7. Video off, track change without a video, or an error: the thread drops the mpv file, which stops fetching and decoding, and returns the frame pool. The UI shows the cover.
8. Overload: if `PlayerSnapshot::underruns` rises while video is on, or one frame renders slower than twice its interval for 2 seconds, mpv gets `framedrop=vo` first and, if it persists, video stops with "Video unavailable". The video thread runs at below-normal priority so audio threads win (criterion 25).

Audio buffers stay owned by `nemo-audio`. Video frames are owned by the pool in `nemo-video`; the UI holds an `Arc` for one draw.

## Interfaces

```rust
// nemo-core/src/video.rs  (no I/O, no runtime)
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum VideoCodec { H264, Vp9, Av1, Other }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoInfo { pub codec: VideoCodec, pub width: u32, pub height: u32, pub fps: Option<u32> } // "Video: <codec>, <resolution>"
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum VideoQuality { Auto /* best <= 720p */, Height(u32) }
pub const AUTO_MAX_HEIGHT: u32 = 720;
pub const MAX_HEIGHT: u32 = 1080;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleTrack { pub language: String, pub label: String, pub auto_generated: bool }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedVideo {
    pub url: Secret<String>,                 // video-only stream, never logged
    pub info: VideoInfo,
    pub available_heights: Vec<u32>,         // only what the video really has, up to MAX_HEIGHT
    pub subtitles: Vec<SubtitleTrack>,
    pub expires_at: Option<std::time::SystemTime>,
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VideoError {
    #[error("no video for this song")] NoVideo,
    #[error("network error: {0}")] Network(String),
    #[error("video player is not available: {0}")] PlayerMissing(String),
    #[error("video could not be decoded")] Decode,
    #[error("the computer cannot play audio and video together")] Overloaded,
}
/// The music video of a song and its streams. One extra resolver run, human-paced.
pub trait VideoStreamResolver: Send + Sync {
    fn has_video<'a>(&'a self, video: &'a VideoId) -> BoxFuture<'a, Result<bool, VideoError>>;
    fn resolve_video<'a>(&'a self, video: &'a VideoId, quality: VideoQuality, audio_language: Option<&'a str>)
        -> BoxFuture<'a, Result<ResolvedVideo, VideoError>>;
}
/// The position the listener hears. Cheap and lock-free; implemented by `nemo-audio`.
pub trait PlaybackClock: Send + Sync {
    fn position(&self) -> std::time::Duration;
    fn is_playing(&self) -> bool;
}
/// One picture. `pixels` is `rgb0`, row stride `width * 4`.
#[derive(Debug, Clone)] pub struct VideoFrame { pub width: u32, pub height: u32, pub pts: std::time::Duration, pub pixels: std::sync::Arc<[u8]> }
/// What `nemo-ui` draws. Never blocks.
pub trait VideoFrameSource: Send + Sync {
    fn latest(&self) -> Option<VideoFrame>;          // None: show the cover
    fn state(&self) -> VideoState;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoState { Off, Starting, Playing(VideoInfo), Unavailable(VideoError) }
/// Starts and stops video. Commands return at once; results show in `state`.
pub trait VideoController: Send + Sync {
    fn start(&self, stream: ResolvedVideo, at: std::time::Duration);
    fn stop(&self);
    fn frames(&self) -> std::sync::Arc<dyn VideoFrameSource>;
}
// nemo-core/src/services.rs additions (settings): `video_quality: VideoQuality`, `subtitle_language: Option<String>`, `keep_screen_awake: bool` (default false).
// A wake lock is a platform call in nemo-app: `trait ScreenWake { fn hold(&self); fn release(&self); }` in video.rs.

// Subtitles: `SubtitleSource` and `SubtitleCue` are owned by youtube-extras (ADR 0004, row 22). `ResolvedVideo.subtitles` stays, filled from the one `yt-dlp` run, so the menu needs no second call; cues are fetched through `SubtitleSource::cues` when the user picks a language.

/// A silent, looping clip for animated covers (now-playing-styles). One open clip at most.
pub struct ClipInfo { pub width: u32, pub height: u32 }
pub trait ClipPlayer: Send + Sync {
    fn available(&self) -> bool;                                          // false when libmpv is missing
    fn open(&self, file: &std::path::Path) -> Result<ClipInfo, VideoError>; // loops, no audio, at most 30 fps; Err(Overloaded) while a music video plays
    fn pause(&self);
    fn resume(&self);
    fn stop(&self);
    fn frames(&self) -> std::sync::Arc<dyn VideoFrameSource>;
}

// nemo-video
pub struct VideoEngine { /* thread handle, command sender, frame pool */ }
impl VideoEngine {
    pub fn new(clock: std::sync::Arc<dyn PlaybackClock>, underruns: std::sync::Arc<dyn Fn() -> u64 + Send + Sync>) -> Result<Self, VideoError>; // loads libmpv
}
impl VideoController for VideoEngine { /* start, stop, frames */ }
impl ClipPlayer for VideoEngine { /* second libmpv handle: loop-file=inf, aid=no, ao=null; own frame pool of two */ }
```

From other designs I need: `SubtitleSource` and `SubtitleCue` from youtube-extras (the captions fetcher and parser), `PlayerCommand::SwitchAudioTrack` and `StreamRequest` from `account-actions` (audio language, criteria 13 and 14). The Opus decoder is not needed here. `ClipPlayer` above is defined here for now-playing-styles.

## Error Handling

- `VideoError` has fixed texts and carries no URL. libmpv messages are mapped to `Decode` or `Network`; raw text is not shown.
- libmpv library missing at run time: `PlayerMissing`; the Video switch is disabled with "Video needs libmpv" and the rest of the app works.
- Decode error, stall over 5 seconds, or end of file early: `Unavailable`; audio keeps playing, the UI shows "Video unavailable" and a retry button (criterion 17).
- Network drop: mpv's cache runs dry first; the state becomes `Unavailable(Network)` with the line "Video paused, audio continues" (criterion 18). Retry re-resolves.
- A URL that expires: `resolve_video` runs again once at the same position. Two failures in a row stop retrying.
- Track change: if the next track has no video, `has_video` is false and `stop` is called, so the cover shows. No black frame (criterion 19).
- Never writes video to disk: mpv runs with `cache=yes` in memory only, `demuxer-max-bytes` capped, `cache-on-disk=no`, no screenshot or record commands exist in the binding (criterion 22). The offline-cache path hides the switch.
- No `unwrap`. Own FFI (if chosen) wraps every raw pointer in a type that frees it on drop.

## Testing Strategy

- Spike task, first, on Windows and on Linux, throwaway: play one 720p VP9 and one 1080p H.264 video into memory with no audio output from libmpv; draw it in an iced window at 100%, 150% and 200% scale; record median and worst frame render time, CPU use, memory, and shutdown without a hang. Pass: 720p30 renders in 16 ms median, no audio underrun in 10 minutes while the real audio path plays, and both systems work. The measured 1080p cost sets whether 1080p is offered. A single source reports about 52 ms for one software-rendered frame, so 1080p may fail; then the owner decides between 720p as the cap and a fallback below.
- Unit tests with a fake `PlaybackClock` and a fake mpv: the drift controller (no action under 40 ms, speed trim inside the band, seek over 250 ms, pause follows the clock, a SponsorBlock jump follows within 100 ms).
- `nemo-ytm`: parse a recorded `yt-dlp` answer: `available_heights` never exceeds what exists or 1080; `Auto` never picks over 720; `has_video` false for audio-only songs; captions labelled `auto_generated`; the URL never appears in `Debug`.
- Fake-engine UI tests: Video switch states, quality and subtitle menus, full screen enter, auto-hide, Escape, keyboard-only use, reduced motion (no transition).
- Audio is untouched: `cargo test -p nemo-audio --test bit_exact` passes in every video task. The only `nemo-audio` change is the `PlaybackClock` impl, which reads atomics outside the callback; a counting-allocator test on the callback is not needed, but the existing one must keep passing.
- Manual: 10 minutes of video with exclusive mode on, then check the badge and the signal-path view are identical to audio only.

## Risks and Alternatives

- Software rendering may be too slow at 1080p. Fallbacks, in order: cap at 720p; an OpenGL render context on a hidden GL context with a read-back (needs a GL crate, owner to approve); a native child window (breaks the glass layout); GStreamer (LGPL-2.1, heavy on Windows). The owner picks if the spike fails.
- libmpv on Windows is a DLL to find, ship and update, with its licence text and a source offer. Which build, and where it lives next to the executable, is decided in the spike; the app must start without it.
- Unsafe code: `code.md` allows `unsafe` in the `ffi` module of `nemo-video` (owner, 2026-11-14, after the spike below), only to load libmpv and call its C API. Every raw pointer is wrapped in a type that frees it on drop, every `unsafe` block has a `// SAFETY:` comment, and the rest of the crate sees only safe functions.
- Licence: an LGPL build keeps the shipped combination GPL-3.0 compatible. A GPL-2.0-or-later build also works with GPL-3.0 but shuts out any later relicensing; the owner accepted either. Own FFI over `libloading` needs no crate exception.
- Speed trimming moves the picture, not the audio, so audio stays bit-exact. A change of audio device latency (a device switch) makes the clock jump; the controller then seeks the picture.
- `yt-dlp` may return no separate video stream for a song id; the music-video id is a different id found from the `next` response (unverified), so "No video" can be a false negative. The spike lists three songs.
- Departures from ADR 0004: none. `nemo-video` depends on `nemo-core` and its binding crate; `nemo-ui` shows frames only through `VideoFrameSource`. The extra `PlaybackClock` trait is added to `nemo-core`.
- Alternative: let libmpv play the audio too. Rejected by the owner and by criterion 15: it would break bit-perfect output and the badge.
- Spike result (task 1, Windows half, 2026-11-14, Windows 11 Pro 26200): **`libmpv2` alone fails; stopped before the iced and timing steps, owner decision needed.** The Linux half was not run (needs the owner's Linux Mint machine).
  - libmpv source: GitHub release `20261111` of `shinchiro/mpv-winbuild-cmake`, asset `mpv-dev-x86_64-20261111-git-bcde6b93cc.7z`, SHA-256 `273c079b11ea5575822f972756f4c9a71149a5c10130353abeb8586738938452` (equal to GitHub's asset digest). It holds `libmpv-2.dll` (mpv v0.42.0-38-gbcde6b93c, FFmpeg N-128766-gcd4e96eb3, client API 2.6), `libmpv.dll.a` and the headers; kept in the lane's scratch folder, not in the repo.
  - Licence: this build is GPL, not LGPL. The repo's `packages/ffmpeg.cmake` configures FFmpeg with `--enable-gpl --enable-version3` and `libx264`, `libx265` and `librubberband`, and mpv is built with its default `gpl` option. No LGPL Windows build was found from a source that can be named and checked; making one (mpv `-Dgpl=false`, FFmpeg without `--enable-gpl`) was not tried.
  - Build and run: a probe binary with `#![forbid(unsafe_code)]` and `libmpv2` =7.0.1 (with `libmpv2-sys` 4.0.1, pregenerated bindings) linked against `libmpv.dll.a` under the MSVC toolchain (copied as `mpv.lib`, passed with `-L native=`). With `vo=null`, `ao=null`, `aid=no`, `hwdec=auto-copy-safe` it played a generated 10 s 720p30 VP9 file and a 10 s 1080p30 H.264 file to the end in real time (10.4 s and 11.5 s), 0.7 s CPU in total, 190 MB peak working set, shutdown 1 ms. `hwdec-current` stayed empty (no video output).
  - Blocker 1: `libmpv2` 7.0.1 wraps only the OpenGL render API (`RenderParamApiType::OpenGl`); it has no `MPV_RENDER_API_TYPE_SW`, which libmpv itself offers. So the software render into memory (`rgb0`) of the data flow needs raw `libmpv2-sys` calls, which are `unsafe`. The OpenGL route needs a GL context and a read-back, also `unsafe` (fallback 2 above, owner to approve).
  - Blocker 2: `libmpv2-sys` links `mpv` at load time, so the probe without `libmpv-2.dll` next to it did not start (`0xC0000135`, DLL not found). "The app must start without it" therefore also needs run-time loading (`libloading`).
  - Both point to the own-FFI path over `libloading` in an `ffi` module of `nemo-video`. The owner allowed it on 2026-11-14 (`code.md`, Unsafe), so the spike goes on with that binding. Not measured yet: the iced `shader` versus `image::Handle` frame times at 100, 150 and 200% scale, the 10-minute audio-underrun run, `rgb0` alpha, the music-video id lookup and the hosts (the probe played local files only).

## Steering and README changes

- `tech.md`: add a "Video" row: libmpv (shared library loaded at run time; GPL or LGPL build, owner-accepted; version pinned in the spike) with our own binding over `libloading` =0.9.1 (ISC, MSRV 1.88; `Cargo.lock` has 0.8.9 for other crates), status "decided". Add the `ffmpeg` rejection note: libmpv carries its own FFmpeg, loaded as a binary. Record `libmpv2` as rejected (spike).
- `structure.md`: add `nemo-video` (depends on `nemo-core` only, plus its binding); `nemo-app` depends on it; `nemo-audio` gets the `PlaybackClock` impl.
- README egress list: `*.googlevideo.com` (video streams, already used for audio), `www.youtube.com` (also the caption host: captions are fetched by `yt-dlp`, youtube-extras design). The spike confirms the final set; no new host is expected. README gets the libmpv licence notice and a source link.
- `deny.toml`: no exception (the `libmpv2` crate is not used).
- ADR: a short ADR 0005 records libmpv, its licence and the audio-clock rule; ADR 0004 needs no change.

<!-- Sources:
- Repo: .kiro/steering/tech.md, structure.md, product.md; crates/nemo-audio/src/{control,control_cmd,engine,cpal_sink}.rs; crates/nemo-core/src/{player,stream,segment}.rs; crates/nemo-ui/Cargo.toml (iced 0.14.0 features wgpu, advanced); crates/nemo-ui/src/gui/app.rs (subscription, window::frames); docs/adr/0004-feature-wave-crates.md
- libmpv2 7.0.1: https://crates.io/crates/libmpv2 (LGPL-2.1, 2026-11-07); libloading 0.9.1: https://crates.io/crates/libloading (ISC, rust-version 1.88.0)
- mpv software render backend: https://github.com/Arobce/zap-tv/blob/main/docs/decisions/0002-video-presentation-path.md , https://github.com/PoHsuanLai/mpv-wgpu , https://github.com/iced-rs/iced/issues/468
- libmpv LGPL builds: https://github.com/zhongfly/mpv-winbuild ; GStreamer licensing: https://github.com/GStreamer/gst-docs/blob/master/markdown/legal-information.md
- Unverified: `rgb0` alpha handling in the shader, mpv `hwdec=auto-copy-safe` on both systems, the music-video id lookup, the caption host, the 1080p software-render cost.
-->
