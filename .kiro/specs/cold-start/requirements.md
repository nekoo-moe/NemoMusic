# Cold start: requirements

## Goal

The first song after the app opens starts about 0.5 s after the click, as the songs after it do. The owner measured 7 s or more for the first song of a fresh app process (2026-11-19) while a replayed song starts at once. The work measures a fresh process first, then removes the cold costs in the order the numbers justify. Owner approvals of 2026-11-19: other access paths and study of similar projects (SimpMusic, Metrolist, NewPipe Extractor, yt-dlp); a persisted resolved-URL cache; precache of the next two songs; a background resolve at startup of the last played song and the first Home items (at most 3); later, in a separate PR, the crate `rquickjs` and the host `raw.githubusercontent.com` for a native extractor with a local JS cipher solver.

## User Stories

- As a listener, I want the first song after the app opens to start as fast as the later ones, so that opening the app and pressing Play feels instant.
- As a listener, I want a song I played in the last hours to start at once after a restart.
- As a new user, I want my first click to work even while the app is still installing its resolver.
- As the owner, I want a measuring example that shows every stage of a fresh process, so that I can see which cold cost is left on my machine.

## Acceptance Criteria

1. THE SYSTEM SHALL provide development examples (`cold_start` in `nemo-app`, `cold_net` in `nemo-ytm`) that run a fresh process through the app's own start-up wiring and one click on Play, print each stage with the time since the process start, and never print a URL, an id or a token.
2. WHEN the native player request and the stream open run, THE SYSTEM SHALL log at debug level, target `nemo::start`, the stages `native player request`, `native player retry with visitor data`, `http client build`, `http first window` and `yt-dlp install worker`, as fixed names with a time and an outcome only.
3. THE SYSTEM SHALL NOT wait for the yt-dlp installer or update check before a resolve. WHEN no yt-dlp program exists and the installer is running, a resolve that needs yt-dlp SHALL wait for the install (at most 90 s) instead of failing with "not installed".
4. WHEN the app has started, THE SYSTEM SHALL prepare in the background, after the first frame and never before the splash ends, what the first play needs: the HTTP client of the stream opener, and one run of `yt-dlp --version` to warm the disk cache. It SHALL run once, at human pace, with no request beyond what the first play makes anyway.
5. WHEN a stream was resolved less than its expiry minus 10 minutes ago, THE SYSTEM SHALL serve it from a private file in the app cache folder, also after a restart. The file is mode 0600, is never logged, is removed on sign-out and by the cache wipe, and a stream is checked with one cheap request before it is reused.
6. WHILE a song plays, THE SYSTEM SHALL resolve at most the two songs after it, one resolve at a time. At startup it SHALL resolve in the background the last played song and the first items of Home, at most 3 songs in all, one at a time.
7. WHEN the pointer presses a play button or a track row, or rests on a row for about 150 ms, THE SYSTEM SHALL start the resolve of that one song (one in flight at most, cancelled when the pointer leaves, never for a list).
8. WHEN the native path is resting or failed for the previous song, THE SYSTEM SHALL start yt-dlp at once and not after 1.5 s.
9. WHEN the audio code changes, THE SYSTEM SHALL keep the output samples bit-identical (`bit_exact`, `rt_no_alloc`).
10. HARD LIMITS (`coc.md`): no PO-token minting, no BotGuard or attestation emulation, no cookie or token sharing, no remote decoder that receives a session, no bulk download, human-paced requests, nothing that identifies a URL or an account in a log.

## Out of Scope

SoundCloud, lyrics, the player bar, the settings screens, the audio output code. The in-process extractor (acceptance criteria are added with its PR, only if the numbers show yt-dlp is still the slow path).

## Open Questions

- The owner's Windows machine is not available here: SChannel handshakes, Defender scans of a first yt-dlp run and the WASAPI device open are not measured. The Codespace IP gets the YouTube bot check for some videos, which affects every number that involves a refusal.
