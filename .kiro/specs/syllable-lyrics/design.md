Spec pre-approved by the owner on 2026-11-15 (design delegated).

## Overview

Research (below, Risks and Alternatives) compared every syllable-capable source the owner named. The source with the most syllable- and word-timed lyrics that needs no account, cookie or key is LyricsPlus, the lyrics relay behind the YouLy+ extension. It serves Apple Music syllable TTML (converted to JSON) from its own cache and, when its own upstream accounts work, live from Apple Music, its user-submitted database, Deezer and QQ Music. It is unofficial, so it ships off by default with a risk note.

What exists before this change (read from `crates/nemo-lyrics/src/` on 2026-11-15):

| Provider | Module | Finest level | Default |
|---|---|---|---|
| Local file (enhanced LRC, tags) | `local.rs` | Syllable | On |
| Unison (Better Lyrics community) | `unison.rs` | Syllable (TTML `richsync`) | On |
| LRCLIB | `lrclib.rs` | Line (`syncedLyrics`; `lyricsfile` word timing not read) | On |
| Better Lyrics API | `blapi.rs` | Syllable (TTML, cached answers only, no key) | On |
| SimpMusic | `simpmusic.rs` | Line | Off (grey) |
| NetEase | `netease.rs` | Line (`yrc` is not read: it needs the encrypted `eapi`) | Off (grey) |

So three sources can give syllable timing today (local, Unison, Better Lyrics API). Unison is community-made and small; Better Lyrics API answers only what is already in its cache. NetEase YRC is not used.

The new provider:
- `GET https://lyricsplus.prjktla.my.id/v2/lyrics/get?title=&artist=&album=&duration=&source=apple,lyricsplus,deezer,qq`. Album and duration are sent when known. `source` is the server's own default order, sent so a server change cannot widen it. `forceReload` is never sent.
- The V2 JSON is parsed by a new parser (`lyricsplus/parse.rs`). TTML is not reused: the server's `/v1/ttml/get` conversion would hide the `synthetic` flag, and the JSON carries the same facts (pieces, background vocals, singers, transliteration).
- Chain position: after Better Lyrics API, before SimpMusic and NetEase. Order only breaks ties; `pick_best` already ranks Syllable > Word > Line > Unsynced, then score, then order, so an acceptable LyricsPlus answer beats an LRCLIB line answer for the same song (criterion 9).
- Settings > Lyrics lists it from `LyricsService::providers()`: "LyricsPlus", tag "Syllable", "Asks lyricsplus.prjktla.my.id. Off until you turn it on. Risk: ...". `settings_lyrics.rs` builds rows from that list, so it needs no new code; a test pins the row text.
- Off by default because it is grey: the service says its lyrics are "distributed unofficially", it reaches Apple Music, Deezer and QQ Music with its operators' own accounts, and it states no terms for other apps. It is not an official, open, documented API in the owner's sense. Owner override, 2026-11-15: this departs from ADR 0002's 2026-11-11 rule "turn on every provider that can give syllable or word timing" for grey sources.

## Affected Crates

- `nemo-lyrics`: new `src/lyricsplus.rs` (provider, query), `src/lyricsplus/parse.rs` (answer and 404 parser), `src/lyricsplus/tests.rs`; `src/http.rs` gains `send_any` (keeps a 4xx body); `src/service.rs` adds the provider to the default chain; `src/lib.rs`; `tests/providers_smoke.rs` (ignored real-network smoke); `tests/fixtures/` (recorded, cut-down answers).
- `nemo-ui`: `src/gui/views/settings_lyrics.rs`, one test only.
- `nemo-app`: no change. `LyricsHub::new` builds the chain, and `backend.rs` passes no extra providers.
- `nemo-core`: no change (existing `LyricsProvider`, `LyricsDoc`, `LyricWord`, `LineMeta`, `LyricsCredit`).
- Docs: README egress list, ADR 0002 addendum.

## Data Flow

- The lookup runs on the app's tokio runtime through `LyricsHub`, as for every provider. The chain gives LyricsPlus its default 4 s timeout, capped by the 6 s global budget, and one request in flight per host. Nothing runs on the audio thread and no audio buffer is touched.
- One request per song: a hit is cached for 30 days, a miss for 7 days, an error for 15 minutes or the server's `Retry-After` if longer. A timeout is not cached.
- Sent: title, first artist, album and duration when known, and the fixed source list. Received: lyrics, the matched song's title, artist, album and duration, and which upstream won. Nothing else leaves the machine.

## Interfaces

```rust
// nemo-lyrics/src/lyricsplus.rs
pub const LYRICSPLUS_ID: ProviderId;            // "LyricsPlus"
pub const CREDIT_URL: &str;                      // https://github.com/ibratabian17/LyricsPlus
pub const SOURCES: &str;                         // "apple,lyricsplus,deezer,qq"
pub fn query_params(query: &LyricsQuery) -> Option<Vec<(&'static str, String)>>;
pub struct LyricsPlusProvider { /* reqwest::Client */ }
impl LyricsPlusProvider { pub fn new() -> Result<Self, LyricsError>; }
impl LyricsProvider for LyricsPlusProvider { /* id, hosts, fetch, fetch_credited, fetch_detailed,
    info = ProviderInfo::opt_in(..), best_granularity = Syllable, default timeout */ }

// nemo-lyrics/src/lyricsplus/parse.rs (re-exported)
pub fn parse_response(body: &str) -> Result<Option<DetailedDoc>, LyricsError>;
pub fn miss_is_temporary(body: &str) -> bool;
pub fn upstream_name(raw: &str) -> Option<String>;
pub fn credit(source: Option<&str>, curator: Option<&str>) -> LyricsCredit;

// nemo-lyrics/src/http.rs
pub async fn send_any(req: RequestBuilder) -> Result<(u16, String), LyricsError>;
```

Mapping: `lyrics[].time`/`duration` (ms) give the line start and end; each `syllabus[]` piece gives one `LyricWord` with its text as sent (trailing space kept). The main line is the non-background pieces; background pieces become their own line after it with `LineMeta.background`. `element.isBackground` makes the whole line background. `element.singer` is numbered from 0 in order of first appearance. `transliteration.text` goes to `LineMeta.roman`. `processingTime.selectedSongMetadata` (else `metadata`) gives `MatchedMeta`. Lines are sorted stably by start, as the TTML parser does.

## Error Handling

- 200: parsed. A body that is not the answer shape is `LyricsError::Parse`; an answer with no non-empty line is no lyrics.
- 404: no lyrics, unless `processingTime.sourcesStatus` has any status other than `BAD` (`SKIP`, `RTO`, `WAIT`, `CANCEL`): then `LyricsError::Unavailable`, cached 15 minutes, because the server did not really look.
- 400, 403, 414: no lyrics (the server refuses the query).
- 429, 503: `LyricsError::RateLimited` with `Retry-After` (shared `http::status_error`). Other 5xx: `LyricsError::Network`.
- URLs never appear in errors or logs (`without_url`). A line dropped to line timing is counted at `debug` level only, with no text.

## Testing Strategy

- Unit tests in `src/lyricsplus/tests.rs` on recorded answers from 2026-11-15, cut to three lines with invented Vietnamese counting words (`tests/fixtures/lyricsplus-v2-word.json`, `lyricsplus-404-skipped.json`, `lyricsplus-404-miss.json`; origin in `tests/fixtures/README.md`), plus small synthetic answers for split words, `synthetic` pieces, `Line` and `None` labels, singers, transliteration and a background-only line.
- Ranking: the recorded answer scores as acceptable for its song and beats a line answer in `pick_best`; it is rejected for another song.
- `service::tests::provider_list_and_choices_persist`: the default chain lists LyricsPlus off with a risk note.
- `nemo-ui` test: the settings row sorts with the syllable sources and names host, default and risk.
- `tests/providers_smoke.rs` (ignored, run by hand) now also probes LyricsPlus.
- No audio code is touched, so no bit-exact test applies.

## Risks and Alternatives

### Research table (2026-11-15)

Coverage notes for Vietnamese (VN), K-pop and Western pop are from public documentation and the five probes listed below; "unverified" means not measured.

| Candidate | Syllable timing and coverage | Access | Token or account | Rate limit | Licence or terms | Hosts | Verdict |
|---|---|---|---|---|---|---|---|
| Apple Music TTML via LyricsPlus (`YouLy+` backend) | Apple syllable TTML as JSON pieces; also its user-submitted DB, Deezer word lines, QQ word lines. Probe: a VN pop hit came back word-timed from Apple (94 lines, all timed, 0 spread). K-pop and VN rap probes missed while the server skipped Apple. Western pop: Apple's catalogue, unverified per song | `GET /v2/lyrics/get`, documented in `docs/endpoints.md` and OpenAPI | None for clients; the server uses its own upstream accounts | 20 requests per 10 s per IP (server default) | Code Apache-2.0. README: lyrics "distributed unofficially", no right to reproduce. No terms for other apps; the extension's server list is commented "plz dont stole" | `lyricsplus.prjktla.my.id` (also `lyricsplus.binimum.org`, `lyricsplus.prjktla.workers.dev`) | **Picked**, opt-in, off by default |
| Apple Music TTML via Better Lyrics API | Syllable TTML (lrc.red cache). Probe: the same VN pop song is cached | `GET /getLyrics` | Cache misses need a key that is not issued; cached answers need none | Unstated | Free for other apps with credit | `api.betterlyrics.org` | Already built, on |
| Apple Music direct or via BiniLyrics / lrc.red | Syllable | Apple web API | Apple media-user token | n/a | Closed | `amp-api.music.apple.com` | Rejected (ADR 0002: account token) |
| Musixmatch richsync | Word timing, largest catalogue, strong VN community | Official API (paid plans), or the desktop-app token used by community clients | Paid key, or an unofficial app token | Per plan | Closed and licensed; free plan gives a 30% preview | `api.musixmatch.com`, `apic-desktop.musixmatch.com` | Rejected: a secret would have to ship, or an unofficial token |
| NetEase YRC | Word timing, strong for C-pop, some K-pop, little VN | `yrc` only through encrypted `eapi` requests (unsigned `/api/song/lyric/v1` returns no `yrc`, ADR 0002 probes 2026-11-11) | None, but requests must be encrypted like the app's | Unstated | No public API; terms unverified | `music.163.com`, `interface3.music.163.com` | Rejected: faking the app's request encryption (ADR 0002). Line LRC stays opt-in |
| QQ Music QRC | Word timing, strong K-pop and C-pop | Private endpoints, `qrc` encrypted (3DES) | Faked `Referer` | Unstated | Unofficial | `c.y.qq.com` | Rejected: decrypting a protected format |
| KuGou KRC | Word timing, C-pop, some K-pop | `lyrics.kugou.com` search and download | None, but `krc` is XOR-encrypted and zlib-packed to keep other players out | Unstated | Unofficial | `lyrics.kugou.com` | Rejected: same reason as QQ |
| LRCLIB `lyricsfile` | Word timing when `hasWordSync`; few records so far (word sync arrived with `lyricsfile`) | Documented v1 API | None | About 45/min per IP | Free open service | `lrclib.net` (listed) | Later: no YAML parser in the stack; coverage small |
| Spotify color-lyrics | Mostly line timing; syllable is rare | Private web-player API | `sp_dc` session | Unstated | Unofficial | `spclient.wg.spotify.com` | Already planned as an opt-in line source in `nemo-spotify` (lyrics-plus #24) |
| AMLL TTML DB | Real syllable TTML, reviewed; mostly C-pop from NetEase ids, little VN or K-pop | Files in a GitHub repo, keyed by NetEase, QQ, Spotify or Apple ids | None | GitHub limits | CC0-1.0 | `raw.githubusercontent.com` | Not now: needs a platform id we do not have, and a large index download; good open fallback later |
| Unison | Syllable TTML, community, small | `GET /lyrics` | None | Unstated | Free with credit; data ODbL-1.0 | `unison.betterlyrics.org` | Already built, on |

Probes (five human-paced requests on 2026-11-15, a few seconds apart, our `User-Agent`; raw answers kept outside the repo):
1. LyricsPlus `lyricsplus.prjktla.my.id`, Son Tung M-TP "Chung Ta Cua Hien Tai", 301 s: 200, `type` `Word`, source Apple, `cached` `Database`, 94 lines with pieces, 17 background pieces, 0 synthetic.
2. Same host, aespa "Supernova", 179 s: 404; Apple, Deezer and QQ `SKIP`, user DB `BAD`.
3. Same host, Den "Bai Nay Chill Phet", no duration: 404, same shape.
4. `lyricsplus.binimum.org`, aespa "Supernova" with album: 404; `qapple`, `qq`, `deezer` `BAD`.
5. Better Lyrics API, the song of probe 1: 200, TTML from lrc.red.

Reading: LyricsPlus has the widest reach (its cache plus four upstreams), but at probe time its live Apple path was skipped or failing on both instances, so a miss is often temporary. The provider therefore reports such a 404 as an error (cached minutes), not a miss (cached a week). For Vietnamese, word timing is already per syllable, because Vietnamese writes one syllable per word.

### Risks

- Grey area: an unofficial relay of other services' lyrics, reached with its operators' accounts. It can stop, block other apps or change shape at any time; it fails soft (the chain goes on). Off by default, risk note in Settings, provider and upstream named under the lyrics.
- The maintainers did not publish terms for other apps, and the extension source asks others not to take its server list. Owner question raised (requirements, Open Questions).
- Copyright: lyrics are shown and cached locally only, as for every provider (coc.md). Fixtures hold invented text.
- One host only: when that instance is down, LyricsPlus gives nothing. Rotation across instances was left out to keep the egress list short.

### Departures

- Owner override, 2026-11-15: a syllable-capable source is off by default when it is grey, against ADR 0002's 2026-11-11 "turn on every syllable or word source". Proposed `design.md` edit: none (no visual rule changes). ADR 0002 gets an addendum with the new row.

### Design checklist (design.md section 15)

No view code changed: the new row comes from the existing data-driven Settings > Lyrics list. 1-9 and 12: unchanged (no new tokens, sizes, motion or drawing). 10: yes, the footer shows "LyricsPlus" and the sync level, and the credit names the upstream service. 11: yes, the settings text says it is off until turned on and names the risk.

<!-- Sources (read 2026-11-15):
https://github.com/ibratabian17/LyricsPlus (README: sources, credentials, notice, Apache-2.0)
https://github.com/ibratabian17/LyricsPlus/blob/gelato/docs/endpoints.md (V2 shape, rate limit, 404, 503)
https://github.com/ibratabian17/LyricsPlus/tree/gelato/internal (domain/v2.go Synthetic flag, orchestrator/grade.go default source order, providers/qaple.go spread timing, service.go negative cache)
https://github.com/ibratabian17/YouLyPlus (src/background/constants.js server list; services/kpoeService.js request shape)
https://github.com/MetrolistGroup/Metrolist/issues/2969 (instances, no key needed)
https://lrclib.net/docs.txt (hasWordSync, lyricsfile, rate limits)
https://docs.musixmatch.com/lyrics-api/content-restrictions ; https://github.com/Strvm/musicxmatch-api (unofficial community token)
https://github.com/kangkang520/kugou-lyric ; https://convert.guru/krc-converter (KRC encrypted)
https://github.com/amll-dev/amll-ttml-db (ids, NetEase focus) ; https://github.com/amll-dev/applemusic-like-lyrics/blob/main/packages/docs/src/content/docs/en/guides/overview/eco.md (CC0-1.0)
docs/adr/0002-lyrics-providers.md (NetEase yrc probes, QQ, BiniLyrics, Better Lyrics API, Unison)
.kiro/specs/lyrics-plus/design.md (Spotify line provider)
-->
