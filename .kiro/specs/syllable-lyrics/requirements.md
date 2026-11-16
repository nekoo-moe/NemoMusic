Spec pre-approved by the owner on 2026-11-15 (UI polish wave, design delegated; decisions below).

Owner decisions (2026-11-15):
- Make syllable (word-level) lyrics better: find the provider with the most syllable-synced lyrics and add it to the lyrics chain, following ADR 0002 and the lyrics-plus spec.
- The new host is approved for this request. It goes on the README egress list, in ADR 0002 and in Settings > Lyrics in the same change.
- A source that is unofficial or grey-area is off by default; only an official, open, documented API is on by default.
- No token or cookie in the repo; any key the user supplies goes through `SecretStore`. Requests are human-paced: one request per song, cached, backoff on 429.
- Owner override, 2026-11-15: ADR 0002's update of 2026-11-11 turned on "every provider that can give syllable or word timing". For grey sources this request wins: the new syllable source is off by default because it is unofficial.

## Goal

When the user turns on the new source, more songs, above all Vietnamese pop and rap, K-pop and Western pop, show lyrics that light up word by word or syllable by syllable instead of line by line. The source is named under the lyrics with the service it came from, it is off until the user turns it on, and Settings says plainly what it is and what it sends.

## User Stories

- As a listener of Vietnamese pop and K-pop, I want syllable-timed lyrics for more of my songs, so that the karaoke-style highlight works on the songs I play.
- As a careful user, I want a grey-area source to stay off until I turn it on and to say its risk, so that nothing unofficial is asked behind my back.
- As a listener, I want to see where the lyrics came from, so that the source and the original service get credit.
- As a listener on a slow day for the source, I want a temporary failure to be retried later, so that a song does not lose its lyrics for a week.

## Acceptance Criteria

1. THE SYSTEM SHALL offer a `LyricsPlus` lyrics provider in `nemo-lyrics` that reports `Granularity::Syllable` as its finest level and asks only `lyricsplus.prjktla.my.id`.
2. WHEN no choice is stored, THE SYSTEM SHALL keep `LyricsPlus` off and show it in Settings > Lyrics with its host, "Off until you turn it on" and a one-line risk note.
3. WHEN `LyricsPlus` is on and a song plays, THE SYSTEM SHALL send at most one request for that song (title, first artist, album and duration when known, and the fixed upstream list) and SHALL store the answer in the lyrics cache like every other provider.
4. WHEN the answer has timed pieces, THE SYSTEM SHALL map each piece to one `LyricWord`, keep its trailing space, and label the document `Syllable` when a piece splits a word, else `Word`.
5. WHEN a line has a piece the server marked `synthetic` (timing spread by letter count), THE SYSTEM SHALL keep only that line's line timing.
6. WHEN the answer is labelled `Line` or `None`, THE SYSTEM SHALL NOT read word timing from it.
7. WHEN a line has background-vocal pieces, THE SYSTEM SHALL show them as a separate background line right after the main line, flagged in `LineMeta`; singers and transliteration go to `LineMeta` too.
8. WHEN the server names the song it matched, THE SYSTEM SHALL score it like other providers and SHALL drop an answer that does not score as acceptable.
9. WHEN `LyricsPlus` and LRCLIB both answer for the same song with acceptable scores, THE SYSTEM SHALL pick the syllable or word answer over the line answer.
10. WHEN a `LyricsPlus` document is shown, THE SYSTEM SHALL show "LyricsPlus" in the footer and the credit "Lyrics via LyricsPlus, from <upstream service>".
11. WHEN the server answers 404 and some upstream source was skipped or timed out, THE SYSTEM SHALL treat it as a temporary error (cached for minutes); a 404 where every source answered `BAD` is a miss.
12. WHEN the server answers 429 or 503, THE SYSTEM SHALL honour `Retry-After` through the existing error cache.
13. THE SYSTEM SHALL keep no key, cookie or token for this source; tests use recorded fixtures with invented lyric text only.

## Out of Scope

- LRCLIB word timing from `lyricsfile` YAML (no YAML parser in the stack; LRCLIB word-synced records are rare).
- The other LyricsPlus instances (`lyricsplus.binimum.org`, `lyricsplus.prjktla.workers.dev`) and server rotation.
- Uploading lyrics to LyricsPlus (its PoW `submit` endpoint).
- NetEase `yrc`, QQ `qrc`, KuGou `krc`, Musixmatch richsync and Spotify (see design, Risks and Alternatives).
- Changing the lyrics view, the footer layout or the snapshots mock.

## Open Questions

- Should the source list sent to LyricsPlus drop `qq` and `deezer` and keep `apple,lyricsplus` only? QQ and Deezer are reached by the server with its own accounts, like Apple Music.
- The YouLy+ extension source marks its server list with "plz dont stole"; LyricsPlus states no terms for other apps. Should the owner ask the maintainer before this source may be turned on by default, or before it ships at all?
