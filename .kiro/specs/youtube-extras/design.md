## Overview

Five parts, all in `nemo-ytm` and the catalog types of `nemo-core`: search over all of YouTube, live streams, comments, dislike counts from Return YouTube Dislike (RYD), and YouTube captions for lyrics and subtitles. The external chart site is dropped (owner, 2026-11-14). No new crate and no new dependency.

The catalog today is `ytmapi-rs` 0.3.4 against `music.youtube.com` (`catalog.rs`, `raw.rs`). `ytmapi-rs` has no video search, channel pages or comments. These parts use raw innertube requests to `www.youtube.com/youtubei/v1/` with the web client, built on the `ClientProfile` and request code in `innertube.rs`. Both hosts are already on the README egress list. Nothing runs unless its setting is on, and nothing loads in the background.

Innertube calls, from community clients and docs and not yet confirmed against the live service (the implementing task runs a spike and stores stripped fixtures):
- Search all of YouTube: `POST /youtubei/v1/search` with `query` and no `params`. The answer mixes `videoRenderer`, `channelRenderer` and `playlistRenderer`; a live item carries a "LIVE NOW" badge. The music search of `music.youtube.com` returns only music shelves, so "All of YouTube" runs the music search first and then this one, one after the other.
- Channel page: `POST /youtubei/v1/browse` with the `UC...` id. Tab `params` come from the tab endpoints in the answer, not from constants.
- Read comments: `POST /youtubei/v1/next` with `videoId` gives a continuation token for the comments section; a second `next` with that token gives threads. Replies use the token in each thread. Newer answers carry comment data in `frameworkUpdates.entityBatchUpdate.mutations` with a `commentViewModel` instead of `commentRenderer`; the parser reads both.
- Post, reply, like: `POST /comment/create_comment` (`createCommentParams`, `commentText`), `/comment/create_comment_reply` (`createReplyParams`, `commentText`) and `/comment/perform_comment_action` (`actions`). The params and action tokens are copied from the read answer, never built. Each call needs the signed-in session (cookie plus `SAPISIDHASH` header, as the `account-actions` design sets up).
- Comments off: the read answer holds a message item instead of a token.

Captions (checked 2026-11-14): `yt-dlp -J` lists them in the `subtitles` (uploaded) and `automatic_captions` (auto-generated) fields of its info dictionary, as language code to a list of `{ext, url}` entries, with `json3` among the formats. The caption text is served from `www.youtube.com/api/timedtext`. Since 2025 some of these URLs return an empty body with status 200 unless they carry a PO token (the `pot` parameter; yt-dlp issue 13075). The token logic lives in `yt-dlp` and is patched there, so the app never builds or requests a timedtext URL. It lets `yt-dlp` write the file: `--skip-download --write-subs --write-auto-subs --sub-langs <code> --sub-format json3 --ignore-config` into a private temp folder, reads it, and deletes it. This is the same reason `tech.md` gives for `yt-dlp` over an in-process client. The list comes from a separate `yt-dlp -J --skip-download` run, because the stream run of `video-playback` is not always made. `yt-dlp` names the original auto-generated track `<code>-orig` (unverified; the spike checks). Neither run gets the account's cookie: the Premium cookie file of account-actions is only passed by `resolve_with`.

RYD (README of the project, read 2026-11-14): `GET https://returnyoutubedislikeapi.com/votes?videoId=<id>`; third-party use is allowed if the project is clearly attributed with a link to returnyoutubedislike.com; limits are 100 requests a minute and 10,000 a day per client, answered with 429; 404 means no such video, 400 a malformed id. Its terms page (`/terms`) answered 404 on 2026-11-14 and its Swagger page was not read, so the README is the only terms source; the design follows it and the owner should re-check the site before release. The answer's `dislikes` is an estimate; `rawDislikes` is ignored.

## Affected Crates

- `nemo-core`: new `captions.rs` (`SubtitleSource`, `SubtitleCue`, `CaptionError`); `lyrics.rs` gets a defaulted `LyricsProvider::timeout` (see Risks); `catalog.rs` gets `SearchScope`, video and channel types, `ChannelDetail`, comment types, `Catalog::search_scoped`, `Catalog::channel`, `Catalog::live_streams`; `services.rs` gets the `CommentSource` and `VideoRatings` traits and two fields on `Backend`; `player.rs` gets `PlayerSnapshot.live`.
- `nemo-ytm`: `web_search.rs` (search and channel parsers), `comments.rs` (read, post, reply, like), `ryd.rs` (client, limiter, cache), `live.rs` (live list from feed badges and the `UrlResolver` call), `captions.rs` (`YtmCaptions`, the json3 parser, the `CaptionsLyricsProvider`).
- `nemo-ui`: scope switch, video and channel results, Live area, comments panel, dislike label, the two settings entries, the one-time note for "All of YouTube".
- `nemo-app`: builds the new services only when their switches are on.
- `nemo-audio`: none, apart from the HLS source designed in the `soundcloud` spec (live streams use it).

## Data Flow

Search: UI task -> `Catalog::search_scoped(text, AllOfYoutube)` -> music search, then web search -> one `SearchResults` with `videos` and `channels` filled. The scope `Music` calls `search` as today (criterion 5).

Live: the `UrlResolver::resolve` of the `soundcloud` design runs `yt-dlp` for the video URL. A live YouTube page may offer only muxed HLS or split video-only and audio-only HLS (community reports differ and change with the client). The policy picks audio-only HLS if it exists, else the lowest-bandwidth muxed rendition. `ResolvedStream.transport = Hls { live: true }`. The shared `HlsSource` in `nemo-ytm` reloads the playlist every half target duration, starts 3 segments behind the live edge, and keeps a bounded channel of 3 segments to the decoder thread. If the container is MPEG-TS, a small `ts.rs` takes the AAC audio packets out of the stream (stream type 0x0F, output as ADTS) and drops the video; the video bytes are still downloaded (at 144p, a few hundred kbit/s). If it is fragmented MP4, `symphonia` picks the audio track. The stream has no length and `is_seekable() = false`. The audio callback is unchanged: decoder -> `rtrb` ring -> `cpal`; an empty ring already gives silence without allocation.

Comments: opening the panel -> `CommentSource::comments(video, None)` -> one `next` for the token, one for the first page; "Load more" passes the cursor. Posting is one call per press; the UI keeps the text until the call succeeds (criterion 16). Pages for the current item live in memory only and are dropped when the item changes or on sign-out.

Captions, lyrics: the lyrics chain calls `CaptionsLyricsProvider::fetch_credited` last, only when the switch is on and no other provider had lyrics. It asks `YtmCaptions` for the track list of `LyricsQuery.ytm_video_id` (one `yt-dlp -J` run, cached 10 minutes by video id), and, if that video has none, once for the music-video counterpart id that `video-playback` finds (unverified). It picks a track (criterion 31) and checks the video length against `duration_ms` (criterion 33). A second `yt-dlp` run writes the json3 file; `parse_json3` turns events into cues (`segs[].utf8` joined, `\n` split into lines, `tStartMs` start, `tStartMs + dDurationMs` end, capped at the next start), drops sound-tag lines and, for auto-generated tracks, lines that repeat the previous line or are a prefix of the next. The result is a `LyricsDoc` with `Granularity::Line`, provider "YouTube captions", and a `LyricsCredit` whose `note` is "Not lyrics-grade (auto-generated)" for auto tracks. Both runs have a 20 s timeout and run one at a time on the existing `yt-dlp` runner.
Captions, subtitles: `SubtitleSource::tracks` answers from the same cached `-J` result (or `ResolvedVideo.subtitles`), and `cues` runs the write step for the picked track only; cues stay in memory for the playing item (last 4 videos) and are dropped on sign-out.

Dislikes: opening an item -> `VideoRatings::dislikes(video)` -> memory cache (24 h) -> limiter -> one `GET`. At most once per item per session; own limit 1 request a second and 30 a minute, well under the service's 100; a 429 stops all requests for 10 minutes.

## Interfaces

```rust
// nemo-core/src/catalog.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchScope { #[default] Music, AllOfYoutube }
pub struct ChannelRef { pub id: BrowseId, pub name: String }
pub struct VideoSummary { pub track: Track, pub channel: Option<ChannelRef>, pub live: bool, pub views: Option<String> }
pub struct ChannelSummary { pub id: BrowseId, pub name: String, pub subscribers: Option<String>, pub thumbnails: Vec<Thumbnail> }
pub struct ChannelDetail { pub summary: ChannelSummary, pub uploads: Vec<VideoSummary>, pub playlists: Vec<PlaylistSummary> }
// SearchResults gains: pub videos: Vec<VideoSummary>, pub channels: Vec<ChannelSummary>  (Default keeps old callers working)
// Catalog gains, all defaulting to CatalogError::Unavailable:
fn search_scoped<'a>(&'a self, query: &'a str, scope: SearchScope) -> BoxFuture<'a, Result<SearchResults, CatalogError>>;
fn channel<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<ChannelDetail, CatalogError>>;
fn live_streams(&self) -> BoxFuture<'_, Result<Vec<VideoSummary>, CatalogError>>;

pub struct CommentId(pub String);
pub struct CommentCursor(pub String);          // opaque, untrusted text
pub struct Comment {
    pub id: CommentId, pub author: String, pub text: String, pub posted: Option<String>,
    pub likes: Option<u64>, pub reply_count: u32, pub liked: bool,
    pub replies: Option<CommentCursor>, pub mine: bool,
}
pub struct CommentPage { pub comments: Vec<Comment>, pub next: Option<CommentCursor>, pub can_post: bool }
pub enum CommentError {
    Disabled, SignInNeeded, RateLimited, Rejected, Network(String), Parse(String), Unavailable(String),
}

// nemo-core/src/services.rs
pub trait CommentSource: Send + Sync {
    fn comments<'a>(&'a self, video: &'a VideoId, page: Option<&'a CommentCursor>) -> BoxFuture<'a, Result<CommentPage, CommentError>>;
    fn replies<'a>(&'a self, cursor: &'a CommentCursor) -> BoxFuture<'a, Result<CommentPage, CommentError>>;
    fn post<'a>(&'a self, video: &'a VideoId, text: &'a str) -> BoxFuture<'a, Result<Comment, CommentError>>;
    fn reply<'a>(&'a self, to: &'a CommentId, text: &'a str) -> BoxFuture<'a, Result<Comment, CommentError>>;
    fn set_liked<'a>(&'a self, id: &'a CommentId, liked: bool) -> BoxFuture<'a, Result<(), CommentError>>;
}
pub enum Dislikes { Estimate(u64), Unknown }
pub trait VideoRatings: Send + Sync {
    fn dislikes<'a>(&'a self, video: &'a VideoId) -> BoxFuture<'a, Dislikes>;
}
// Backend gains: pub comments: Option<Arc<dyn CommentSource>>, pub ratings: Option<Arc<dyn VideoRatings>>
// player.rs: PlayerSnapshot gains `pub live: bool`

// nemo-ytm
pub struct WebCatalogExt;                       // search_scoped, channel, live_streams
pub struct YtmComments;                         // impl CommentSource
pub struct RydClient;                           // impl VideoRatings
pub const RYD_HOST: &str = "returnyoutubedislikeapi.com";

// nemo-core/src/captions.rs (new; `SubtitleTrack` is owned by video-playback, `VideoId` by track.rs)
pub struct SubtitleCue { pub start: Duration, pub end: Duration, pub text: String }
pub enum CaptionError { Unavailable(&'static str) /* yt-dlp missing or too old */, NoCaptions, Empty, Timeout, Network(String), Parse(String) }
pub trait SubtitleSource: Send + Sync {
    fn tracks<'a>(&'a self, video: &'a VideoId) -> BoxFuture<'a, Result<Vec<SubtitleTrack>, CaptionError>>;
    fn cues<'a>(&'a self, video: &'a VideoId, track: &'a SubtitleTrack) -> BoxFuture<'a, Result<Vec<SubtitleCue>, CaptionError>>;
}
// nemo-core/src/lyrics.rs: LyricsProvider gains `fn timeout(&self) -> Duration { Duration::from_secs(4) }` (ADR 0002 limit); captions return 20 s.
// nemo-ytm/src/captions.rs
pub struct YtmCaptions;                                  // impl SubtitleSource over the yt-dlp runner
pub struct CaptionsLyricsProvider;                       // impl LyricsProvider: id "YouTube captions", ProviderInfo::opt_in(..), hosts ["www.youtube.com"], Line
pub fn parse_json3(text: &str) -> Result<Vec<SubtitleCue>, CaptionError>;       // pure; input capped at 2 MiB
pub fn cues_to_doc(cues: Vec<SubtitleCue>, auto_generated: bool) -> Option<LyricsDoc>;  // pure; None when nothing is left
pub fn pick_track(tracks: &[SubtitleTrack], video_language: Option<&str>) -> Option<usize>;  // criterion 31
```

Needed from other designs: the signed request builder and `SessionSource` in `nemo-ytm` (`raw.rs`, account-actions design; both stay inside `nemo-ytm`, so no `nemo-core` item), and `SignOutWipe` (account-actions): `YtmComments` and `RydClient` register one step named "ytm-extras" that empties comment pages and the dislike cache; `UrlResolver`, `Transport`, `HlsSource` (the `soundcloud` spec); the shared opt-in settings pattern (ADR 0004); `VideoController` and `VideoFrameSource` (video-playback) for the picture of a video result.

## Error Handling

`CommentError::Disabled` shows "Comments are turned off". `SignInNeeded` hides the post actions. `Rejected` (the service refused the post, for example a spam filter) keeps the text and shows a plain line. `Parse` means YouTube changed its layout: the panel says comments are not available and nothing else breaks. RYD: 404, 400, `deleted: true`, timeouts and any parse failure give `Dislikes::Unknown`; a 429 gives `Unknown` for the 10-minute pause and no request is made in it. A video ID is not a secret, but the cookie header and the signed hash are: they pass through `Secret<T>` and are never logged. Captions: `NoCaptions`, `Empty` and a length mismatch give `Ok(None)` to the lyrics chain and "No subtitles" or "Subtitles unavailable" to video; `Unavailable` (yt-dlp missing or too old) shows the `yt-dlp` state of soundcloud's settings page; `Timeout` is `LyricsError::Network`. A bad json3 file is `Parse`; nothing from it is shown. The temp folder is removed after success, failure and timeout (a drop guard), and a start-up sweep removes leftovers. `yt-dlp` stderr is never logged, because it can echo URLs.
Live: a playlist reload that fails 3 times in a row, or an `#EXT-X-ENDLIST` line, ends the stream with an "ended" event; the app never restarts it.

## Testing Strategy

- Parsers from stripped JSON fixtures: web search (video, channel, live badge, mixed), channel page, comments in both layouts, replies, comments off, posting answers.
- Fake `CommentSource` and a counting fake `Catalog` prove nothing loads before the panel opens and that "Load more" is the only trigger for page two (criteria 11, 12).
- RYD against `test_server.rs`: 200, 404, 400, 429 (pause holds), deleted, timeout; the limiter with a fake clock; cache expiry at 24 h; one request per item.
- HLS live (shared with `soundcloud`): live window, reload failure, `ENDLIST`, TS packet parser on a captured 2-second sample.
- Captions: `parse_json3` on invented fixtures (a real song's captions are lyrics and stay out of the repository): plain uploaded track, auto-generated roll-up with repeated lines, `[Music]` and note-symbol lines, empty events, a 2 MiB cap, Vietnamese and CJK text; `pick_track` (uploaded before auto, `-orig` before translations, never a translation); the length check at 5 s and 3 percent. A fake `yt-dlp` script (the pattern near `ytdlp.rs`) prints a list and writes a json3 file: no `--cookies` argument, the temp folder is gone after success, failure and timeout, an empty file gives `Empty`, and with the switch off zero processes start. A chain test shows the provider is called last and only once per track.
- Sign-out test: comment pages, subtitle cues and RYD cache are empty afterwards.
- Audio: `cargo test -p nemo-audio --test bit_exact` must keep passing; the counting-allocator test must pass with a live source that underruns.
- Manual: one live stream, one video with comments off, one post and one reply on a test account.

## Risks and Alternatives

- Posting through the web client can look like automation and risk the account. Reads are the same requests a browser makes. The owner decides whether posting ships first or later (requirement Open Question 1). Alternative rejected: the official Data API (`commentThreads.insert`) needs an OAuth client the user would have to create.
- The innertube layouts change often; comments already changed shape in 2026. Mitigation: dual parser, fixtures, a plain unavailable state.
- Live audio may arrive only inside muxed HLS, so video bytes are downloaded and dropped. A hand-written TS audio extractor is the fallback if no crate fits; no crate was checked, and `symphonia` 0.6.1 is believed to have no TS reader (unverified).
- RYD is run by volunteers, may close or change terms, and its figure is an estimate. The setting is off by default, the label says so, and failure shows "Unknown".
- Captions need a PO token on some videos and `yt-dlp` may return an empty file (issue 13075, 2025). Mitigation: `yt-dlp` does the fetch and carries the token logic; an empty file is "no captions", never an error screen. Auto-generated text mishears words and has no punctuation, so it is labelled "Not lyrics-grade" and sits last in the chain. Alternatives rejected: the app calling `/api/timedtext` itself (needs its own PO token handling and a second request path to `www.youtube.com`); the innertube `player` endpoint (same token, and more client changes to track).
- The lyrics chain gives each provider 4 s (ADR 0002), and a `yt-dlp` run takes longer. The design adds a defaulted `LyricsProvider::timeout` (20 s here). This touches a `nemo-core` trait and ADR 0002, so the owner approves it with this spec. If refused, captions cannot work for lyrics and only the subtitle menu ships.
- Captions text of a song is lyrics text: shown, cached in the lyrics cache, never exported (`coc.md`). The 4-line share image of lyrics-plus is not offered for captions.
- Captions spike result (task 6, 2026-11-14, `yt-dlp` 2026.10.26 from winget, Windows 11, no cookies, `--ignore-config`, 13 runs spaced 20 to 30 s apart). **Pass**: non-empty json3 for tracks of all three songs. Songs, music-video id / audio id: Rick Astley "Never Gonna Give You Up" `dQw4w9WgXcQ` / `lYBUbBu4W08`; Luis Fonsi "Despacito" `kJQP7kiw5Fk` / `FXovf5dsRTw`; Adele "Hello" `YQHsXMglC9A` / `Ei8UnOPJX7w` (audio ids from the YouTube Music song search).
  - Empty body (issue 13075): not seen. Every json3 file had events: uploaded 61, 74 and 91 events; auto `en-orig` 104 and 140. One answer was HTTP 429 on the third caption file fetched within a second of the first two in one run; with `--sleep-subtitles 10` two files in one run passed.
  - Which id: the **audio ids carry no captions** (empty `subtitles` and `automatic_captions` for all three); the music-video ids carry them. So the music-video id lookup of task 10 is required, not a fallback that can be dropped.
  - Naming: the original auto track is `<code>-orig` (`en-orig`, name "English (Original)"); plain `<code>` keys in `automatic_captions` are machine translations (157 to 941 keys), and translations of an uploaded track are `<target>-<source key>` (`ab-en-nP7-2PuUl7o`). Uploaded keys are `<code>` (`en`, `es`) or `<code>-<track id>` (`en-nP7-2PuUl7o`, `en-US-njLgzgtehjs`); one video had three English uploads. The info dictionary's `language` was `en` for two music videos and `null` for "Despacito", which also had no `-orig` track. Some translated auto tracks list only `vtt`, served as an HLS playlist from `manifest.googlevideo.com`.
  - json3 shape: `wireMagic` `pb3`, `events[]` with `tStartMs`, `dDurationMs`, `segs[].utf8`. Uploaded: one event per caption, `\n` inside, note symbols around lines, `[♪♪♪]`. Auto: a first window event with no `segs`, word `segs` with `tOffsetMs` and `acAsrConf`, `aAppend: 1` events holding only `"\n"`, `[Music]` lines. Invented fixtures with this shape are in `crates/nemo-ytm/tests/fixtures/captions/`.
  - Length check (criterion 33): music video against audio track was 213 / 214 s, 282 / 229 s and 367 / 296 s, so 2 of 3 songs get no captions, as intended (the captions are timed to the video's intro). Captions as lyrics will rarely apply; the owner is told.
  - Hosts: a plain `-J --skip-download` run contacted `www.youtube.com` (watch page, two `youtubei/v1/player` calls, player JS) and `manifest.googlevideo.com` (the HLS manifest). With `--extractor-args youtube:skip=hls,dash` it contacted only `www.youtube.com` and still listed every track. json3 files come from `www.youtube.com/api/timedtext`.
- Departure from ADR 0004: none. `Catalog` is extended with defaulted methods, as `home_feed` was.

## Steering and README changes

- Opt-in: "All of YouTube" search, comments, dislike counts and the YouTube captions provider each use the shared `OptInRow` and `OptInInfo` (ADR 0004, "Opt-in settings pattern"). Hosts: `www.youtube.com`, and `returnyoutubedislikeapi.com` for dislike counts only. The texts are in criteria 10, 17, 27 and 30.
- `tech.md`: no new crate and no new row for captions (`serde_json` and `yt-dlp` are decided). If `m3u8-rs` 6.0.1 is not yet added by the `soundcloud` task, this one adds its row (MIT, released 2026-07-30, `rust-version` unverified).
- `structure.md`: no new crate or edge.
- README egress list: add `returnyoutubedislikeapi.com` (only when dislike counts are on; sent: the video ID and the IP). `www.youtube.com` and `music.youtube.com` are listed already; note that `www.youtube.com` is also used for comments, all-YouTube search and captions (fetched by `yt-dlp`, only when the captions provider is on or a subtitle is picked) when those are on.
- ADR 0002: add the "YouTube captions" row (off by default, risk note, last in the chain) and `LyricsProvider::timeout`. ADR 0004: add `CommentSource`, `VideoRatings`, `SubtitleSource` and the `Catalog` additions to Decision 2 (done in the Decision 4 table).

<!-- Sources:
crates/nemo-ytm/src/{catalog,raw,innertube,feeds}.rs, crates/nemo-core/src/{catalog,services}.rs, .kiro/specs/youtube-extras/requirements.md, docs/adr/0004-feature-wave-crates.md
https://github.com/Anarios/return-youtube-dislike/blob/main/README.md (API, attribution, 100/min and 10,000/day); https://returnyoutubedislike.com/terms (404 on 2026-11-14)
https://github.com/youtube/api-samples/issues/360 (perform_comment_action path); https://dev.to/_b5c55d74e876409d257ca/youtube-changed-how-comments-work-internally-heres-what-broke-1klg (2026 comment layout change)
https://github.com/bjarneo/cliamp/issues/280, https://github.com/yt-dlp/yt-dlp/issues/16420, https://github.com/yt-dlp/yt-dlp/issues/17486 (live formats)
https://github.com/yt-dlp/yt-dlp/issues/13075 (timedtext needs a PO token); https://github.com/yt-dlp/yt-dlp/wiki/PO-Token-Guide; yt-dlp README, "Subtitle Options" and the `subtitles` and `automatic_captions` fields (read 2026-11-14)
crates/nemo-core/src/lyrics.rs (LyricsProvider, LyricsCredit, ProviderInfo); .kiro/specs/{video-playback,lyrics-plus}/design.md
-->
