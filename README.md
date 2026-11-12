<h1 align="center">NemoMusic</h1>

<p align="center">FOSS music streaming platform with fast perfomance, cross-platform native & easy to use.</p>

<div align="center">
  <img width="300" height="150" alt="badge1" src="https://github.com/user-attachments/assets/a3f8e948-470f-4b13-865b-3c304c159ab4" />
  <img width="300" height="150" alt="badge2" src="https://github.com/user-attachments/assets/c6138556-398f-466d-822e-d1e7933eb469" />
</div>

### 🎉 About NemoMusic

**NemoMusic** (or *Nimomyūjikku*) is a calm desktop music player for long listening sessions, written in Rust. It plays the YouTube Music catalog and your own files through an untouched output path, and it treats cover art, type and motion as part of the music.

Status: early development. Runs on Windows 10/11 and Linux (x86_64).

## What "lossless" means here

- YouTube Music serves lossy audio only (AAC or Opus, about 256 kbps at most). NemoMusic never labels a YouTube stream as lossless. The badge shows the real codec and bitrate.
- NemoMusic keeps the output path untouched: no resampling, gain, EQ or dither unless the device needs it or you turn it on. Any such step is shown in the signal-path view.
- True lossless applies only to local FLAC, ALAC and WAV files.

## Please read: risks

- NemoMusic is not made, approved or supported by Google or YouTube. It uses YouTube Music without an official API. This may break the YouTube Terms of Service. Google could limit or suspend the account you use, and access can stop working without notice.
- Ad blocking (off by default) means the app never requests or plays ads. Artists and rights holders earn less from those listens. It does not unlock Premium audio quality.
- SponsorBlock (off by default) skips non-music parts of music videos. Its data is CC BY-NC-SA 4.0, so NemoMusic stays non-commercial while it uses that data.

## Local only

NemoMusic runs entirely on your computer. The project runs no server and collects no telemetry. Sign-in data is kept in the OS credential store: Windows Credential Manager, or the Secret Service keyring on Linux (GNOME Keyring, KWallet). It is never written to a file. Your library, history and caches stay in `%LOCALAPPDATA%\NemoMusic` on Windows and `$XDG_DATA_HOME/NemoMusic` (usually `~/.local/share/NemoMusic`) on Linux. Signing out deletes them.

### Network egress list

These are the only hosts NemoMusic contacts. Adding a host needs owner approval.

| Host | Why | When |
|---|---|---|
| `music.youtube.com` | Search, browse, timed lyrics | Always (YouTube Music features) |
| `www.youtube.com` | Stream lookup by `yt-dlp` | When a YouTube track plays |
| `*.googlevideo.com` | The audio stream itself | When a YouTube track plays |
| `i.ytimg.com`, `*.ggpht.com`, `*.googleusercontent.com` | Cover art and artist pictures | When covers are shown |
| `lrclib.net` | Synced lyrics (line timing) | When lyrics are shown |
| `unison.betterlyrics.org` | Community lyrics (Unison), syllable timing, credited in the lyrics view | When lyrics are shown |
| `api.betterlyrics.org` | Better Lyrics API, syllable timing, cached answers only (no key), credited in the lyrics view | When lyrics are shown (on by default, can be switched off in settings) |
| `music.163.com`, `api-lyrics.simpmusic.org` | Extra lyrics sources, grey area, line timing | Only if you switch the source on in settings (off by default) |
| `sponsor.ajay.app` | SponsorBlock segments (only a 4-character hash prefix is sent) | Only when SponsorBlock is on |
| `accounts.google.com`, `*.gstatic.com` | Google sign-in page | Only while you sign in (not built yet) |

## Build and run

Needs Rust (pinned in `rust-toolchain.toml`) and [yt-dlp](https://github.com/yt-dlp/yt-dlp) on `PATH` for YouTube playback. Build `nemo-signin` too: the app looks for it next to `nemo-app`.

### Windows

Also needs Visual Studio Build Tools with the C++ workload.

```powershell
winget install --id yt-dlp.yt-dlp -e --scope user
cargo build -p nemo-signin --release
cargo run -p nemo-app --release
```

### Linux

Tested on Linux Mint 22.3 (Ubuntu 24.04 base), X11. Needs these system packages (Debian and Ubuntu names):

```bash
sudo apt install -y pkg-config libasound2-dev libgtk-3-dev libwebkit2gtk-4.1-dev \
  libsoup-3.0-dev libxdo-dev libssl-dev
sudo apt install -y yt-dlp   # or: pipx install yt-dlp
cargo build -p nemo-signin --release
cargo run -p nemo-app --release
```

Linux notes:

- Sign-in needs a running Secret Service (GNOME Keyring or KWallet). Without one, sign-in is off and nothing is saved.
- The sign-in window is WebKitGTK. Google may refuse it; "Paste cookie instead" still works.
- Sound goes through ALSA, so PipeWire or PulseAudio may resample. The signal-path view shows this as shared mode, never as bit-perfect. A direct exclusive path is planned later.

Checks (same on both systems):

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Credits

- Lyrics: [Unison](https://unison.betterlyrics.org) and the [Better Lyrics](https://better-lyrics.boidu.dev/) API (syllable timing), [LRCLIB](https://lrclib.net), and your own `.lrc` files; NetEase and SimpMusic only if you switch them on. The best timed result is picked automatically (syllable, then word, then line). The provider name is shown with every lyric.
- Skip data: [SponsorBlock](https://sponsor.ajay.app) by Ajay Ramachandran, [CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/), fetched live.

## License

GPL-3.0. See `LICENSE`. Community rules: `CODE_OF_CONDUCT.md`.
