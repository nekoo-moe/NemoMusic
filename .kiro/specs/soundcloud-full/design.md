## Overview

`nemo-soundcloud` gets its own client for SoundCloud's web API v2, the API the soundcloud.com web app calls (what yt-dlp's extractor and similar open-source clients use). It replaces the `yt-dlp` route of `.kiro/specs/soundcloud/` for search, pages, accounts and playback; `yt-dlp` stays only as a later fallback for streams the native path cannot resolve.

What the probes of 2026-11-19 showed (anonymous, a few requests; details in ADR 0005):
- The web app embeds a 32-character `client_id` in one of its scripts on `a-v2.sndcdn.com` (found in the last of nine `<script src>` tags of the home page, a 3 MB file). Every API call needs it as `?client_id=`.
- Of 81 tracks from five searches, 76 offer only HLS: `aac_160k` and `aac_96k` as fragmented MP4 (`#EXT-X-MAP` init segment and ten-second `.m4s` segments, 13 for a 2-minute track) and an adaptive `abr_sq` list that yt-dlp also skips. 4 offer MP3 as HLS and progressive (`mp3_0_0`, `mp3_1_0`): three old full uploads and one 30 s preview (`policy: SNIP`, host `cf-preview-media.sndcdn.com`). One track had only `cbc-encrypted-hls` and `ctr-encrypted-hls` (DRM). So HLS is the normal case, not the second step, and the first playable PR must read it.
- Resolving a transcoding is one call (`GET <transcoding url>?client_id=…[&track_authorization=…]` returns `{"url": …}` to a signed `playlist.m3u8` on `playback.media-streaming.soundcloud.cloud`). The CDN answered the init segment in 62 ms and the first 200 KB segment in 87 ms from this Codespace. The API call took 0.26 s and the playlist 0.46 s on fresh connections, so a warm connection and an in-memory track record should reach the 0.5 s target.
- `symphonia` 0.6.1 `isomp4` reads fragmented MP4 without a segment index and starts at the first `moof` once it has `ftyp` and `moov`. PR 3 confirmed that the init segment plus the segments decode to PCM through the app's `StreamDecoder`.

## Affected Crates

- `nemo-soundcloud`: the client (HTTP, client id, paging, typed JSON, mapping to `nemo-core` types), `SoundcloudCatalog` (`Catalog`), `ScStreamResolver` (`UrlResolver`: stream lookup by track page URL), the HLS reader `HlsSource` and `ScStreamOpener` (`StreamOpener`), later the account (token in `SecretStore`). New dependencies are those the workspace already pins: `reqwest` =0.13.5 (`native-tls`), `tokio` =1.54.1, `serde`, `serde_json`, `thiserror`, `tracing`. No new crate: the HLS media playlist is a dozen line types, parsed by hand.
- `nemo-core`: PR 3 adds three defaulted trait methods and no type: `UrlResolver::warm` and `UrlResolver::stream_failed` (as `StreamResolver` has them) and `ByteSource::duration_hint` (the init segment of a fragmented stream states no length, and `symphonia` would report the first fragment's two seconds; the HLS source reports the sum of the playlist's `EXTINF`). Wave 2 adds `Backend.soundcloud_account: Option<Arc<dyn SourceAccount>>` (see Interfaces).
- `nemo-audio`: `PlayerDeps.url_resolver` and `PrepareDeps.url_resolver: Option<Arc<dyn UrlResolver>>` (a track with `source_url` and no video id resolves through it; `finish_stream` takes the origin from the resolver's stream info instead of assuming YouTube Music), `warm_ahead` also warms the next SoundCloud entry, and `StreamDecoder::open` prefers `duration_hint`. Nothing on the audio callback. A track from a stream the decoder cannot read gives the same plain error as today. Run `bit_exact` and the counting-allocator test after it.
- `nemo-signin`: wave 2, a SoundCloud mode (`--site soundcloud`) that opens `soundcloud.com/signin` and prints the `oauth_token` through the same pipe.
- `nemo-secrets`: used only through the `SecretStore` trait (key `soundcloud/oauth-token`); no change.
- `nemo-ui`: waves 2 and 3: source switch, SoundCloud pages on the shared playlist and album components, like and add-to-playlist actions, skeleton loading from the new kit.
- `nemo-app`: builds `SoundcloudCatalog`, `ScStreamResolver` and `ScStreamOpener` only when the switch is on and confirmed; a routing opener sends `Origin::SoundCloud` streams to `ScStreamOpener` and the rest to the YouTube opener.

`nemo-soundcloud` still depends on `nemo-core` only. It does not depend on `nemo-ytm`, so it carries its own copy of the small HTTP rules (HTTPS-only redirects, no URL in errors, `Retry-After` in seconds).

## Data Flow

Client id. The first SoundCloud request after the switch is on fetches `https://soundcloud.com/`, collects the `<script src>` values on `a-v2.sndcdn.com`, fetches them from last to first and stops at the first that holds `client_id:"<32 characters>"`. The id lives in memory (`Mutex<Option<Secret<String>>>`, never logged). A 401 or 403 on a call that sent a client id discards it, discovers again once and repeats the call once; a second failure ends the call. One discovery at a time (a second caller waits for the first). The app starts it in the background when SoundCloud is on and a SoundCloud screen first opens, so the first play does not pay for it.

Requests. One `ScHttp` trait (`send(request) -> answer`), implemented over `reqwest` (HTTPS only, three redirects, 15 s timeout, no cookie store, the app's own `User-Agent`, which the home page and the API accepted on 2026-11-19; bodies read up to 4 MiB, scripts up to 8 MiB) and by fakes in tests. API calls pass through one limiter: a `tokio::sync::Mutex` held across the whole call and a 250 ms minimum gap between starts. A 429 sets a shared "not before" time from `Retry-After` (60 s without it, at most 15 minutes) and later calls answer `CatalogError::RateLimited` without a request. 5xx and connection errors retry twice after 1 s and 3 s, on reads only (never on a write). Media requests (playlist, init and segments) use the CDN host with no client id and no token and are not in the API limiter, but one reader fetches at most three segments ahead.

Paging (wave 2; the first page of every list is read now). A list endpoint answers `{collection, next_href}`; `next_href` is followed only when the user asks for more and only if it starts with the API base. Playlist tracks come as full objects for the first few entries and `{id, kind, policy}` stubs for the rest, so the rest is filled by `GET /tracks?ids=<up to 50 ids>` in batches, on opening the page (a 416-track playlist is nine calls, a few seconds; the first screen shows after the first batch).

Mapping. `Track.id` is `sc:<numeric id>`, `Track.title` the `title`, `artists` `[user.username]` (the uploader; `publisher_metadata.artist`, when it differs, comes first), `album` `publisher_metadata.album_title` when present, `duration` the `full_duration`, `origin` `Origin::SoundCloud`, `source_url` the `permalink_url` as a `MediaUrl` on `HOSTS`, `artwork` the `artwork_url` (else the uploader's `avatar_url`) with the `-large` size name replaced by `-t300x300`, `-t500x500` and the base size (`i1.sndcdn.com`). Playlists and albums (`is_album`) become `PlaylistSummary` and `PlaylistDetail`, users `ArtistSummary` and `ArtistDetail`. Nothing identifying about a user beyond `id`, `username`, `permalink`, `avatar_url`, `followers_count`, `track_count` is parsed; `first_name`, `last_name`, `date_of_birth`, `city` and the rest are not in the typed structs and so never stored.

Play. `prepare` calls `ScStreamResolver::resolve(source_url, policy)` on the prepare thread. The resolver keeps a small in-memory map `permalink -> TrackRecord { id, transcodings, track_authorization, policy }` (at most 200 entries, filled by every catalog call that maps a track) and asks `GET /resolve?url=` only when the page URL is not in it. It lists the transcodings as `FormatCandidate`s (`aac_*` and `mp3_*` presets, `progressive` or `hls`; `abr_*` and `*-encrypted-hls` are not candidates; any encrypted-only track answers `NotAvailable`), calls `rank_formats`, resolves the winner (`GET <transcoding url>?client_id=…&track_authorization=…`, `Authorization` header when signed in) and returns a `ResolvedStream` (`transport` `Hls { live: false }` or `File`, `preview` from `snipped` or policy `SNIP`, measured `StreamInfo` from the preset, `expires_at` from the signed `expires` query value). Answers are cached in memory for three minutes. The playlist address stops working about four minutes after the lookup (its signed `Policy` carries a `DateLessThan`, measured on 2026-11-19; the `expires` query value is two hours out and is not the real limit; an address 45 minutes old answered 403). The segment addresses inside the playlist are signed for about two hours, so one playlist fetch carries a track of up to two hours and no mid-track refresh is needed. A longer stream is a later wave.

`ScStreamOpener::open` for HLS: one `reqwest` blocking client, `GET playlist.m3u8`, parse (`#EXT-X-MAP`, `#EXTINF`, segment URIs, `#EXT-X-ENDLIST`; any `#EXT-X-KEY` other than `NONE` ends with `NotAvailable` before a segment is fetched), then one fetch thread sends `init + segment bytes` through a `sync_channel(3)` to the decoder thread that reads them as one forward byte stream. `init.mp4` and segment 0 are requested at the same time on two threads at start so that the first bytes arrive after one round trip. The source reports `is_seekable() == false` until wave 3 and `byte_len() == None`. Until wave 3 a seek on a SoundCloud HLS track fails with "could not seek in this track" (the feeder drops the track on a failed seek); the wave 2 UI greys the seek bar for these tracks. Progressive MP3 (a few old uploads) is a `Transport::File` and goes to the ranged file reader of `nemo-ytm`, which seeks. `nemo-app` routes by transport and origin. The audio callback is unchanged: decoder, `rtrb` ring, `cpal`.

Prefetch. `ScStreamResolver::warm(url)` (called by the player for the entry after the current one) looks up that track's stream, then fetches its playlist, init segment and first segment into memory (about 200 KB, at most two streams, dropped after 150 s or when the stream fails). One blocking HTTP client for the media hosts is built when SoundCloud starts (it loads the system certificates, about 50 ms) and reused for every track, so the connection to the media host stays open between tracks.

Account (wave 2). The sign-in window (`nemo-app --signin --site soundcloud`) loads `https://soundcloud.com/signin`, waits for a page on `soundcloud.com` that has an `oauth_token` cookie and prints `{"token":"…"}` on stdout. The app stores the value with `SecretStore::set("soundcloud/oauth-token", …)`, calls `GET /me` and keeps only `id`, `username`, `avatar_url` in memory. Sign-out deletes the key, the window's profile folder and the caches (criterion 12).

Writes (wave 4): `PUT|DELETE /users/<me>/track_likes/<id>`, `PUT|DELETE /users/<me>/playlist_likes/<id>`, `POST|DELETE /me/followings/<id>`, `POST /playlists`, `PUT|DELETE /playlists/<id>`, `POST /me/play-history` (only when switched on). The shapes are read from the web app's own requests in wave 4 before they are written; the route table was read on 2026-11-19, the bodies were not.

## Interfaces

```rust
// nemo-soundcloud (signatures only)
pub const HOSTS: &[&str];                       // see the README rows below
pub trait ScHttp: Send + Sync {
    fn send<'a>(&'a self, request: ScRequest) -> BoxFuture<'a, Result<ScAnswer, ScHttpError>>;
}
pub struct ScRequest { pub url: String, pub max_bytes: usize }   // GET only; the token comes in wave 2
pub struct ScAnswer { pub status: u16, pub body: Vec<u8>, pub retry_after: Option<Duration> }
pub struct ScClient;                            // client id, limiter, backoff, paging
impl ScClient { pub fn new(http: Arc<dyn ScHttp>) -> Self; pub async fn warm_up(&self) -> Result<(), ClientError>; }
pub struct SoundcloudCatalog;                   // impl Catalog: search, search_kind, album, artist,
                                                // playlist, liked_songs, library_*, history, home_feed
impl SoundcloudCatalog { pub fn new(client: Arc<ScClient>) -> Self;
    pub async fn related(&self, track: &TrackId) -> Result<Vec<Track>, CatalogError>;
    pub async fn open_link(&self, url: &MediaUrl) -> Result<Link, CatalogError>; }
pub struct ScStreamResolver;                    // impl UrlResolver (resolve, state; list = NotAvailable)
pub struct ScStreamOpener;                      // impl StreamOpener (Hls and File)
pub struct HlsSource;                           // impl ByteSource, Read, Seek (forward only)

// nemo-audio/src/prepare.rs
pub struct PrepareDeps { /* new */ pub url_resolver: Option<Arc<dyn UrlResolver>>, /* ... */ }

// nemo-core (wave 2, next agent)
pub trait SourceAccount: Send + Sync {
    fn state(&self) -> SourceAccountState;      // SignedOut | SignedIn { name, avatar } | Expired
    fn sign_in(&self) -> BoxFuture<'_, Result<(), AccountError>>;
    fn sign_out(&self) -> BoxFuture<'_, Result<(), AccountError>>;
}
```

`UrlResolver::resolve` takes the track page URL as `MediaUrl`, as the `soundcloud` spec fixed. `list` answers `NotAvailable` for the native resolver (the catalog lists). `Catalog` methods that SoundCloud does not have (`podcast`, `channel`, `live_streams`, `explore`, `mood_playlists`) keep their default "not available" answers.

## Error Handling

- No new `StreamError` or `CatalogError` variant. Encrypted-only and region-blocked (`policy: BLOCK`) tracks map to `NotAvailable` with the plain text "cannot play here"; private and removed tracks (404) to `NotFound`; 429 to `RateLimited`; offline and timeouts to `Network` with fixed texts (never the URL or the body).
- 401 with a token: session ended (criterion 10): the account state becomes `Expired`, the token is not sent again, and public calls continue without it. 401 or 403 without a token or with a fresh client id: `Unavailable("SoundCloud refused the request")`.
- HLS: a playlist fetch or segment fetch failing three times in a row ends the stream with `Network`; a segment 403 or 410 asks the resolver to look the stream up again once (`stream_failed`).
- Parse errors name the field, never the value. Unknown JSON fields are ignored; a missing field the mapping needs makes that item skipped, not the page failed.

## Testing Strategy

- A `ScHttp` fake drives the client, catalog and resolver. `test_server.rs` (a small loopback HTTP/1.1 server copied in style from `nemo-ytm`, `#[cfg(test)]`) drives the real `reqwest` implementation: redirects to http refused, 429 with `Retry-After`, 5xx retry, body cap, no URL in errors.
- Recorded and sanitised JSON fixtures under `crates/nemo-soundcloud/tests/fixtures/`: a track, a search page (tracks, playlists, users), a playlist with stubs and the batch answer, a user, related tracks, a snipped preview, an encrypted-only track. IDs, names, URLs and tokens are replaced with invented values; the fixtures keep the real field names and shapes. A test fails if a fixture holds `client_id`, `oauth`, a signature or a real host that is not on `HOSTS`.
- Pure tests: client id regex (bundle text with the id, without, two ids), script order, `rank_formats` inputs from the real preset names, the HLS playlist parser (init, `EXTINF`, key line, endlist, relative and absolute URIs), artwork size rewriting, `Retry-After` clamp.
- `nemo-audio`: `cargo test -p nemo-audio --test bit_exact` and the counting-allocator test after the `prepare` change.
- Real service: one `#[ignore]` example (`crates/nemo-soundcloud/examples/latency.rs`, at most about 15 requests, a few seconds apart) prints the stage times in milliseconds (client id, lookup, playlist, first bytes, decoder open, first frame). It writes no token and no URL.
- Manual (owner): sign in, like, follow, playlist write, sign out on a real account; a listening comparison against the web player.

## Risks and Alternatives

- The web API is unofficial and can change: the `client_id` location, the response shapes, the transcoding names. Mitigations: tolerant parsing, one refresh of the client id, the `yt-dlp` fallback in a later wave, the switch off by default and the plain risk text. SoundCloud may rate-limit or block this client; the app backs off and says so.
- Signing in with a browser session token ties NemoMusic to the user's account: SoundCloud may restrict the account. The token is the web session's own, so it may stop working when the user signs out in the browser. The risk text says both (ADR 0005).
- Fragmented AAC through `symphonia` 0.6.1: verified in PR 3. The init segment followed by the segments, with no index, opens and decodes through `StreamDecoder` (recorded 4 s test tone in `crates/nemo-soundcloud/tests/fixtures/hls/`, generated with `ffmpeg`, no third-party audio). The container length is wrong for such a stream (the first fragment's), so the source passes the playlist length. Seeking is the open part (wave 3).
- Latency. Four stages sit between the click and sound; the target is met only with a warm client id, a known transcoding list and a reused connection. A cold start (client id discovery, 3 MB) is not covered by the 0.5 s target.
- Alternatives rejected: the official API (needs an app registration and does not give full-library streaming to desktop players; terms unverified); `yt-dlp` for everything (an extra process start of about 0.5 s per track and no account); `m3u8-rs` (a new crate for a format this small); a headless web view that plays the web player (heavy, no output-path control).
- Departures from earlier decisions, each labelled where it is edited: `soundcloud` spec criteria 21 and 24 and its crate boundary (owner, 2026-11-19); the `structure.md` `nemo-soundcloud` bullet. ADR 0005 supersedes the `nemo-soundcloud` row of ADR 0004 Decision 1 ("stream lookup uses the URL-resolver trait that `nemo-ytm` implements with `yt-dlp`"); ADR 0004 itself is left as written.

## Steering and README changes

- `structure.md`: the `nemo-soundcloud` bullet (own HTTP client, account, HLS reader; still depends on `nemo-core` only), the `nemo-signin` bullet (a second site), the layout comment. (owner, 2026-11-19; soundcloud-full)
- `tech.md`: ground rule line for SoundCloud, the SoundCloud access row with the pinned `reqwest` and the new hosts. No new crate. (owner, 2026-11-19; soundcloud-full)
- `design.md`: a SoundCloud screens section arrives with the first UI PR (wave 2), not now.
- README egress list: `soundcloud.com` (home page for the client id, sign-in page), `api-v2.soundcloud.com` (search, pages, account), `a-v2.sndcdn.com` (the web app's script that holds the client id), `i1.sndcdn.com` (covers and avatars), `playback.media-streaming.soundcloud.cloud` and `*.sndcdn.com` media hosts (the audio, preview and legacy MP3), each "only when SoundCloud is on". The `*.sndcdn.com` row stays a family because preview and legacy files come from `cf-*-media.sndcdn.com`; `MediaUrl` accepts only those, https only, no user or port.
- The old `soundcloud` spec gets a labelled note at the top of its three files and its tasks 7 to 20 are marked replaced.

<!-- Sources:
crates/nemo-core/src/{track,stream,services,url_resolver,catalog}.rs, crates/nemo-audio/src/{prepare,source,stream_decoder}.rs, crates/nemo-ytm/src/{http,web_search,test_server}.rs, crates/nemo-signin/src/lib.rs
symphonia-format-isomp4 0.6.1 src/demuxer.rs (fragmented streams, seek reads forward)
yt-dlp extractor/soundcloud.py (Unlicense): client id search, transcoding handling, DRM protocol names
Probes of 2026-11-19, see requirements.md Sources
-->
