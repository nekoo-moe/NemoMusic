# Test audio fixtures

Short files the integration tests read from disk. Each is under 200 KB (`structure.md`). Most
fixtures are written from code instead (`tests/common/fixtures.rs`); only formats the tests
cannot write themselves live here.

| File | What | Made with | Licence |
|---|---|---|---|
| `tone-aac.m4a` | 1 s, 440 Hz sine, stereo, 44.1 kHz, AAC-LC about 32 kbps in an MP4 (M4A) container, 4159 bytes, no metadata | `ffmpeg -f lavfi -i "sine=frequency=440:sample_rate=44100:duration=1" -ac 2 -c:a aac -b:a 32k -fflags +bitexact -flags:a +bitexact -map_metadata -1 tone-aac.m4a` (FFmpeg's native AAC encoder) | Generated test tone, no recorded material; CC0-1.0 |
| `opus-surround.opus` | 0.1 s, 440 Hz sine in three channels (3.0 layout), 48 kHz, Opus about 48 kbps in Ogg, 912 bytes, no metadata. The Opus decoder takes one or two channels, so this file must fail to build a decoder and play nothing | `ffmpeg -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=0.1" -af "pan=3.0\|FL=c0\|FR=c0\|FC=c0" -c:a libopus -b:a 48k -map_metadata -1 -fflags +bitexact -flags:a +bitexact opus-surround.opus` (FFmpeg `N-127043-g5a54fcf75e-20260930` with libopus) | Generated test tone, no recorded material; CC0-1.0 |
| `autoeq/ten-band.txt` | Equalizer profile in the `AutoEq` fixed-band text format: a preamp and ten peak filters at 31 Hz to 16 kHz, Q 1.41 | Written by hand for the parser tests; the gains are made up, not a measurement | CC0-1.0 |
| `autoeq/parametric.txt` | Equalizer profile in the `AutoEq` parametric text format: a preamp, a low shelf, four peaks and a high shelf | Written by hand for the parser tests; the values are made up, not a measurement | CC0-1.0 |
| `autoeq/refused-low-pass.txt` | Equalizer profile with a low-pass filter on line 3, which the equalizer cannot represent and refuses | Written by hand for the parser tests | CC0-1.0 |

The `autoeq/` files follow the text format of the AutoEq project (MIT-licensed code,
<https://github.com/jaakkopasanen/AutoEq>) but copy none of its results: a measurer asked that
its data not be published, and the app bundles no measurement data (audio-effects criterion 12).
