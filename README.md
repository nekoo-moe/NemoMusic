<div align="center">
<h1>NemoMusic</h1>

A calm, FOSS desktop music player in Rust for YouTube Music and your own files,<br>with a bit-perfect output path, honest quality labels and Material 3 + Liquid Glass design.
<br><br>
<a href="https://github.com/nekoo-moe/NemoMusic/actions"><img src="https://github.com/nekoo-moe/NemoMusic/actions/workflows/ci.yml/badge.svg"></a>
<a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-blue"></a>
<img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux-informational">
<img src="https://img.shields.io/badge/rust-1.97-orange">
<br><br>
<img width="300" height="150" alt="badge1" src="https://github.com/user-attachments/assets/a3f8e948-470f-4b13-865b-3c304c159ab4" />
<img width="300" height="150" alt="badge2" src="https://github.com/user-attachments/assets/c6138556-398f-466d-822e-d1e7933eb469" />
</div>

> NemoMusic (*Nimomyūjikku*) is in early development. It runs on Windows 10/11 and Linux (x86_64). Plans and progress: [`docs/feature-wave-plan.md`](docs/feature-wave-plan.md) and [`docs/leftover-work.md`](docs/leftover-work.md).

## Features ✨

Working now:
- Play YouTube Music: search, albums, artists, playlists, queue, gapless playback
- Bit-perfect output on Windows (WASAPI exclusive) with a device picker and no resampling
- Honest quality badge and a signal-path view: real codec, bitrate, rate and depth
- Local files: FLAC, ALAC, WAV, AIFF, MP3, AAC, Vorbis and Opus, plus CUE sheets
- Google sign-in, stored in the OS credential store, with library, likes, playlists and history
- Synced lyrics from several providers (Unison, Better Lyrics, LRCLIB, your `.lrc` files), word by word where available, with a timing offset
- SponsorBlock and ad blocking, each off by default with its own switch
- Material 3 Expressive and Classic Now Playing styles, light and dark themes
- Keyboard shortcuts and media keys

Being built (feature wave, approved specs in `.kiro/specs/`):
- Ten-band equalizer, AutoEq profiles, delay, reverb and crossfade, all off by default (DSP code merged, UI next)
- ReplayGain and loudness scan for local files, off by default (merged, UI next)
- Offline cache: save songs, albums and playlists, plus capped smart downloads
- SoundCloud public tracks
- Search all of YouTube, channels, live stations, comments and dislikes
- Several accounts, preferred audio language, Premium quality (opt-in)
- Own synced lyrics editor, romanization, lyrics translation (opt-in AI)
- Listening stats, Last.fm scrobbling, Discord presence, OS media controls
- Playlist import from Exportify and Soundiiz CSV files
- Video playback through libmpv, Cast and DLNA, Listen Together rooms, song recognition
- Apple Music-style Now Playing

> [!WARNING]
> NemoMusic is not made, approved or supported by Google or YouTube. It uses YouTube Music without an official API, which may break the YouTube Terms of Service. Your account could be limited, and playback can break without notice.

## What "lossless" means here

- YouTube Music serves lossy audio only (AAC or Opus, about 256 kbps at most). NemoMusic never labels a YouTube stream as lossless. The badge shows the real codec and bitrate.
- NemoMusic keeps the output path untouched: no resampling, gain, EQ or dither unless the device needs it or you turn it on. Any such step is shown in the signal-path view.
- True lossless applies only to local FLAC, ALAC and WAV files, and the badge says "Lossless" only while the path is bit-perfect (exclusive mode, nothing changing the samples). Through the shared system mixer, a resampler or any effect, it shows the codec and format, and the signal-path view says why. On Linux the output is shared mode for now, so the badge stays off there.

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
| `accounts.google.com`, `*.gstatic.com` | Google sign-in page | Only while you sign in |

## Build and run

Needs Rust (pinned in `rust-toolchain.toml`) and [yt-dlp](https://github.com/yt-dlp/yt-dlp) on `PATH` for YouTube playback. Build `nemo-signin` too: the app looks for it next to `nemo-app`.

### Windows

Also needs Visual Studio Build Tools with the C++ workload, and CMake on `PATH` (the Opus decoder builds libopus from source). The Build Tools' "C++ CMake tools for Windows" component installs CMake under `Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin`; add that folder to `PATH`, or install CMake with `winget install --id Kitware.CMake -e`.

```powershell
winget install --id yt-dlp.yt-dlp -e --scope user
cargo build -p nemo-signin --release
cargo run -p nemo-app --release
```

### Linux

Tested on Linux Mint 22.3 (Ubuntu 24.04 base), X11. Needs these system packages (Debian and Ubuntu names):

```bash
sudo apt install -y pkg-config libasound2-dev libgtk-3-dev libwebkit2gtk-4.1-dev \
  libsoup-3.0-dev libxdo-dev libssl-dev cmake
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

### Third-party notices

Opus audio is decoded with libopus 1.6.1, built into NemoMusic through the `opusic-sys` crate. Its licence (BSD-3-Clause):

```text
Copyright 2001-2023 Xiph.Org, Skype Limited, Octasic,
                    Jean-Marc Valin, Timothy B. Terriberry,
                    CSIRO, Gregory Maxwell, Mark Borgerding,
                    Erik de Castro Lopo, Mozilla, Amazon

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions
are met:

- Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.

- Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

- Neither the name of Internet Society, IETF or IETF Trust, nor the
names of specific contributors, may be used to endorse or promote
products derived from this software without specific prior written
permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER
OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

Opus is subject to the royalty-free patent licences listed by the [Xiph.Org Foundation](https://datatracker.ietf.org/ipr/1524/), [Microsoft](https://datatracker.ietf.org/ipr/1914/) and [Broadcom](https://datatracker.ietf.org/ipr/1526/).
