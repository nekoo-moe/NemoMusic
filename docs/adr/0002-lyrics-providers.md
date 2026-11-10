# ADR 0002: Lyrics providers and sync

Status: proposed - owner to confirm
Date: 2026-11-10
Related: product.md (Local only), structure.md (nemo-lyrics), coc.md, tech.md

## Context
Owner wants time-synced lyrics from several open providers, no project server, every contacted host listed in the README. Lyrics are copyrighted text. Facts below come from the sources at the end; anything not confirmed is marked `unverified`.

## Provider evaluation
| Provider | Open? | ToS / legal | Sync | Coverage | Key | Stability | Verdict |
|---|---|---|---|---|---|---|---|
| Local `.lrc` sidecar, embedded tags (lofty) | Yes, user's own files | None (user data) | line (LRC), word (enhanced LRC), SYLT line | Only what the user has | No | Stable | default on |
| LRCLIB `lrclib.net` | Yes, free open DB | Free service, must send identifying User-Agent and honor `Retry-After`. Data license of the DB: unverified (dumps are public sqlite3 files per the maintainer) | line (`syncedLyrics`), word via `lyricsfile` when `hasWordSync` | Large, community-made, uneven for obscure tracks | No (optional App-Id for higher limits) | v1 documented as stable; v2 search is beta | default on |
| YTM timed lyrics (innertube) | No, unofficial access | Same ToS risk as the rest of YTM (accepted 2026-11-10) | line, ms start/end (ytmusicapi `TimedLyrics`) | Only tracks YTM has; source often a third party, named in `source` | Signed-in session | Breaks when YouTube changes | default on (rides the YTM session, no extra host) |
| NetEase Cloud Music | No official public API; community wrappers use private endpoints | ToS status: unverified; unofficial access likely breaches it | line LRC; word (yrc) on newer endpoints, unverified | Strong for CJK | Not for lyric lookup, unverified | Private endpoints, change without notice | opt-in, off by default, with risk text |
| QQ Music | No official public API for this; community wrappers | ToS status: unverified; same concern as NetEase | line LRC, word (qrc) unverified | Strong for CJK | Unverified | Private, unstable | opt-in, off by default, with risk text |
| Musixmatch | Closed, licensed | Official API free plan returns only a ~30% lyrics preview; full and synced lyrics need a paid commercial license (third-party summaries, unverified). Unofficial desktop-token use breaches ToS | line, word (richsync) | Largest | Yes | Official API stable | reject (no free full or synced access, no key may be shipped) |
| Genius | Closed | API gives metadata, not lyrics; scraping lyrics violates ToS (lyricsgenius docs); commercial API use needs a license | none (unsynced) | Large | Yes | n/a | reject |
| Aggregators (Lyrica, SyncLRC, BetterLyrics API) | Wrappers of other sources, often run by individuals | Inherit upstream ToS, plus a third-party server dependency | line, word | Varies | Varies | Hobby projects | reject (adds hosts we cannot vouch for) |
| Spotify / Apple / KuGou lyrics | Closed | Private endpoints, breach ToS | line, word | Large | Account | Unstable | reject |

Notes:
- ytmapi-rs 0.3.4 (docs.rs): `GetLyricsIDQuery` then `GetLyricsQuery` returns `Lyrics { lyrics: String, source: String }`, plain text only. Timestamps are not exposed there (unverified for the newest release). ytmusicapi (Python) returns line timestamps in ms from the mobile client, so `nemo-ytm` must either call that innertube request itself or contribute it. Do not invent the request body; copy from ytmusicapi source after owner approval (touches endpoints, coc.md stop rule).
- LRCLIB `GET /api/get` needs `track_name`, `artist_name`; `album_name` and `duration` (1-3600 s) recommended. It matches only when duration is equal or within +-2 s. `GET /api/search` returns max 20 records, no paging. `GET /api/get/{id}` fetches one record. Responses carry `plainLyrics`, `syncedLyrics`, `lyricsfile` (YAML), `instrumental`, `hasWordSync`.
- LRCLIB limits (approximate, may change): ~45 req/min per IP unidentified, ~90 identified. 429 or 503 carries `Retry-After`; honor it with jitter. Send requests sequentially.
- LRCLIB DB dump (`lrclib.net/db-dumps`, sqlite3 files): we do NOT bundle or download it by default (size, license unverified, "no project server" fits but offline mirror is out of scope). Possible later opt-in.
- lofty: USLT (unsynced), Vorbis `LYRICS`, MP4 lyrics map to the lyrics item key (unverified exact mapping). SYLT (synced ID3v2) typed support in lofty 0.25.5 is unverified; if absent, read the raw frame or skip it.
- Network egress list additions (README): `lrclib.net` (default). NetEase and QQ hosts only when the user enables them, exact hostnames unverified, owner approval needed per tech.md/product.md.

## Decision (proposed)
1. Default chain: local file, YTM timed lyrics, LRCLIB. Opt-in: NetEase, QQ (separate toggles, each with a plain risk note). Rejected: Musixmatch, Genius, aggregators.
2. `LyricsProvider` trait and shared types live in `nemo-core`. YTM provider lives in `nemo-ytm`. LRCLIB, local, NetEase, QQ, parsers, matching, chain and cache live in the new `nemo-lyrics` crate, which depends only on `nemo-core`. `nemo-app` wires them.

## Design
### Trait (nemo-core, signatures only)
```rust
pub struct LyricsQuery { pub title: String, pub artists: Vec<String>, pub album: Option<String>,
    pub duration_ms: Option<u64>, pub ytm_video_id: Option<String>, pub local_path: Option<PathBuf> }
pub enum Granularity { Word, Line, Unsynced }          // ordered best to worst
pub struct LyricsDoc { pub lines: Vec<LyricLine>, pub granularity: Granularity,
    pub provider: ProviderId, pub matched: Option<MatchedMeta>, pub instrumental: bool }
pub struct LyricLine { pub start_ms: Option<u64>, pub end_ms: Option<u64>, pub text: String, pub words: Vec<LyricWord> }
#[async_trait] pub trait LyricsProvider: Send + Sync {
    fn id(&self) -> ProviderId;                 // stable string, shown in UI
    fn hosts(&self) -> &'static [&'static str]; // for the egress list and settings
    async fn fetch(&self, q: &LyricsQuery) -> Result<Option<LyricsDoc>, LyricsError>; // None = not found
}
```
Errors are typed (`thiserror`): `RateLimited { retry_after }`, `Network`, `Parse`, `Unavailable`. No `unwrap`; no secrets in logs.

### Fallback chain
- Order is user-editable. Default: local, YTM, LRCLIB, then enabled opt-ins.
- Local runs first, alone (no network). Remote providers then run in parallel with a per-provider timeout of 4 s and a global 6 s budget, at most 1 in-flight request per host (LRCLIB asks for sequential requests).
- Pick the best result, not the first: rank by granularity (Word > Line > Unsynced), then match score, then chain order. Show the first acceptable result at once and upgrade if a better one arrives within the budget (no flicker after budget).
- Match score (0-100) on normalized strings (Unicode NFKC, lowercase, strip `(feat. ...)`, `- Remastered`, punctuation): title 40, primary artist 30, album 10 (ignored if missing), duration 20. Duration: full points within 1 s, linear to 0 at 3 s; reject beyond 3 s when both durations are known (LRCLIB itself accepts +-2 s). Accept at score >= 70 and title score > 0. Below that, treat as not found.
- A synced result whose last timestamp exceeds track duration + 5 s is rejected as a wrong match.
- Instrumental records show "Instrumental", not an empty panel.

### Parsers (nemo-lyrics, `proptest` on each)
- LRC: `[mm:ss.xx]` and `[mm:ss.xxx]`, multiple tags per line, `[offset:+/-ms]` header, ignore `[ar:]`, `[ti:]`, `[al:]`, `[length:]`. Sort by time; keep original order for equal times.
- Enhanced LRC: inline `<mm:ss.xx>` word tags, optional trailing end tag. Lines with inline tags become `Granularity::Word`.
- Lyricsfile YAML (LRCLIB `lyricsfile`): word timings under `lines[].words`. Field names unverified (spec page not readable); write the parser against fixtures after reading `lrclib.net/lyricsfile`.
- YTM timed lines: ms start and end as returned.
- NetEase yrc and QQ qrc (only if their providers are enabled): formats unverified; implement only after fixtures are captured and reviewed.
- Plain text (USLT, `plainLyrics`): `Granularity::Unsynced`, no timestamps. Never derive timing by guessing.

### Cache (SQLite, rusqlite, in the app DB or its own file in the local data dir)
```
lyrics_cache(track_key TEXT, provider TEXT, status TEXT /*hit|miss|error*/, granularity TEXT,
             body BLOB /*zstd-free, plain serialized LyricsDoc*/, score INTEGER, fetched_at INTEGER, expires_at INTEGER,
             PRIMARY KEY(track_key, provider))
lyrics_offset(track_key TEXT PRIMARY KEY, offset_ms INTEGER)
```
- `track_key` = YTM video id, or hash of normalized artist+title+album+duration for local files.
- TTL: hit 30 days (refresh quietly), miss (negative cache) 7 days, error 15 min with backoff from `Retry-After`. "Refetch lyrics" in the UI clears the entry.
- Per-track user offset (+-10 s, 50 ms steps) is stored separately and applied on top of the provider `[offset:]` tag; resetting it never touches cached text.
- Sign-out wipes account-linked rows; the user can clear the whole lyrics cache in settings. Cache stays local, never uploaded.

### Playback clock and highlighting
- The UI does not use a wall-clock timer. `nemo-audio` publishes the played position as frames written to the device minus the reported output latency, divided by sample rate (position at the speakers). It exposes it through `PlayerEvent` / a shared atomic read by the UI thread; the audio callback does no extra work (RT rules in code.md).
- Each frame the UI computes `t = position_ms + lyrics_offset_ms`, finds the current line by binary search (cached index, step forward on normal play, re-search after seek or track change), and for word lines the current word by `start_ms <= t`.
- Latency compensation is the device-reported value; if unknown, use 0 and show nothing about it. Users fix residue with the per-track offset.
- Highlight easing follows design.md (<= 400 ms, honors reduced motion; with reduced motion switch lines without animation). Auto-scroll pauses when the user scrolls and resumes on a line click, which seeks to that line's start.

### Honest labeling and rights
- Always show the provider name under the lyrics ("Lyrics: LRCLIB"), plus the YTM `source` string when present and the matched title/artist if the match score is below 90.
- Granularity badge: "Synced by word", "Synced by line", or "Not synced". Unsynced lyrics scroll nothing and highlight nothing. Never fake timing (no even spreading of lines across the song).
- Lyrics are copyrighted text. The app displays them and keeps a local cache only. No export, copy-all, share, save-as-file, or "write to tags" feature. Do not publish to LRCLIB (`/api/publish`) from the app; it is out of scope.
- Respect takedown or flag outcomes: a provider's removed record disappears at the next refresh.
- No lyrics, queries or track titles go to any host not on the egress list. Each query sends only title, artist, album, duration to the enabled providers; the settings screen says so.

## Consequences
- Good: three default sources, word-level where available, graceful degradation, no project server.
- Risk: YTM timed lyrics and the opt-in CJK providers depend on unofficial endpoints and may break or breach ToS. They fail soft (chain continues).
- Risk: LRCLIB is a volunteer service; limits may change. Cache plus sequential requests keep load low.
- Open for the owner: approve NetEase/QQ as opt-in (hosts and ToS), approve `lrclib.net` in the egress list, approve the YTM timed-lyrics request (ADR 0001 session), decide on LRCLIB dump use, confirm the lyrics license posture for the UI.
- Verification still needed: lofty SYLT support, Lyricsfile field names, LRCLIB data license, NetEase/QQ endpoints and ToS, ytmapi-rs timed-lyrics support.

<!-- Sources:
https://lrclib.net/llms.txt
https://lrclib.net/docs.txt (endpoints, rate limits, User-Agent, +-2 s duration match)
https://news.ycombinator.com/item?id=39480390 (maintainer says DB dumps are public sqlite3 files at https://lrclib.net/db-dumps)
https://github.com/tranxuanthang/lrclib
https://ytmusicapi.readthedocs.io/en/latest/reference/api/ytmusicapi.models.html (LyricLine, TimedLyrics, ms)
https://docs.rs/ytmapi-rs/latest/ytmapi_rs/parse/struct.Lyrics.html (lyrics + source strings only)
https://github.com/nick42d/youtui (GetLyrics, GetLyricsID implemented in ytmapi-rs)
https://docs.genius.com/ (commercial use needs license)
https://lyricsgenius.readthedocs.io/en/stable/how_it_works.html (scraping violates Genius ToS)
https://docs.musixmatch.com/lyrics-api/content-restrictions
https://freeapihub.com/apis/musixmatch and https://publicapis.io/musixmatch-api (free plan 30% preview; third-party, unverified)
https://github.com/Binaryify/NeteaseCloudMusicApi/issues/1667 (NetEase word-level on mobile endpoint)
https://github.com/p2o51/lyrics-fetcher/blob/main/SKILL.md (NetEase, QQ, LRCLIB used together by community tools)
https://github.com/jitwxs/163MusicLyrics
https://github.com/Wilooper/Lyrica and https://github.com/TharukRenuja/SyncLRC (aggregators)
-->
