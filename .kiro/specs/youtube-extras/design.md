## Overview

Four opt-in parts, all in `nemo-ytm` and the catalog types of `nemo-core`: search over all of YouTube, live streams, comments, and dislike counts from Return YouTube Dislike (RYD). The external chart site is dropped (owner, 2026-11-14). No new crate and no new dependency.

The catalog today is `ytmapi-rs` 0.3.4 against `music.youtube.com` (`catalog.rs`, `raw.rs`). `ytmapi-rs` has no video search, channel pages or comments. These parts use raw innertube requests to `www.youtube.com/youtubei/v1/` with the web client, built on the `ClientProfile` and request code in `innertube.rs`. Both hosts are already on the README egress list. Nothing runs unless its setting is on, and nothing loads in the background.

Innertube calls, from community clients and docs and not yet confirmed against the live service (the implementing task runs a spike and stores stripped fixtures):
- Search all of YouTube: `POST /youtubei/v1/search` with `query` and no `params`. The answer mixes `videoRenderer`, `channelRenderer` and `playlistRenderer`; a live item carries a "LIVE NOW" badge. The music search of `music.youtube.com` returns only music shelves, so "All of YouTube" runs the music search first and then this one, one after the other.
- Channel page: `POST /youtubei/v1/browse` with the `UC...` id. Tab `params` come from the tab endpoints in the answer, not from constants.
- Read comments: `POST /youtubei/v1/next` with `videoId` gives a continuation token for the comments section; a second `next` with that token gives threads. Replies use the token in each thread. Newer answers carry comment data in `frameworkUpdates.entityBatchUpdate.mutations` with a `commentViewModel` instead of `commentRenderer`; the parser reads both.
- Post, reply, like: `POST /comment/create_comment` (`createCommentParams`, `commentText`), `/comment/create_comment_reply` (`createReplyParams`, `commentText`) and `/comment/perform_comment_action` (`actions`). The params and action tokens are copied from the read answer, never built. Each call needs the signed-in session (cookie plus `SAPISIDHASH` header, as the `account-actions` design sets up).
- Comments off: the read answer holds a message item instead of a token.

RYD (README of the project, read 2026-11-14): `GET https://returnyoutubedislikeapi.com/votes?videoId=<id>`; third-party use is allowed if the project is clearly attributed with a link to returnyoutubedislike.com; limits are 100 requests a minute and 10,000 a day per client, answered with 429; 404 means no such video, 400 a malformed id. Its terms page (`/terms`) answered 404 on 2026-11-14 and its Swagger page was not read, so the README is the only terms source; the design follows it and the owner should re-check the site before release. The answer's `dislikes` is an estimate; `rawDislikes` is ignored.

## Affected Crates

- `nemo-core`: `catalog.rs` gets `SearchScope`, video and channel types, `ChannelDetail`, comment types, `Catalog::search_scoped`, `Catalog::channel`, `Catalog::live_streams`; `services.rs` gets the `CommentSource` and `VideoRatings` traits and two fields on `Backend`; `player.rs` gets `PlayerSnapshot.live`.
- `nemo-ytm`: `web_search.rs` (search and channel parsers), `comments.rs` (read, post, reply, like), `ryd.rs` (client, limiter, cache), `live.rs` (live list from feed badges and the `UrlResolver` call).
- `nemo-ui`: scope switch, video and channel results, Live area, comments panel, dislike label, the two settings entries, the one-time note for "All of YouTube".
- `nemo-app`: builds the new services only when their switches are on.
- `nemo-audio`: none, apart from the HLS source designed in the `soundcloud` spec (live streams use it).

## Data Flow

Search: UI task -> `Catalog::search_scoped(text, AllOfYoutube)` -> music search, then web search -> one `SearchResults` with `videos` and `channels` filled. The scope `Music` calls `search` as today (criterion 5).

Live: the `UrlResolver::resolve` of the `soundcloud` design runs `yt-dlp` for the video URL. A live YouTube page may offer only muxed HLS or split video-only and audio-only HLS (community reports differ and change with the client). The policy picks audio-only HLS if it exists, else the lowest-bandwidth muxed rendition. `ResolvedStream.transport = Hls { live: true }`. The shared `HlsSource` in `nemo-ytm` reloads the playlist every half target duration, starts 3 segments behind the live edge, and keeps a bounded channel of 3 segments to the decoder thread. If the container is MPEG-TS, a small `ts.rs` takes the AAC audio packets out of the stream (stream type 0x0F, output as ADTS) and drops the video; the video bytes are still downloaded (at 144p, a few hundred kbit/s). If it is fragmented MP4, `symphonia` picks the audio track. The stream has no length and `is_seekable() = false`. The audio callback is unchanged: decoder -> `rtrb` ring -> `cpal`; an empty ring already gives silence without allocation.

Comments: opening the panel -> `CommentSource::comments(video, None)` -> one `next` for the token, one for the first page; "Load more" passes the cursor. Posting is one call per press; the UI keeps the text until the call succeeds (criterion 16). Pages for the current item live in memory only and are dropped when the item changes or on sign-out.

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
```

Needed from other designs: the signed-in innertube request helper and sign-out registry (`account-actions`, multi-account); `UrlResolver`, `Transport`, `HlsSource` (the `soundcloud` spec); the shared opt-in settings pattern; the `video-playback` trait for pictures.

## Error Handling

`CommentError::Disabled` shows "Comments are turned off". `SignInNeeded` hides the post actions. `Rejected` (the service refused the post, for example a spam filter) keeps the text and shows a plain line. `Parse` means YouTube changed its layout: the panel says comments are not available and nothing else breaks. RYD: 404, 400, `deleted: true`, timeouts and any parse failure give `Dislikes::Unknown`; a 429 gives `Unknown` for the 10-minute pause and no request is made in it. A video ID is not a secret, but the cookie header and the signed hash are: they pass through `Secret<T>` and are never logged. Live: a playlist reload that fails 3 times in a row, or an `#EXT-X-ENDLIST` line, ends the stream with an "ended" event; the app never restarts it.

## Testing Strategy

- Parsers from stripped JSON fixtures: web search (video, channel, live badge, mixed), channel page, comments in both layouts, replies, comments off, posting answers.
- Fake `CommentSource` and a counting fake `Catalog` prove nothing loads before the panel opens and that "Load more" is the only trigger for page two (criteria 11, 12).
- RYD against `test_server.rs`: 200, 404, 400, 429 (pause holds), deleted, timeout; the limiter with a fake clock; cache expiry at 24 h; one request per item.
- HLS live (shared with `soundcloud`): live window, reload failure, `ENDLIST`, TS packet parser on a captured 2-second sample.
- Sign-out test: comment pages and RYD cache are empty afterwards.
- Audio: `cargo test -p nemo-audio --test bit_exact` must keep passing; the counting-allocator test must pass with a live source that underruns.
- Manual: one live stream, one video with comments off, one post and one reply on a test account.

## Risks and Alternatives

- Posting through the web client can look like automation and risk the account. Reads are the same requests a browser makes. The owner decides whether posting ships first or later (requirement Open Question 1). Alternative rejected: the official Data API (`commentThreads.insert`) needs an OAuth client the user would have to create.
- The innertube layouts change often; comments already changed shape in 2026. Mitigation: dual parser, fixtures, a plain unavailable state.
- Live audio may arrive only inside muxed HLS, so video bytes are downloaded and dropped. A hand-written TS audio extractor is the fallback if no crate fits; no crate was checked, and `symphonia` 0.6.1 is believed to have no TS reader (unverified).
- RYD is run by volunteers, may close or change terms, and its figure is an estimate. The setting is off by default, the label says so, and failure shows "Unknown".
- Departure from ADR 0004: none. `Catalog` is extended with defaulted methods, as `home_feed` was.

## Steering and README changes

- `tech.md`: no new crate. If `m3u8-rs` 6.0.1 is not yet added by the `soundcloud` task, this one adds its row (MIT, released 2026-07-30, `rust-version` unverified).
- `structure.md`: no new crate or edge.
- README egress list: add `returnyoutubedislikeapi.com` (only when dislike counts are on; sent: the video ID and the IP). `www.youtube.com` and `music.youtube.com` are listed already; note that `www.youtube.com` is also used for comments and all-YouTube search when those are on.
- ADR 0004: add `CommentSource`, `VideoRatings` and the `Catalog` additions to Decision 2.

<!-- Sources:
crates/nemo-ytm/src/{catalog,raw,innertube,feeds}.rs, crates/nemo-core/src/{catalog,services}.rs, .kiro/specs/youtube-extras/requirements.md, docs/adr/0004-feature-wave-crates.md
https://github.com/Anarios/return-youtube-dislike/blob/main/README.md (API, attribution, 100/min and 10,000/day); https://returnyoutubedislike.com/terms (404 on 2026-11-14)
https://github.com/youtube/api-samples/issues/360 (perform_comment_action path); https://dev.to/_b5c55d74e876409d257ca/youtube-changed-how-comments-work-internally-heres-what-broke-1klg (2026 comment layout change)
https://github.com/bjarneo/cliamp/issues/280, https://github.com/yt-dlp/yt-dlp/issues/16420, https://github.com/yt-dlp/yt-dlp/issues/17486 (live formats)
-->
