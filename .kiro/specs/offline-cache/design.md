## Overview

The offline cache saves songs the user picks, plus a capped set of the most-played songs, as the files YouTube served, and plays them with no network. It lives in `nemo-ytm` (ADR 0004) as a new `offline` module: plain files with opaque names in `AppPaths.offline` (new, `<root>/offline`) and one SQLite index, `offline.db`. No encryption: the owner decided it gives no real protection, and the index next to the files lists titles anyway. The opaque names only keep a casual folder browse from showing song names.

How it fits the code that exists:
- Lookup stays `StreamResolver` (`yt-dlp`, human-paced). The downloader asks it for a `ResolvedStream`, then reads the URL itself, one connection, in order.
- Playback reuses `FileSource` and `StreamDecoder::open(source, None)` from `nemo-audio`, the same call a YouTube stream uses, so a cached Opus-in-WebM or AAC-in-MP4 file needs no container hint and no re-encode.
- `prepare()` in `nemo-audio/src/prepare.rs` checks the offline store before the resolver. A hit makes no lookup and no audio request (criterion 3).
- Lyrics, SponsorBlock segments and covers are fetched once at save time through the existing `nemo-core` traits (`LyricsService`, `SegmentSource`, `ImageSource`), so `nemo-ytm` does not depend on `nemo-lyrics`.

Limits (owner, 2026-11-14): default size limit 2 GB; smart downloads at most 50 songs and 25% of the limit; one download at a time, 10 to 30 seconds between songs. A song YouTube removes or blocks is deleted and the user is told.

## Affected Crates

- `nemo-core`: new `downloads.rs` with `DownloadService`, `OfflineStore`, `NetworkMode` and their types. `services.rs`: `LyricsService` gets a defaulted `keep_offline(&self, track_key: &str, keep: bool)` so cached lyrics do not expire.
- `nemo-ytm`: new `offline/` (`store.rs` SQLite and files, `worker.rs` queue and pacing, `fetch.rs` ranged download, `smart.rs` selection, `verify.rs` takedown check, `catalog.rs` an offline `Catalog` over the index, `gate.rs` request gates). New dependencies: `rusqlite` =0.41.1 (same as `nemo-lyrics`), `fs4` =1.1.0, `getrandom` =0.3.4.
- `nemo-audio`: `prepare.rs` (offline check, `PrepareDeps.offline`, bitrate measured from file size and duration); no decoder change. `FileSource` is reused as is.
- `nemo-lyrics`: implements `keep_offline` (a `pinned` flag that skips expiry in `cache.rs`). Contact point with the lyrics-plus design.
- `nemo-ui`: Save offline actions, saved mark, storage view, offline switch and shortcut, smart-downloads opt-in.
- `nemo-app`: builds the store, worker, gates and the offline `Catalog`; picks the Premium resolver when the account-actions switch is on; calls `record_play`; runs `OfflineStore::wipe` at sign-out.

## Data Flow

- Save: the UI calls `DownloadService::save`. The service writes `queue` rows (`queued`) and wakes one worker thread, `nemo-downloads`. It is a `std` thread using blocking `reqwest` (already a dependency) and `Handle::block_on` for the resolver future. No other thread downloads.
- One song at a time: resolve, then `GET` with one `Range: bytes=<part length>-` stream into `tmp/<opaque>.part`, in 1 MiB reads, checking the pause flag between reads. After the last byte it compares the length to `Content-Length`, syncs, renames to `blobs/<opaque>`, and in one SQL transaction inserts the `song` row and the segments. Cover bytes go to `art/<opaque>`; lyrics are fetched with `LyricsService::lookup` and pinned.
- Pacing: after each song the worker sleeps a `Jitter` value, uniform 10 to 30 s (`getrandom`; injectable for tests). A resolver or network failure backs off 60 s, doubling to 30 min, and honors `Retry-After` on 429. After five failures in a row the queue pauses and tells the user. No retry runs without the wait; the user can retry one song by hand.
- Pause: an atomic flag read between chunks. The `.part` file stays; resume re-resolves (URLs expire) and continues with `Range`, restarting if the server answers 200.
- Play: `prepare()` calls `OfflineStore::open(&VideoId)`, gets `CachedAudio { path, info }`, opens `FileSource`, probes. The badge uses the probed codec and a bitrate of `bytes * 8 / duration`; it reads "Unknown" if either is missing and never says Lossless, HD or Hi-Res (`Origin::YoutubeMusic` keeps `quality_class` at Lossy). If the open or probe fails, the store marks the song unsaved; online playback then falls back to the stream, offline shows an error.
- Segments: `OfflineSegmentSource` answers from the `segment` table for saved songs and from the online source otherwise. Covers: `OfflineImages` does the same for `ImageSource`.
- Offline mode: `NetworkMode` is one shared flag. When on, `nemo-app` swaps in the offline `Catalog`, and the gates around the resolver, stream opener, segment source and image source refuse at once, so no request reaches a YouTube host. It switches on by itself after two network failures within 30 s on user actions, with one notice; only the user switches it off. No connectivity probe is sent.
- Smart downloads: `nemo-app` calls `record_play` on the same condition `nemo-stats` counts a play. Counts live in `play_count` in this computer's index. After a play and at start, `smart.rs` picks the most played songs that are not saved (at least 3 plays, a proposal) and enqueues them with reason `Smart`. Before a smart save it evicts the least played smart songs until both caps hold. `User` songs are never evicted.
- Takedown check: `verify.rs` re-resolves one saved song at a time, at most one every 10 to 30 s, each song about every 30 days (a proposal), only online and idle. `NotAvailable` twice in a row deletes the file and index row and shows "removed by YouTube". This is the only way a removal is seen, because cached play makes no request.
- Sign-out: `OfflineStore::wipe` closes the connection, removes `<root>/offline` completely (songs, art, index, play counts, queue) and returns the file count for `SignOutReport.files_removed`. With several accounts, signing out of any one wipes the whole cache: simpler and private.

## Interfaces

```rust
// nemo-core/src/downloads.rs (new)
pub enum SaveReason { User, Smart }
pub enum CollectionRef { Album(BrowseId), Playlist(BrowseId) }
pub enum FailReason { LimitReached, DiskFull, NotAvailable, Network, RateLimited, Damaged }
pub enum SaveState { NotSaved, Queued, Saving { done: u64, total: Option<u64> }, Saved, Failed(FailReason) }
pub struct SaveRequest { pub tracks: Vec<Track>, pub collection: Option<CollectionRef> }
pub struct StorageReport {
    pub used_bytes: u64, pub free_bytes: u64, pub limit_bytes: u64,
    pub smart_bytes: u64, pub art_bytes: u64, pub collections: Vec<(CollectionRef, String, u64)>,
}
pub enum DownloadEvent { State { video: VideoId, state: SaveState }, Removed { video: VideoId, reason: RemovedReason }, QueuePaused(PauseReason) }
pub enum RemovedReason { User, RemovedByYoutube, Damaged, SmartEviction }

pub trait DownloadService: Send + Sync {
    fn save(&self, request: SaveRequest) -> BoxFuture<'_, Result<(), DownloadError>>;
    fn state(&self, video: &VideoId) -> SaveState;        // cheap, safe every frame
    fn pause(&self);
    fn resume(&self);
    fn retry(&self, video: &VideoId);
    fn remove_track(&self, video: &VideoId);
    fn remove_collection(&self, collection: &CollectionRef);   // keeps songs another collection or the user still holds
    fn clear_all(&self) -> BoxFuture<'_, Result<u64, DownloadError>>;  // freed bytes
    fn storage(&self) -> StorageReport;
    fn set_limit(&self, bytes: u64);
    fn set_smart(&self, on: bool);
    fn subscribe(&self) -> Box<dyn DownloadReceiver>;
}

pub struct CachedAudio { pub path: Secret<PathBuf>, pub info: StreamInfo }
pub trait OfflineStore: Send + Sync {
    fn open(&self, video: &VideoId) -> Option<CachedAudio>;
    fn segments(&self, video: &VideoId) -> Option<Vec<Segment>>;
    fn mark_unsaved(&self, video: &VideoId);
    fn record_play(&self, video: &VideoId);
    fn wipe(&self) -> Result<u64, DownloadError>;
}
pub trait NetworkMode: Send + Sync { fn is_offline(&self) -> bool; fn set_offline(&self, on: bool); }

// nemo-ytm/src/offline
pub struct OfflineCache { /* index, folders, limit */ }
impl OfflineCache { pub fn open(dir: &Path, clock: Arc<dyn Clock>) -> Result<Self, OfflineError>; }  // `Clock` is a small trait of this module, like the one in `nemo-lyrics`
pub trait Jitter: Send + Sync { fn pause(&self) -> Duration; }   // 10..=30 s
```

`Secret<PathBuf>` keeps the path out of logs; only `prepare()` calls `expose`. The path never reaches the UI. `PrepareDeps` gains `offline: Option<Arc<dyn OfflineStore>>`. The Premium switch is read by `nemo-app`, which hands the worker the Premium or the anonymous `StreamResolver`; I need that switch and the Premium resolver from the account-actions design.

## Error Handling

- `OfflineError` and `DownloadError` (`thiserror`): `Db`, `Io`, `Resolve`, `Http`, `LimitReached`, `DiskFull`, `Damaged`. No `unwrap`. URLs, file names and video ids are never logged.
- A failed song is marked `Failed` with a plain reason and the worker moves on; the user retries by hand (criterion 12).
- Limit: before each download the worker adds the expected size (from the first response) to `used_bytes`; over the limit, the song fails `LimitReached`. Free disk under size plus 256 MiB (a proposal) fails `DiskFull`.
- Start-up cleanup: leftover `.part` files without a queue row are deleted; `song` rows whose file is missing are marked unsaved (criterion 24). A damaged index is deleted with its files and the user is told; saved songs are lost, which is the safe side for a cache.
- Writes go to `.part` then rename, so a crash never leaves a half file that looks saved.

## Testing Strategy

- Store: reopen, migration, wipe leaves no file under `offline/`, no file name contains a video id or title, sign-out removes play counts and queue.
- Worker with a fake clock and fake `Jitter`: never two fetches at once; every gap is 10 to 30 s; 429 with `Retry-After` waits that long; failures back off and stop after five; pause stops within one chunk; resume continues from the `.part` length. The loopback `test_server.rs` serves ranges and 429.
- Caps with `proptest`: random play counts and sizes never push smart songs past 50 or 25% of the limit and never evict a `User` song; user saves stop at the limit.
- Takedown: two `NotAvailable` answers delete the song; one does not.
- `cargo test -p nemo-audio --test bit_exact`: add a test that a cached copy of the fixture decodes to the same samples as the streamed copy, and that the bit-exact result is unchanged with the cache present but unused.
- `cargo test -p nemo-audio --test rt_no_alloc` (counting allocator): add a run playing from a cached file; the callback allocates nothing.
- Badge: `quality_class` stays Lossy for cached songs; bitrate is Unknown when duration is missing.
- Offline mode: with the gate on, a recording fake host sees zero requests while saved songs browse and play.

## Risks and Alternatives

- Background fetching can look like scraping and YouTube may restrict the account. Mitigations: opt-in smart downloads with the risk text, one connection, 10 to 30 s gaps, backoff, caps. The behavior of YouTube toward this is unverified; the numbers are unmeasured.
- Takedown detection needs recurring lookups (about one song per 10 to 30 s, each song every 30 days) because cached play makes no request. This adds traffic to hosts already listed. The 30 days is a proposal; the owner can lengthen it or accept passive detection only.
- Network loss is detected from failed user requests, not from the OS. A wrong switch to offline mode costs the user one click; a missed switch costs a visible error.
- `yt-dlp` or URL changes can break saves; saved songs keep playing because they make no lookup. Premium downloads depend on the account-actions design.
- Alternatives rejected: encryption with a key in the OS store (owner decision); a new `nemo-cache` crate (ADR 0004 puts it in `nemo-ytm`); `HttpStream` as the downloader (it treats 429 as a plain retry and hides `Retry-After`); `sysinfo` for free space (heavy, MSRV 1.95). `fs4` 1.1.0 (MIT OR Apache-2.0, MSRV 1.75, April 2026) gives `available_space`; its C toolchain needs are unverified (expected none).
- No departure from ADR 0004.

## Steering and README changes

- `tech.md`: replace the line "build no audio cache" with the decided rule (cache in `nemo-ytm`, plain files, caps). Add rows: `fs4` 1.1.0 (MIT OR Apache-2.0), `getrandom` 0.3.4 (MIT OR Apache-2.0, already in `Cargo.lock`), and `rusqlite` 0.41.1 now also in `nemo-ytm`.
- `structure.md`: `nemo-ytm` owns `offline/` and `offline.db`; `nemo-app` wires `OfflineStore`, `DownloadService` and `NetworkMode`; no new crate-to-crate edge.
- README: egress table gets no new host. Change the "When" of `www.youtube.com` and `*.googlevideo.com` to include "when songs are saved offline or checked". Add the offline-cache risk text (account restriction, deletion on takedown, sign-out wipe).
- ADR 0004: add `downloads.rs` and the `LyricsService::keep_offline` method to the expected `nemo-core` additions.
- `AppPaths` (`nemo-app/src/paths.rs`): add `offline`.

<!-- Sources:
.kiro/specs/offline-cache/requirements.md; docs/adr/0004-feature-wave-crates.md; .kiro/steering/coc.md, tech.md, structure.md; README.md (egress table)
crates/nemo-audio/src/{prepare,source,stream_decoder}.rs; crates/nemo-ytm/src/{stream,resolver,ytdlp,account,images,test_server}.rs; crates/nemo-core/src/{stream,segment,account,services}.rs; crates/nemo-lyrics/src/cache.rs; crates/nemo-app/src/paths.rs
https://crates.io/crates/fs4 1.1.0; https://crates.io/crates/getrandom 0.3.4; https://crates.io/crates/rusqlite
-->
