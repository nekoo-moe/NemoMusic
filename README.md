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
- SponsorBlock and ad blocking, each on by default with its own switch
- Material 3 Expressive and Classic Now Playing styles, light and dark themes
- Keyboard shortcuts and media keys

Being built (feature wave, approved specs in `.kiro/specs/`):
- Ten-band equalizer, AutoEq profiles, delay, reverb and crossfade, all off by default (DSP code merged, UI next)
- ReplayGain and loudness scan for local files, off by default (merged, UI next)
- Offline cache: save songs, albums and playlists, plus capped smart downloads
- SoundCloud as a second listening area next to YouTube Music: search, fast playback, then your account (likes, playlists, following, history), opt-in. You choose the area in Your accounts or in Settings; the sidebar, Library and Search show one service at a time, and the song that plays keeps playing when you switch
- Search all of YouTube, channels, live stations, comments and dislikes
- Several accounts, preferred audio language, Premium quality (opt-in)
- Own synced lyrics editor, romanization, lyrics translation (opt-in AI)
- Listening stats, Last.fm scrobbling, Discord presence, OS media controls
- Playlist import from Exportify and Soundiiz CSV files
- Video playback through libmpv, Cast and DLNA, Listen Together rooms, song recognition
- Apple Music-style Now Playing

> [!WARNING]
> NemoMusic is not made, approved or supported by Google or YouTube. It uses YouTube Music without an official API, which may break the YouTube Terms of Service. Your account could be limited, and playback can break without notice.

> [!WARNING]
> SoundCloud is not an official integration either. NemoMusic uses the same web calls as soundcloud.com, without an official API key. SoundCloud may block this, may restrict your account if you sign in, and its terms may not allow it. When you sign in, a token for your SoundCloud session is kept in your computer's credential store and is sent only to SoundCloud. Plays here may not count in artists' SoundCloud statistics. SoundCloud audio is lossy (AAC or MP3); NemoMusic never calls it lossless. SoundCloud is off by default.

## What "lossless" means here

- YouTube Music serves lossy audio only (AAC or Opus, about 256 kbps at most). NemoMusic never labels a YouTube stream as lossless. The badge shows the real codec and bitrate.
- NemoMusic keeps the output path untouched: no resampling, gain, EQ or dither unless the device needs it or you turn it on. Any such step is shown in the signal-path view.
- True lossless applies only to local FLAC, ALAC and WAV files, and the badge says "Lossless" only while the path is bit-perfect (exclusive mode, nothing changing the samples). Through the shared system mixer, a resampler or any effect, it shows the codec and format, and the signal-path view says why. On Linux the output is shared mode for now, so the badge stays off there.

## Please read: risks

- NemoMusic is not made, approved or supported by Google or YouTube. It uses YouTube Music without an official API. This may break the YouTube Terms of Service. Google could limit or suspend the account you use, and access can stop working without notice.
- Ad blocking (on by default, switch it off in Settings) means the app never requests or plays ads. Artists and rights holders earn less from those listens. It does not unlock Premium audio quality.
- SponsorBlock (on by default, switch it off in Settings) skips non-music parts of music videos. It sends a 4-character hash prefix of the video id to `sponsor.ajay.app`. Its data is CC BY-NC-SA 4.0, so NemoMusic stays non-commercial while it uses that data.

## Local only

NemoMusic runs entirely on your computer. The project runs no server and collects no telemetry. Sign-in data is kept in the OS credential store: Windows Credential Manager, or the Secret Service keyring on Linux (GNOME Keyring, KWallet). It is never written to a file. Your library, history and caches stay in `%LOCALAPPDATA%\NemoMusic` on Windows and `$XDG_DATA_HOME/NemoMusic` (usually `~/.local/share/NemoMusic`) on Linux. Signing out deletes them.

### Network egress list

These are the only hosts NemoMusic contacts. Adding a host needs owner approval.

| Host | Why | When |
|---|---|---|
| `music.youtube.com` | Search, browse, timed lyrics | Always (YouTube Music features) |
| `www.youtube.com` | Stream lookup by `yt-dlp` and by the in-app resolver (the `player` request, `iframe_api`, and the player script of about 2.7 MB, kept for hours) | When a YouTube track plays |
| `*.googlevideo.com` | The audio stream itself | When a YouTube track plays |
| `i.ytimg.com`, `*.ggpht.com`, `*.googleusercontent.com` | Cover art and artist pictures | When covers are shown |
| `lrclib.net` | Synced lyrics (line timing) | When lyrics are shown |
| `unison.betterlyrics.org` | Community lyrics (Unison), syllable timing, credited in the lyrics view | When lyrics are shown |
| `api.betterlyrics.org` | Better Lyrics API, syllable timing, cached answers only (no key), credited in the lyrics view | When lyrics are shown (on by default, can be switched off in settings) |
| `music.163.com`, `api-lyrics.simpmusic.org` | Extra lyrics sources, grey area, line timing | Only if you switch the source on in settings (off by default) |
| `lyricsplus.prjktla.my.id` | LyricsPlus, an unofficial volunteer relay of Apple Music, Deezer and QQ Music lyrics, syllable timing, credited in the lyrics view. Sends title, artist, album and duration | Only if you switch the source on in settings (off by default) |
| `sponsor.ajay.app` | SponsorBlock segments (only a 4-character hash prefix is sent) | Only when SponsorBlock is on |
| `ws.audioscrobbler.com` | Last.fm API: now playing and scrobbles (title, artist, album, play time), over HTTPS only | Only when Last.fm is on |
| `accounts.google.com`, `*.gstatic.com` | Google sign-in page | Only while you sign in |
| `soundcloud.com` | The home page, to find the web app's client id; the sign-in page while you sign in (SoundCloud's own page in the sign-in window may load more of its own files and a bot check from the providers it uses, which the app does not control) | Only when SoundCloud is on |
| `api-v2.soundcloud.com` | Search, track, playlist and artist pages, stream lookup, and your account (likes, playlists, following, history) when signed in | Only when SoundCloud is on |
| `a-v2.sndcdn.com` | The web app's script that holds the client id | Only when SoundCloud is on |
| `i1.sndcdn.com` | Cover art and avatars | Only when SoundCloud is on |
| `playback.media-streaming.soundcloud.cloud`, `*.sndcdn.com` | The audio stream itself (HLS segments, previews, older MP3 files) | Only when a SoundCloud track plays |
| `github.com`, `release-assets.githubusercontent.com` | Download the official [yt-dlp](https://github.com/yt-dlp/yt-dlp) release and its SHA-256 list | Only to install or update the app's own yt-dlp (automatic install on, no `yt_dlp_path` set); at most one update check a day |

## Build and run

Needs Rust (pinned in `rust-toolchain.toml`). YouTube playback uses [yt-dlp](https://github.com/yt-dlp/yt-dlp): the app downloads the official release into its own data folder (`bin/yt-dlp`) on Windows x64 and Linux x86_64, checks it against the published SHA-256 list and keeps that copy up to date. A yt-dlp on `PATH` or from winget is used only until that copy is in place, because distribution packages are often too old for YouTube. It never changes a yt-dlp you installed. To use your own copy, set `yt_dlp_path`. Set `"yt_dlp_auto_install": false` in `settings.json` to turn this off; on other platforms install yt-dlp yourself. Songs that YouTube's plain request refuses are first solved inside the app (an embedded JavaScript engine runs the signature and `n` functions of YouTube's player script; no extra program, no PO token), and only then handed to yt-dlp. Set `"in_process_resolver": false` in `settings.json` to skip that step (default `true`). If a network refuses the streams it builds three times in one session, the app switches the step off for 24 hours by itself, also across restarts (a one-number marker file, `cache/in-process-off.json` in the data folder, deleted when the day is over; deleting it earlier turns the step back on), and logs one `info` line when it does. NemoMusic is one program: the sign-in window is built into `nemo-app` and runs as a second `nemo-app --signin` process while you sign in.

### Windows

Also needs Visual Studio Build Tools with the C++ workload, and CMake on `PATH` (the Opus decoder builds libopus from source). The Build Tools' "C++ CMake tools for Windows" component installs CMake under `Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin`; add that folder to `PATH`, or install CMake with `winget install --id Kitware.CMake -e`.

```powershell
cargo run -p nemo-app --release
```

With Window glass on (Settings, Appearance), Windows 11 and Windows 10 1809 or newer blur what is behind the window (Mica or acrylic). The system does it; nothing is downloaded and no host is contacted.

### Linux

Tested on Linux Mint 22.3 (Ubuntu 24.04 base), X11. Needs these system packages (Debian and Ubuntu names):

```bash
sudo apt install -y pkg-config libasound2-dev libgtk-3-dev libwebkit2gtk-4.1-dev \
  libsoup-3.0-dev libxdo-dev libssl-dev cmake
cargo run -p nemo-app --release
```

Linux notes:

- Sign-in needs a running Secret Service (GNOME Keyring or KWallet). Without one, sign-in is off and nothing is saved.
- The sign-in window is WebKitGTK. Google may refuse it; "Paste cookie instead" still works. SoundCloud's sign-in with Google, Apple or Facebook uses pop-up windows and may not work in the sign-in window; use SoundCloud's e-mail form there, or paste your `oauth_token` instead. If the SoundCloud window shows you signed in but does not close, wait for "I am signed in, check again" or paste your token; to report it, start the app with `NEMO_SIGNIN_DEBUG=1` (names, flags and lengths of the window's cookies go to the log, never values).
- `nemo-app` links GTK 3, WebKitGTK 4.1 and libsoup 3 at load time, so their runtime libraries must be installed for the app to start at all, even if you never sign in.
- Sound goes through ALSA, so PipeWire or PulseAudio may resample. The signal-path view shows this as shared mode, never as bit-perfect. A direct exclusive path is planned later.

Checks (same on both systems):

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Credits

- Lyrics: [Unison](https://unison.betterlyrics.org) and the [Better Lyrics](https://better-lyrics.boidu.dev/) API (syllable timing), [LRCLIB](https://lrclib.net), and your own `.lrc` files; NetEase, SimpMusic and [LyricsPlus](https://github.com/ibratabian17/LyricsPlus) (syllable timing; an unofficial relay of Apple Music, Deezer and QQ Music lyrics that can stop without notice) only if you switch them on. The best timed result is picked automatically (syllable, then word, then line). The provider name is shown with every lyric.
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
