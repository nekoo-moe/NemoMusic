---
inclusion: always
---

# NemoMusic: Product

Related files: tech.md, structure.md, code.md, design.md, coc.md, workflow.md (same folder). This file owns the "what and why" only.

## Vision
NemoMusic is a desktop music player in Rust that treats the signal path with respect and the screen as something worth looking at. It plays the YouTube Music catalog and local files through an untouched, bit-perfect output path. It should feel like a quiet room, not a feed.

## Target user
- Listens at a desk for hours (work, study, writing) on a good DAC, headphones or speakers.
- Cares about the output chain (exclusive/bit-perfect output, no resampling) and about UI craft.
- Wants fewer notifications, fewer recommendations, fewer decisions.
- Windows and Linux. macOS users come later.

## Product principles (in priority order; use them to settle arguments)
1. Sound first. Any feature that alters samples by default (EQ, normalization, resampling, DSP) is off by default and visibly shown when on. Sound quality wins over visual polish and features.
2. Art first. Cover art, typography, motion and spacing are product features, not decoration. When two designs work equally, ship the more beautiful one.
3. Honest quality labels. Show what is actually playing (codec, bitrate, sample rate, bit depth). Never overstate. See "Quality badge rule".
4. Calm by default. No autoplay surprises, no popups, no badges for engagement, no animation that loops without purpose. Calm beats clever.
5. Privacy. No telemetry by default. Credentials and tokens stay local, stored in the OS credential store. No data leaves the machine except requests to the listed third-party services (see "Local only").
6. Keyboard and focus friendly. Every core action works without a mouse. Playback never stops because the UI is busy.
7. Say the risk out loud. Where the product depends on something fragile or legally gray, the docs and UI say so.

## Local only (owner decision, 2026-11-10)
- NemoMusic runs entirely on the user's computer. The project owns and runs NO server: no backend, relay, account service or telemetry endpoint.
- The user signs in with a Google account. All library and account data comes from YouTube Music and that account, and is stored locally.
- Third-party public services are allowed (YouTube Music, SoundCloud, lyrics providers, SponsorBlock, and the services approved on 2026-11-14 under Scope). Every host the app contacts must be listed in the README ("network egress list"). Adding a host needs owner approval.
- Sign-out wipes all local credentials and cached account data.

## What "lossless" means here
- YouTube Music serves lossy audio only (up to about 256 kbps AAC or Opus on Premium High). It has no lossless tier.
- For YouTube Music, "lossless" in NemoMusic means a bit-perfect output path: the decoded stream is delivered to the device without added processing. The source stays lossy.
- SoundCloud serves lossy audio only (MP3, AAC or Opus). The same rules apply to it.
- True lossless applies only to local FLAC, ALAC and WAV files.

## Scope
### MVP
- Windows 10/11 and Linux (x86_64) desktop app. Linux plays in shared mode through ALSA/PipeWire for now (owner, 2026-11-12).
- Playback from YouTube Music: search, album/artist/playlist views, queue, play/pause/seek/next/prev.
- Bit-perfect output on Windows (WASAPI exclusive), device picker, no resampling when the device supports the stream rate.
- Quality badge and a signal-path view (source codec, decode, output device and format).
- Gapless playback where the source allows it.
- Large cover art, calm theme, light and dark.
- Keyboard shortcuts and media-key support.
- Google sign-in; library, likes, playlists and history from the signed-in account (method: ADR `docs/adr/0001-youtube-music-auth.md`).
- Time-synced lyrics from several open providers with a fallback chain (ADR `docs/adr/0002-lyrics-providers.md`).
- Ad blocking and SponsorBlock, each with an on/off switch (ADR `docs/adr/0003-adblock-sponsorblock.md`).
- UI in the Material + Liquid Glass design language (`design.md`).
- Local file library (FLAC, ALAC, WAV, others) and local playlists, with the true-lossless badge (owner, 2026-11-14).

### Later
- Linux bit-perfect output (direct ALSA `hw:` device) and macOS (CoreAudio) output.
- Focus/session features (timers, quiet mode).

### Approved additions (owner, 2026-11-14)
Each one goes through its own spec in `.kiro/specs/<name>/` (`workflow.md` section 1). Anything that alters samples is off by default and shown in the signal path (principle 1). Anything that shares data, sends notifications or uses the microphone is opt-in and off by default (principles 4 and 5).
- `local-library`: local files and playlists (M3U), folder scan, tags, ReplayGain (opt-in), CUE sheets.
- `offline-cache`: save songs, albums and playlists for offline listening, plus capped smart downloads of the most-played songs (see coc.md).
- `soundcloud`: SoundCloud as a second source.
- `audio-effects`: ten-band equalizer with presets and AutoEq headphone profiles, crossfade, delay and reverb.
- `output-devices`: output device picker and WASAPI exclusive mode. `cast`: play on Cast or DLNA devices.
- `now-playing-styles`: Classic, Material 3 Expressive and an Apple Music-style Now Playing, with animated cover art where available.
- `lyrics-plus`: lyrics editor (line or word timing), romanization, a lyrics excerpt as an image, Spotify and YouTube transcript sources, AI translation.
- `listening-stats`: local listening history, charts, listening clock, a listening receipt, a year in music and monthly recap playlists.
- `account-actions`: dislike, playlist editing synced with YouTube Music, several accounts, Premium quality, preferred audio language, notifications from followed artists.
- `youtube-extras`: search all of YouTube, live stations, comments, Return YouTube Dislike. `video-playback`: video up to 1080p with subtitles.
- `integrations`: OS media controls and media keys, Last.fm scrobbling, Discord Rich Presence.
- `song-recognition`, `ai-features` (song suggestions and a reading of your taste), `playlist-import` (from Spotify and other apps), `listen-together` (rooms compatible with Metrolist).

### Explicit non-goals
- Claiming or simulating lossless for YouTube or SoundCloud streams (no upsampling, no "enhanced" labels).
- Social feeds, follower counts, engagement badges and ads. Allowed and opt-in (owner, 2026-11-14): YouTube comments in Now Playing, Listen Together rooms, and a short lyrics excerpt shared as an image.
- Mobile apps and a web version.
- Audio ripping or bulk downloading tools. The offline cache is not one: it keeps songs inside the app and never exports them (coc.md).
- Streaming services other than YouTube Music and SoundCloud.
- Telemetry or analytics enabled by default. Listening stats are computed and kept on this computer only.
- Any server run by the project.

## Quality bar: "done" means
Audio:
- Bit-perfect path verified by a test that compares the samples sent to the output with the decoded samples (no change in bits) for at least one local WAV and one decoded stream.
- No audible glitches over a 1-hour playback test on the reference machine; no underruns logged.
- Track change, seek and device change never produce clicks or pops.
- The quality badge matches the real stream info in every tested case.
Look:
- Every screen has been reviewed at 100%, 150% and 200% DPI and at the minimum window size.
- Contrast is at least WCAG AA for text; focus is visible; reduced-motion setting is honored.
- Motion is 60 fps on the reference machine with no dropped audio.
- A named reviewer (see coc.md and workflow.md) signs off on the visual result, not only the code.

## Quality badge rule (honest labeling)
- The badge is built from measured stream facts: codec, bitrate (or "unknown"), sample rate, bit depth, and whether the output path is bit-perfect.
- A YouTube stream must show a lossy badge such as "AAC 256 kbps" or "Opus ~160 kbps". The word "Lossless" must never appear on it, nor "Hi-Res", "HD audio" or similar.
- "Bit-perfect output" may be shown as a separate path indicator and must not be merged into the source badge.
- "Lossless" is allowed only when the source file is FLAC, ALAC, WAV or another lossless codec and the path is bit-perfect.
- If the facts are unknown, show "Unknown", never a guess.
- Changing badge logic needs a test for each case above.

## Risks (do not hide)
- Access to YouTube Music is unofficial (no public API for third-party players) and may breach YouTube's Terms of Service. Accounts could be limited, and access could break without notice.
- Owner decision 2026-11-10: this risk is accepted, including ad blocking and SponsorBlock. Ad blocking means the app does not request or play ads. It also means less income for artists and rights holders, and it may get an account restricted.
- Owner decision 2026-11-14: the same kind of risk is accepted for the approved additions that use unofficial access: SoundCloud, the offline cache, Premium quality through the signed-in session, Spotify lyrics and animated covers through the user's own Spotify sign-in, song recognition through a Shazam-compatible service, and Listen Together rooms compatible with Metrolist. Each feature names its risk in the README and in its own settings.
- README and first-run screen must state this plainly.
- NemoMusic must not bypass DRM or Premium audio-quality limits. Premium quality is used only for accounts that have Premium.

## Open decisions for the owner (agents must not decide these)
1. Offline caching of YouTube audio: DECIDED (2026-11-14), allowed inside the app only: no export, a size limit the user sets, deleted on sign-out; smart downloads are capped (coc.md). The `offline-cache` spec sets the numbers.
2. Local-files mode: DECIDED (2026-11-14), part of MVP.
3. Login: DECIDED, Google account, local only. STILL OPEN: the technical method (embedded sign-in, OAuth, cookie import). See ADR 0001.
4. Name and branding: is "NemoMusic" final? Logo, icon, palette direction.
5. License of the project (the repo has a LICENSE file; confirm it is the intended one) and policy for third-party dependencies.
6. Terms of Service risk: ACCEPTED by the owner (2026-11-10). STILL OPEN: the exact notice text shown to users.
7. Minimum supported Windows version and reference test hardware (DAC, headphones).
8. Distribution: installer, portable zip, or store; auto-update yes or no.
9. SponsorBlock data is CC BY-NC-SA 4.0 (ADR 0003). NemoMusic must stay non-commercial while it uses that data. Confirm.
10. Pass the signed-in cookies to `yt-dlp` so Premium accounts get Premium audio quality: DECIDED (2026-11-14), yes. The `account-actions` spec sets the switch and its default.
11. Approve opt-in NetEase and QQ lyrics (unofficial, ToS unverified) and the hosts `lrclib.net` and `sponsor.ajay.app` for the egress list (ADR 0002, 0003).
12. Ad blocking and SponsorBlock default OFF with a first-run choice (ADRs recommend this, matching the no-pre-checked-consent rule). Confirm.

<!-- Source for the "YouTube Music is lossy, max 256 kbps AAC/Opus" claim: https://support.google.com/youtubemusic/answer/9076559 (corroborated by https://www.tunecable.com/youtube-music-tip/youtube-music-lossless.html) -->
