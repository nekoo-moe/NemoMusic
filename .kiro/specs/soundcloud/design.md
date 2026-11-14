## Overview

SoundCloud is a second catalog next to YouTube Music. Public tracks only, no sign-in, all through `yt-dlp` (owner, 2026-11-14). A new crate `nemo-soundcloud` turns `yt-dlp` metadata into the catalog types the UI already shows. It never starts a process or opens a socket itself: it calls the `UrlResolver` trait in `nemo-core`, which `nemo-ytm` implements with its existing `yt-dlp` code (`ytdlp.rs`). Downloads and video will use the same trait.

Three things are new on the playback side. (1) `Origin::SoundCloud` and a `Track.source_url`, because `Track.video_id` only holds YouTube ids. (2) A format ranking that chooses by how a stream sounds, not by the highest `abr` number as `ytdlp.rs::parse_with` does today. (3) HLS playback, because `nemo-ytm/src/stream.rs` reads one ranged HTTPS file. The HLS reader lives in `nemo-ytm` (module `hls`) and is shared with YouTube live streams (`youtube-extras`).

Why `nemo-ytm` for HLS: the reader needs the HTTPS-only redirect rules, the retry rules, the URL redaction and the client of `http.rs`, and ADR 0004 already gives `nemo-ytm` live streams. A copy in `nemo-audio` would need `reqwest` there. A separate crate would need an edge to `nemo-ytm` or a copy of `http.rs`. `nemo-soundcloud` needs no reader: `nemo-app` hands the player one `StreamOpener` (`YtmStreamOpener`) that looks at `ResolvedStream.transport` and reads a file or HLS.

## Affected Crates

- `nemo-core`: `audio.rs` gets `Origin::SoundCloud` (always `QualityClass::Lossy`, never lossless). `track.rs` gets `MediaUrl` and `Track.source_url`. `stream.rs` gets `Transport` and two fields on `ResolvedStream`. New file `url_resolver.rs` (trait, entries, ranking function). `services.rs` gets `ByteSource::is_seekable` (default true) and `Backend.soundcloud: Option<Arc<dyn Catalog>>`.
- `nemo-soundcloud` (new): `SoundcloudCatalog` implements `Catalog` (search, track, playlist and user pages; `album` answers `NotFound`), the host list, the settings entry data, the draft risk text.
- `nemo-ytm`: `YtDlpUrlResolver` implements `UrlResolver`; `hls.rs` (playlist reading, segment fetch, live window); `YtmStreamOpener` replaces the direct use of `HttpStreamOpener`.
- `nemo-audio`: `prepare.rs::prepare` gets a branch for `Origin::SoundCloud` through `PrepareDeps.url_resolver`; `source.rs::MediaAdapter::is_seekable` forwards the new flag; the Opus decoder comes from the `local-library` design (see Risks).
- `nemo-ui`: SoundCloud result group, pages, settings entry, the "Preview" tag. Reads only `nemo-core` types.
- `nemo-app`: builds `SoundcloudCatalog` only when the switch is on and injects the resolver and opener.

## Data Flow

Search: UI task -> `SoundcloudCatalog::search` -> `UrlResolver::list` (`yt-dlp --flat-playlist -J "scsearch10:<text>"`, one process, 20 s timeout) -> `MediaEntry` list -> mapped to `SearchResults` (tracks to `songs`, playlists, users to `artists`). Opening a page is one more `list` call on the entry URL. Calls are one at a time, human-paced, and the switch off means the catalog is never built, so nothing is contacted (criterion 2).

Play: the control thread calls `prepare` on the decoder-prepare thread. `UrlResolver::resolve(url, policy)` runs `yt-dlp -J` once, reads every audio format, drops encrypted and non-audio ones, builds `FormatCandidate`s and calls `rank_formats`. The answer is a `ResolvedStream` with `transport` = `File` or `Hls`, `preview`, and measured `StreamInfo`. `YtmStreamOpener::open` returns a `ByteSource`. For HLS the source owns one fetch thread and a bounded `std::sync::mpsc::sync_channel` of segment buffers (3 segments ahead). The decoder thread reads from the current buffer. The audio callback is unchanged: decoder -> `rtrb` ring -> `cpal`. No new work happens in the callback.

Seek: a file stream seeks by byte as now. An HLS on-demand stream maps the time to a segment through the `EXTINF` sums, restarts the fetch at that segment and reports `is_seekable() = true`; the decoder's accurate-seek skip trims the rest. Live HLS reports `is_seekable() = false`.

Stream choice (`rank_formats`): candidates are the audio formats the decoder can play now (`supported_stream_codecs()` at run time, so Opus is out until the decoder lands and the owner approves adding it to that list, ADR 0004). Score = bitrate x efficiency: Opus 2.0, AAC 1.25, Vorbis 1.15, MP3 1.0. The highest score wins; a tie goes to the newer codec (Opus, AAC, MP3). A plain file beats HLS of the same codec. A missing bitrate scores 0 but stays playable. The factors are a first heuristic from the `yt-dlp` issue that finds 64 kbps Opus better than 128 kbps MP3; the owner's ears settle them in a listening test (Open item in Risks). Previews rank below every full stream and are never replaced by a stream of another site (criterion 10).

## Interfaces

```rust
// nemo-core/src/track.rs
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MediaUrl(String);
impl MediaUrl {
    /// https only, no credentials in the URL, host must match one of `hosts` (exact or `*.suffix`).
    pub fn parse(text: &str, hosts: &[&str]) -> Result<Self, InvalidMediaUrl>;
    pub fn as_str(&self) -> &str;
}
pub struct InvalidMediaUrl;
// Track gains: pub source_url: Option<MediaUrl>   (id is "sc:<numeric id>")

// nemo-core/src/stream.rs
pub enum Transport { File, Hls { live: bool } }
// ResolvedStream gains: pub transport: Transport, pub preview: bool

// nemo-core/src/services.rs
pub trait ByteSource { fn is_seekable(&self) -> bool { true } /* byte_len unchanged */ }

// nemo-core/src/url_resolver.rs
pub enum Site { SoundCloud, Youtube }
pub enum MediaQuery { Url(MediaUrl), Search { site: Site, text: String, limit: u8 } }
pub enum EntryKind { Track, Playlist, User }
pub struct MediaEntry {
    pub kind: EntryKind, pub url: MediaUrl, pub id: String, pub title: String,
    pub uploader: Option<String>, pub uploader_url: Option<MediaUrl>,
    pub duration: Option<Duration>, pub thumbnails: Vec<Thumbnail>, pub live: bool,
}
pub struct FormatPolicy { pub playable: Vec<Codec>, pub allow_hls: bool }
pub struct FormatCandidate {
    pub index: usize, pub codec: Codec, pub bitrate_kbps: Option<u32>,
    pub hls: bool, pub encrypted: bool, pub preview: bool,
}
pub fn rank_formats(candidates: &[FormatCandidate], policy: &FormatPolicy) -> Option<usize>;
pub enum ResolverState { Ready { version: String }, Missing, TooOld }
pub trait UrlResolver: Send + Sync {
    fn resolve<'a>(&'a self, url: &'a MediaUrl, policy: &'a FormatPolicy)
        -> BoxFuture<'a, Result<ResolvedStream, StreamError>>;
    fn list<'a>(&'a self, query: &'a MediaQuery)
        -> BoxFuture<'a, Result<Vec<MediaEntry>, StreamError>>;
    fn state(&self) -> ResolverState;
}

// nemo-soundcloud
pub const HOSTS: &[&str];
pub struct SoundcloudCatalog { /* Arc<dyn UrlResolver> */ }
impl SoundcloudCatalog { pub fn new(resolver: Arc<dyn UrlResolver>) -> Self; }
impl Catalog for SoundcloudCatalog { /* search, playlist, artist; album = NotFound */ }

// nemo-ytm
pub struct YtDlpUrlResolver;
pub struct YtmStreamOpener;            // impl StreamOpener: File or Hls
pub struct HlsSource;                  // impl ByteSource
```

Needed from other designs: the Opus decoder (`local-library`), the shared opt-in settings entry (switch, what is sent, hosts, risk text), the sign-out registry (SoundCloud keeps no account data, so it registers nothing).

## Error Handling

`StreamError` gains no variant. Encrypted-only tracks map to `NotAvailable`, and `nemo-soundcloud` adds the text "cannot play here" through `CatalogError::Unavailable`. A 429 or a `yt-dlp` rate message gives a backoff of at least 60 s for all SoundCloud calls; the queue goes to the next item (criterion 12). `ResolverState::Missing` or `TooOld` shows the `yt-dlp` state on the settings page (criterion 16). HLS: a segment that fails three times ends the stream with `Network`; a playlist that lists a key (`#EXT-X-KEY` other than `NONE`) ends with `NotAvailable` before any segment is fetched, so the app never reads an encrypted stream. URLs are `Secret<String>` and never appear in errors or logs.

## Testing Strategy

- Pure tests: `rank_formats` with the cases in the issue (64 kbps Opus vs 128 kbps MP3), Opus not playable, encrypted dropped, preview last, tie rules. `MediaUrl::parse` host and scheme rejection.
- Parser tests: `yt-dlp` JSON fixtures for search, track, playlist, user, preview and DRM answers (fixtures are captured during the spike and stripped of URLs).
- HLS tests against `test_server.rs`: media playlist with `EXTINF`, a key line (refused), a segment 429 (retried), seek to a segment, live window drop.
- A fake `UrlResolver` drives `SoundcloudCatalog` tests; no process starts.
- Audio: `cargo test -p nemo-audio --test bit_exact` must still pass. The real-time path is unchanged, so the counting-allocator test only needs to keep passing.
- Manual: one public track, one playlist, one preview, one encrypted track; a network capture lists the hosts (criterion 24).

## Risks and Alternatives

- `yt-dlp` breaks when SoundCloud changes. The extractor also scrapes a `client_id`. Mitigation: show the `yt-dlp` state, keep the feature off by default, no fallback to the official API.
- AAC HLS is fragmented MP4 (a `lavaplayer` change describes it). Whether `symphonia` 0.6.1 reads fMP4 without an index is unverified. The spike decides: if it fails, `HlsSource` joins the init segment and the fragments into one stream, and if that fails too the format is dropped from `playable` and SoundCloud shows the track as not playable.
- No Opus decoder yet, so today's pick is AAC or MP3. A listening test on 5 tracks sets the factors in `rank_formats`.
- Terms of Service: scraping through `yt-dlp` can be refused by SoundCloud. Draft risk text for approval: "SoundCloud is not an official integration. NemoMusic reads public pages the way a browser does, through yt-dlp. SoundCloud may block this, and its terms may not allow it. Off by default." Hosts shown before the first request: the list approved after the capture.
- Alternative rejected: the official API (needs an Artist Pro key, terms for a desktop player unverified). Alternative rejected: HLS in `nemo-audio` (needs an HTTP client there) or in a new crate (needs an edge ADR 0004 forbids).
- Departure from ADR 0004: none. The ADR says `nemo-soundcloud` uses the URL-resolver trait; this design adds `list` to it because search and pages also run through `yt-dlp`.

## Steering and README changes

- `tech.md`: row for `m3u8-rs` 6.0.1 (MIT, released 2026-07-30, `rust-version` unverified, pure Rust, parser only; verify its `nom` version against `Cargo.lock` `nom` 8.0.0). No new row for `yt-dlp` (already decided). Note that `symphonia` fMP4 and Opus support is checked in the spike.
- `structure.md`: add `nemo-soundcloud` (depends on `nemo-core` only); no new edge.
- README egress list: `soundcloud.com`, `api-v2.soundcloud.com`, `*.sndcdn.com`, `playback.media-streaming.soundcloud.cloud`, each marked "only when SoundCloud is on"; final list from the capture, approved by the owner.
- ADR 0004: add `UrlResolver::list`, `Transport`, `Track.source_url`, `ByteSource::is_seekable` to Decision 2's expected additions; record that HLS lives in `nemo-ytm`.

<!-- Sources:
crates/nemo-core/src/{audio,stream,services,track,catalog}.rs, crates/nemo-ytm/src/{ytdlp,stream,http}.rs, crates/nemo-audio/src/{prepare,decoder,source}.rs
.kiro/specs/soundcloud/requirements.md, docs/adr/0004-feature-wave-crates.md
https://github.com/yt-dlp/yt-dlp/issues/6668 (64k Opus vs 128k MP3), https://github.com/yt-dlp/yt-dlp/issues/17335 (encrypted HLS)
https://github.com/lavalink-devs/lavaplayer/pull/202 (SoundCloud AAC HLS is fragmented MP4)
https://crates.io/crates/m3u8-rs (6.0.1, MIT, 2026-07-30)
-->
