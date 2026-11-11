# NemoMusic

A calm desktop music player for long listening sessions, written in Rust. It plays the YouTube Music catalog and your own files through an untouched output path, and it treats cover art, type and motion as part of the music.

Status: early development (Windows first).

## What "lossless" means here

- YouTube Music serves lossy audio only (AAC or Opus, about 256 kbps at most). NemoMusic never labels a YouTube stream as lossless. The badge shows the real codec and bitrate.
- NemoMusic keeps the output path untouched: no resampling, gain, EQ or dither unless the device needs it or you turn it on. Any such step is shown in the signal-path view.
- True lossless applies only to local FLAC, ALAC and WAV files.

## Please read: risks

- NemoMusic is not made, approved or supported by Google or YouTube. It uses YouTube Music without an official API. This may break the YouTube Terms of Service. Google could limit or suspend the account you use, and access can stop working without notice.
- Ad blocking (off by default) means the app never requests or plays ads. Artists and rights holders earn less from those listens. It does not unlock Premium audio quality.
- SponsorBlock (off by default) skips non-music parts of music videos. Its data is CC BY-NC-SA 4.0, so NemoMusic stays non-commercial while it uses that data.

## Local only

NemoMusic runs entirely on your computer. The project runs no server and collects no telemetry. Sign-in data is kept in Windows Credential Manager. Your library, history and caches stay in `%LOCALAPPDATA%\NemoMusic`. Signing out deletes them.

### Network egress list

These are the only hosts NemoMusic contacts. Adding a host needs owner approval.

| Host | Why | When |
|---|---|---|
| `music.youtube.com` | Search, browse, timed lyrics | Always (YouTube Music features) |
| `www.youtube.com` | Stream lookup by `yt-dlp` | When a YouTube track plays |
| `*.googlevideo.com` | The audio stream itself | When a YouTube track plays |
| `i.ytimg.com`, `*.ggpht.com`, `*.googleusercontent.com` | Cover art and artist pictures | When covers are shown |
| `lrclib.net` | Synced lyrics | When lyrics are shown |
| `unison.betterlyrics.org` | Community lyrics (Unison), credited in the lyrics view | When lyrics are shown |
| `api.betterlyrics.org`, `music.163.com`, `api-lyrics.simpmusic.org` | Extra lyrics sources, grey area | Only if you switch the source on in settings (off by default) |
| `sponsor.ajay.app` | SponsorBlock segments (only a 4-character hash prefix is sent) | Only when SponsorBlock is on |
| `accounts.google.com`, `*.gstatic.com` | Google sign-in page | Only while you sign in (not built yet) |

## Build and run

Needs Rust (pinned in `rust-toolchain.toml`), Visual Studio Build Tools with the C++ workload, and [yt-dlp](https://github.com/yt-dlp/yt-dlp) on `PATH` for YouTube playback.

```powershell
winget install --id yt-dlp.yt-dlp -e --scope user
cargo run -p nemo-app --release
```

Checks:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Credits

- Lyrics: [LRCLIB](https://lrclib.net) and YouTube Music. The provider name is shown with every lyric.
- Skip data: [SponsorBlock](https://sponsor.ajay.app) by Ajay Ramachandran, [CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/), fetched live.

## License

GPL-3.0. See `LICENSE`. Community rules: `CODE_OF_CONDUCT.md`.
