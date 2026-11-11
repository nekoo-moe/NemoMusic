# Playback start latency (measured 2026-11-11, owner's laptop)

Owner report: 5-10 s from click to sound; playlist pages also load slowly.

| Step | Time |
|---|---|
| `yt-dlp.exe --version` (process start only) | 4.2 s |
| search + resolve, current args | 11.8 s |
| search + resolve, `--extractor-args "youtube:player_client=android_vr;player_skip=webpage,configs"` | 6.8 s |

Causes:
1. The winget `yt-dlp.exe` is a PyInstaller one-file build: it unpacks itself on every run (~4 s).
2. Default extraction downloads the watch page and several player clients.
3. Every click starts a fresh process; nothing is cached or resolved ahead of time.

Plan for the next wave (`nemo-ytm`, `nemo-audio`):
- Use the yt-dlp "onedir" Windows build (`yt-dlp_win.zip`) unpacked under `%LOCALAPPDATA%\NemoMusic\bin`, or keep one long-lived worker; measure both.
- Add the faster extractor args above; keep the current args as a fallback when the fast path fails.
- Cache resolved stream URLs per video id until their `expire` time minus 10 minutes.
- Resolve ahead: the next track in the queue, and the first visible Quick picks rows on hover.
- Consider a native innertube `player` request with the ANDROID_VR client (no signature needed), yt-dlp as fallback. Same ToS risk as today.
- Playlist page bug: "could not parse catalog answer: unexpected answer from YouTube Music" when opening a playlist from Home (see `crates/nemo-ytm/src/raw.rs:130`, `catalog.rs:540`). Find the id type (radio `RDCLAK…`, mix, community playlist) and handle it.
