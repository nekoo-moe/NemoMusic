# Live stream fixtures

From the youtube-extras spike (task 7, 2026-11-14; results in
`.kiro/specs/youtube-extras/design.md`, Risks).

- `live-formats.json`: the formats of a public 24/7 live stream as `yt-dlp` 2026.10.26 `-J`
  listed them, with every URL removed. Audio-only formats `233` and `234` have `vcodec` "none"
  and `acodec` `null`; the video formats are video-only.
- `live-playlist.m3u8`: the shape of the audio-only media playlist (format `234`): the head as
  served, segment URLs replaced, and one ad cue point pair whose context and identifier values
  are invented.
- `live-segment.aac`: a 2-second segment in the same packed-audio form the live audio segments
  use: an ID3v2.3 tag with a `PRIV` frame `com.apple.streaming.transportStreamTimestamp`
  (73 bytes, as served), then ADTS AAC-LC, 44.1 kHz, stereo, 128 kbps. Made for this repository
  instead of copying the stream's audio, which belongs to the channel: two sine tones (440 and
  660 Hz) encoded with FFmpeg `N-127043-g5a54fcf75e-20260930`:
  `ffmpeg -f lavfi -i sine=frequency=440:sample_rate=44100:duration=2 -f lavfi -i sine=frequency=660:sample_rate=44100:duration=2 -filter_complex "[0][1]amerge=inputs=2" -c:a aac -profile:a aac_low -b:a 128k -fflags +bitexact -flags:a +bitexact -f adts`,
  with the ID3 tag (timestamp 900000, 10 s on the 90 kHz clock) prepended. SHA-256
  `972a96d6d25c0a25faaf0110b98f6baf8588a9ff09dba2e3b217b3dbb9111779`. Licence: the
  repository's (`LICENSE`).
