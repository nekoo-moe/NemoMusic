# Test audio fixtures

Short files the integration tests read from disk. Each is under 200 KB (`structure.md`). Most
fixtures are written from code instead (`tests/common/fixtures.rs`); only formats the tests
cannot write themselves live here.

| File | What | Made with | Licence |
|---|---|---|---|
| `tone-aac.m4a` | 1 s, 440 Hz sine, stereo, 44.1 kHz, AAC-LC about 32 kbps in an MP4 (M4A) container, 4159 bytes, no metadata | `ffmpeg -f lavfi -i "sine=frequency=440:sample_rate=44100:duration=1" -ac 2 -c:a aac -b:a 32k -fflags +bitexact -flags:a +bitexact -map_metadata -1 tone-aac.m4a` (FFmpeg's native AAC encoder) | Generated test tone, no recorded material; CC0-1.0 |
