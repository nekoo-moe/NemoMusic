# Caption fixtures

Invented for the youtube-extras captions spike (task 6). A real song's captions are its lyrics,
so none are kept here: the text is made up, and only the shape copies what `yt-dlp` 2026.10.26
returned on 2026-11-14 (results in `.kiro/specs/youtube-extras/design.md`, Risks). They carry no
URL, cookie or token. Licence: the repository's (`LICENSE`).

- `uploaded.json3`: an uploaded track. One event per line, `segs` with `utf8` only, a `[♪♪♪]`
  sound tag, note symbols around lines, a line break inside one event, an empty event, Vietnamese
  and Japanese text.
- `auto-orig.json3`: the original auto-generated track (`<code>-orig`). A window event without
  `segs`, roll-up events with per-word `tOffsetMs` and `acAsrConf`, `aAppend` events that only add
  a line break, and `[Music]` tags.
- `tracks.json`: the caption fields of a `-J` answer for a music video and for the matching audio
  track (which has none), with the observed track names.
