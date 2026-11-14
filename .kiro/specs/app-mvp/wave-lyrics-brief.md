# Wave: lyrics quality and song-coloured canvas (2026-11-11)

Owner feedback (live use, screenshots 19:44 to 19:53):
- Long lyric lines are cut with an ellipsis ("...tâm hồn tôi là..."), in the right pane and full screen.
- Wants a Better Lyrics style source preference: syllable first, then word, then line. Turn on by
  default every provider that gives syllable timing, then word timing.
- The move to the next line has no animation.
- Remove the provider chip row under the lyrics. Pick the best provider automatically; put a quick
  "switch to another top source" control in the Tools panel.
- The Now Playing box (right pane) and the full-screen Now Playing do not take the song's colours
  (full screen is plain black in AMOLED mode with the Sakura theme).

## Owner decisions recorded here

1. Preference order: Syllable > Word > Line > Unsynced. Within a level: match score, then chain
   order. The best result is picked automatically; a per-track manual pick still wins.
2. Default ON for every provider that can return syllable or word timing (owner, 2026-11-11).
   A grey-area provider that can only give line timing stays opt-in (off by default).
3. Better Lyrics API (`api.betterlyrics.org`) is now an open source: its docs say it is free and
   may be used in other apps; cached answers need no key; it asks for a link to Better Lyrics
   (<https://lyrics-api-docs.boidu.dev/>, <https://docs.betterlyrics.org/docs/authentication>,
   read 2026-11-11). Default ON, credit "Lyrics via Better Lyrics" with the link. Never send or
   ask for an API key. Send all four parameters (s, a, al, d).
4. Still rejected: Musixmatch (user token, terms), the Better Lyrics extension backend
   (Turnstile is an access control), any source that needs Apple Music or other account tokens
   (BiniLyrics if it does), direct QQ/KuGou endpoints. No request signing is faked, ever.
5. No new hosts in this wave. All hosts used are already in the README egress list.

## Shared contract (already in `nemo-core`, committed 9296370 and 7c423c8)

- `Granularity::Syllable` (above `Word`). Syllable pieces are `LyricWord`s whose `text` keeps its
  own whitespace; pieces of one word have no space between them. Join texts without a separator.
- `LyricsProvider::best_granularity()` (default `Line`) and `LyricsProviderStatus::best`.
- Unchanged: `lookup`, `lookup_all` (best first), `choose`, `chosen`, `clear_choice`, `credit`,
  `providers`, `set_provider_enabled`.

## Acceptance criteria

Lyrics data (agent L):
1. WHEN a TTML document has spans that split one word into several timed pieces (a span with no
   trailing whitespace followed by another span of the same word), or the source says
   `syncType: syllable`, THE SYSTEM SHALL mark the doc `Syllable`; timed whole words give `Word`.
2. WHEN several providers answer, THE SYSTEM SHALL return them best first by Syllable > Word >
   Line > Unsynced, then score, then chain order.
3. WHEN no user choice is stored, THE SYSTEM SHALL have every syllable- or word-capable provider
   on, Better Lyrics API included; line-only grey providers stay off.
4. WHEN NetEase or SimpMusic can give word or syllable timing through a public, unsigned request
   whose format can be verified (public docs or the provider's own open-source client), THE
   SYSTEM SHALL read it and report `best_granularity` accordingly; otherwise leave them as they
   are and say why in the report.
5. Each provider reports a true `best_granularity`.

UI (agent U):
6. WHEN a lyric line is wider than the lyrics column, THE SYSTEM SHALL wrap it onto more lines and
   never cut it with an ellipsis (right pane, full screen, compact). Scroll anchoring uses the
   wrapped heights.
7. WHEN the active line changes, THE SYSTEM SHALL glide the list so the new line sits at the
   anchor (35% of the viewport) with the `spatial.default` spring, and cross-fade size and colour
   of the old and new active lines. Under 400 ms. Reduced motion: jump.
8. WHEN the doc is Syllable or Word timed, THE SYSTEM SHALL colour each piece by its own timing:
   sung pieces `on-surface` at 0.9, the piece being sung fades from 0.55 to full over its own
   duration (at least 100 ms), coming pieces 0.55. Pieces of one word stay joined. No bouncing,
   no hard-edged sweep. Line-timed lyrics keep the line highlight.
9. THE SYSTEM SHALL NOT show the provider chip row in the right pane or the full-screen view.
   The footer keeps `Lyrics: <provider> · <level>` and the credit line (Unison submitter,
   Better Lyrics link). Level text: `Syllable synced`, `Word synced`, `Synced`, `Not synced`.
10. WHEN the Tools panel opens and lyrics candidates exist, THE SYSTEM SHALL show a "Lyrics
    source" section: `Best (auto)` (clears the per-track pick) and each candidate with a level
    tag, the current one marked. Keyboard reachable.
11. Settings > Lyrics lists providers sorted by `best` (Syllable, Word, Line) with a level tag,
    on/off switch and risk note, like the Better Lyrics "Sources" screen.
12. THE SYSTEM SHALL paint the right Now Playing pane and the full-screen Now Playing with a
    canvas from the playing song's cover (the pre-blurred texture with stronger colour, or a
    multi-stop gradient of cover tones), in every base mode including AMOLED and with any theme.
    It cross-fades in 400 ms on song change, is static (no loop), keeps all text at 4.5:1 or more
    (scrim rule), and stays neutral grey for a grey cover (no invented colour). Tier Low only:
    no live blur.

## Rules for both agents

- Read `.kiro/steering/*.md` (code.md, structure.md, coc.md; agent U also design.md and
  `docs/ui-redesign.md`).
- Do not run the GUI app and do not move the mouse or keyboard: the owner is using the PC.
  Verify with unit tests and headless snapshots only.
- No new dependencies. No `unwrap`/`expect` outside tests. Files under 500 lines, functions under
  60 lines; split files that grow past that.
- Tests never call real services; use synthetic fixtures with no tokens.
- Do not commit. Report changed files, test counts and anything left undone.

## Agent L task details (crates/nemo-lyrics)

1. ttml.rs: `Syllable` when spans split a word into several timed pieces (a timed span with no
   trailing whitespace followed in the same line by another timed span, no whitespace text node
   between them); whole-word spans give `Word`. Keep piece whitespace exactly. Synthetic tests
   for syllable, word, line, unsynced and background-vocal lines.
2. unison.rs and blapi.rs: map the documented `syncType` values and the TTML result to the right
   level; `best_granularity` = `Syllable`.
3. blapi.rs: decision 3. `ProviderInfo::OPEN`, no key ever, always send s, a, al, d (whole
   seconds), 401 means "not cached, no lyrics", credit "Lyrics via Better Lyrics" with url
   <https://better-lyrics.boidu.dev/>. Update module docs with the sources above.
4. pick/chain/service: confirm the order for `lookup` and `lookup_all`, tests with `Syllable`.
   A stored per-track pick still wins. Defaults apply only when the user has no stored on/off
   choice: check how on/off is persisted so a changed default reaches users who never touched it.
5. True `best_granularity` for every provider: LRCLIB `Line` unless <https://lrclib.net/docs.txt>
   documents a word-timed field you then parse; local `.lrc`/tags `Word` only if enhanced-LRC word
   tags are parsed; YTM `Line`.
6. NetEase: is word-timed `yrc` available from a public unsigned request (for example
   `/api/song/lyric/v1` with `yv`/`ytv`)? At most 5 manual probe requests in total, several
   seconds apart, no cookies, tokens, signing or weapi/eapi encryption. If yes and the format is
   clear: parse it (Word, or Syllable if pieces split words), default ON, keep the risk note.
   If not: leave it Line and opt-in, and say why.
7. SimpMusic: learn the `richSyncLyrics` format from public docs or SimpMusic's own open-source
   client (learn the format only, do not copy GPL code). If verified: parse, set level, default
   ON with the risk note. Else unchanged, say why.
8. Optional, last: the documented `/kugou/getLyrics` and `/qq/getLyrics` on api.betterlyrics.org
   (no key, same host), only if the docs give the response format. Ids `KuGou via Better Lyrics`
   and `QQ Music via Better Lyrics`, risk note naming the upstream, default ON only with word or
   syllable timing.
9. Rejected list in decision 4. Check BiniLyrics: reject if it has no public token-free API.
10. README egress rows follow the results; ADR 0002 gets an "Update 2026-11-11" section.

## Agent U task details (crates/nemo-ui)

Code to read: `gui/views/pane.rs` (right pane lyrics), `now_playing.rs` (full screen),
`panels.rs` (`tools_panel`, provider chip row near the `Granularity` match), the settings lyrics
page, `gui/app.rs` (lyrics state, cover, `prev_cover`, `bg_fade`), `gui/textfit.rs`,
`gui/style.rs`, `gui/fx.rs`, `cover.rs`, `scheme.rs`, `motion.rs`, `mock.rs`, `gui/headless.rs`.

- 6: wrap; scroll anchoring from measured wrapped heights (textfit).
- 7: keep manual-scroll pause and the `Back to current line` pill.
- 8: iced rich text spans. This extends design.md section 10 (per-piece fade over the piece's
  duration instead of a fixed 100 ms step), owner request; note it in `docs/ui-redesign.md`.
- 10: `Best (auto)` calls `clear_choice`; targets at least 32 px.
- 11: update `mock.rs` with real-looking `best` values and a mock Syllable doc for snapshots.
- 12: the colour work runs on the worker thread in `cover.rs`. Must render on wgpu and
  tiny-skia (tiny-skia cannot round-clip images, see `bar.rs`). Reuse the existing crossfade.
  Contrast tests over all-black, all-white, grey, saturated and noisy covers.
- Snapshots (headless, tiny-skia) to
  `$env:KIROCREW_SCRATCH\wave-lyrics\` (set `NEMO_SNAPSHOT_DIR`): right pane with a
  long wrapped active line; full screen with the canvas in AMOLED + Sakura; tools panel with the
  Lyrics source section; settings lyrics list. Look at each PNG and fix what looks wrong.
