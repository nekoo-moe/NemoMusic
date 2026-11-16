# nemo-lyrics test fixtures

Recorded answers, cut down so that only their shape and timing are kept. Lyrics are copyrighted
(coc.md), so every lyric text here is invented, and titles, artists, albums and ids are
replaced with test values.

| File | Source | Recorded | What was kept |
|---|---|---|---|
| `lyricsplus-v2-word.json` | LyricsPlus `GET /v2/lyrics/get` (`lyricsplus.prjktla.my.id`), a cached Apple Music answer | 2026-11-15 | Field names and nesting, `type`, `cached`, `processingTime`, and the start and duration of three lines and their pieces (one with background-vocal pieces). The text is invented Vietnamese counting words, three lines of a 94-line answer. |
| `lyricsplus-404-skipped.json` | Same host, a miss while the Apple Music source was skipped | 2026-11-15 | The whole error body, with the song replaced by test values. |
| `lyricsplus-404-miss.json` | LyricsPlus `lyricsplus.binimum.org`, a miss where every source answered `BAD` | 2026-11-15 | The whole error body, with the song replaced by test values. |
