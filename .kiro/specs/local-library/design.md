## Overview

Local files already play: `nemo-library` scans folders in memory and `nemo-audio` opens a track with a `local_path` through `FileSource` and `StreamDecoder` (`prepare.rs`). This design adds what is missing: a SQLite library that survives restarts, ALAC, Vorbis, AIFF and Opus decoding, CUE tracks, local playlists with M3U, tag editing, ReplayGain, a folder dialog, and open-with and drag-and-drop.

Decisions in one place:
- Storage: `rusqlite` =0.41.1 (bundled SQLite, WAL), the version `nemo-lyrics` already builds. One file, `library.db`, in `AppPaths.library`.
- Formats: `symphonia` features `alac`, `vorbis`, `ogg`, `aiff` (all MPL-2.0, 0.6.1). Opus through a decoder registered with `symphonia` (pure-Rust `opus-decoder` first, `symphonia-adapter-libopus` =0.3.0 as the fallback; see Risks), so `StreamDecoder` stays one code path. The same decoder can play YouTube's Opus streams. Adding `Codec::Opus` to `supported_stream_codecs()` (`decoder.rs`), which makes the YouTube and SoundCloud rankers pick Opus, is a separate task after the decoder lands; the owner approved Opus playback for streams on 2026-11-14 (account-actions: Opus where it sounds better). `Codec::Opus` already exists in `audio.rs`.
- Folder dialog: `rfd` =0.18.0 (MIT) in `nemo-ui`. Typed path stays as the fallback.
- Watching: none. The library loads from SQLite at start, then rescans in the background, and rescans on demand. `notify` is CC0-1.0 (not on the `code.md` allowlist) and its 9.0 line is a release candidate; no criterion needs live watching.
- Gapless marks: `nemo-library` implements `GaplessPolicy` (defined by audio-effects, `gapless.rs`). `join_without_fade(from, to)` is true for consecutive tracks of one CUE file, and for two consecutive tracks of one album that has the `gapless` flag (a column on the album row, set from the iTunes `pgap` tag or by the user).
- ReplayGain: tags are read at scan time. The opt-in scan for untagged files measures loudness with `ebur128` =0.1.11 (MIT, pure Rust) and stores the result in the library only.
- CUE tracks: a hand-written parser (`rcue` is MIT but unreleased since 2022; `cue` is GPL-2.0-or-later). A CUE track is a time span of one audio file.
- Linux: the lossless badge stays off because `SignalPath::is_bit_perfect` needs `OutputMode::Exclusive`, which `cpal` on ALSA never reports. No code change is needed for this.

## Affected Crates

- `nemo-core`: new `library.rs` (types and traits below). Edits to `track.rs`: none; per-track extras (disc, year, genre, album artist, CUE span, gain) live in `LocalTrackDetails`, not in `Track`.
- `nemo-audio`: `Cargo.toml` features; `decoder.rs` `codec_from_id` maps ALAC, Vorbis, Opus, AIFF; new `codecs.rs` builds the registry with the Opus adapter; `prepare.rs` asks `LocalTrackLookup` for span and gain; `feeder.rs` seeks to `span.start` and ends the job at `span.end`; new `loudness.rs` implements `LoudnessMeter`; `source.rs` `FileSource` is unchanged and is also the reader for cached files (offline-cache design).
- `nemo-library`: new `store.rs` (migrations, queries), `scan.rs` (incremental), `cue.rs`, `m3u.rs`, `playlists.rs`, `tag_edit.rs`, `replaygain.rs`; `library.rs` and `catalog.rs` read from the store snapshot instead of a rescan.
- `nemo-ui`: Settings folder list with the `rfd` dialog, library and playlist screens, drop target, tag editor, ReplayGain settings. Reduced motion applies to the scan list.
- `nemo-app`: builds `LocalLibraryService`, passes `LocalTrackLookup` into `PrepareDeps`, passes the `LoudnessMeter` into the library scan job, handles file-open arguments and the single-instance hand-off.
- `nemo-ytm`, `nemo-lyrics`: no change.

## Data Flow

- Start: `nemo-app` opens `library.db` on a worker thread, loads every row into an `Arc<RwLock<Snapshot>>` that `LocalCatalog` reads, then queues a rescan. Budget: shown within 1 s for 50 000 tracks (`MAX_FILES`); unmeasured, so a test checks it.
- Scan: one scan thread walks folders (existing `scan_folder`, no symlinks, depth 16) and sends `ScanEvent` values over an `mpsc` channel to one writer thread. The writer owns the only write connection, batches inserts in transactions, and swaps new rows into the snapshot. A file is re-read only when its size or modified time differ from the row. Missing files are removed after the walk ends.
- Playback: the control thread starts `spawn_prepare`. The prepare thread calls `LocalTrackLookup::playback(&Track)` (a read on its own SQLite connection, never on the callback), opens `FileSource`, probes with `StreamDecoder`, and returns `Prepared` with the span and gain. The feeder thread owns decoded blocks and the ring producer; the `cpal` callback only reads the ring. Nothing new runs in the callback.
- CUE gapless: tracks N and N+1 of one file are two jobs on two decoders. Job N ends at frame `E`; job N+1 seeks to the same frame `E` with `SeekMode::Accurate` (the existing `skip_frames` logic) and starts there. The feeder counts frames, so the join is sample-exact for FLAC, ALAC and WAV. For lossy files the cut is exact in decoded frames; codec delay is handled by the container's gapless data (criterion 11).
- ReplayGain: the feeder applies one scalar per sample block before the ring, only while the stage exists. Off means the stage is not built, so there is no arithmetic. The stage is registered as `ProcessingStage::ReplayGain { db }` (defined by audio-effects, which also fixes the order: after the song's own `Chain`, before the transition mixer; `Gain` stays the volume stage). This design defines only the values, the mode and the peak rule: the scalar is lowered so peak x gain stays at or below 1.0.
- Loudness scan: a low-priority thread takes one untagged file at a time, decodes it with `StreamDecoder` inside `nemo-audio`, feeds `ebur128`, and returns `Loudness`. The library writes it to table `loudness`. A cancel flag stops it between blocks.
- Open and drop: a second launch with a path sends it to the running app over a Windows named pipe (`\\.\pipe\NemoMusic-<user SID>`, limited to the current user) or a Unix socket in `$XDG_RUNTIME_DIR` (mode 0600), then exits. No token and no file is written (owner, 2026-11-14). Dropped paths arrive as UI messages. Neither adds to the library unless the file lies under a watched folder.
- Sign-out: no wipe step; the library holds no account data (criterion 29).

## Interfaces

```rust
// nemo-core/src/library.rs (new)
pub struct TrackSpan { pub start: Duration, pub end: Option<Duration> }
pub enum ReplayGainMode { Track, Album }
pub struct ReplayGainValues {
    pub track_gain_db: Option<f32>, pub album_gain_db: Option<f32>,
    pub track_peak: Option<f32>, pub album_peak: Option<f32>,
}
pub struct LocalPlayback { pub span: Option<TrackSpan>, pub gain: ReplayGainValues }
pub struct LocalTrackDetails {
    pub track: Track, pub album_artist: Option<String>, pub disc: Option<u32>, pub number: Option<u32>,
    pub year: Option<u32>, pub genre: Option<String>, pub status: TrackStatus,
}
pub enum TrackStatus { Playable, Unplayable(UnplayableReason), Missing }
pub enum UnplayableReason { UnsupportedFormat, DecodeFailed, Unreadable }

/// Read on the prepare thread. May block on SQLite; never called from the audio callback.
pub trait LocalTrackLookup: Send + Sync {
    fn playback(&self, track: &Track) -> Option<LocalPlayback>;
}

pub struct Loudness { pub integrated_lufs: f32, pub true_peak: f32 }
pub trait LoudnessMeter: Send + Sync {
    fn measure(&self, path: &Path, cancel: &AtomicBool) -> Result<Loudness, LoudnessError>;
}

pub struct LocalPlaylistId(pub i64);
/// `path` is set for a local file. A streamed song (saved by playlist-import or a stats recap) has `path: None` and its `track`.
pub struct PlaylistEntry { pub path: Option<PathBuf>, pub track: Option<Track>, pub missing: bool }
pub struct LocalPlaylist { pub id: LocalPlaylistId, pub name: String, pub entries: Vec<PlaylistEntry> }
pub struct M3uImport { pub playlist: LocalPlaylistId, pub added: usize, pub skipped: usize }
pub struct TagEdit { pub title: Option<String>, pub artists: Option<Vec<String>>, pub album: Option<String>,
    pub album_artist: Option<String>, pub number: Option<u32>, pub disc: Option<u32>,
    pub year: Option<u32>, pub genre: Option<String> }
pub struct TagDiff { pub field: &'static str, pub old: String, pub new: String }

pub trait LocalLibraryService: Send + Sync {
    fn folders(&self) -> Vec<PathBuf>;
    fn add_folder(&self, path: PathBuf) -> BoxFuture<'_, Result<(), LibraryError>>;
    fn remove_folder(&self, path: &Path) -> BoxFuture<'_, Result<(), LibraryError>>;
    fn rescan(&self) -> BoxFuture<'_, Result<ScanSummary, LibraryError>>;
    fn subscribe_scan(&self) -> Box<dyn ScanReceiver>;
    fn details(&self, id: &TrackId) -> Option<LocalTrackDetails>;
    fn playlists(&self) -> Vec<LocalPlaylist>;
    fn create_playlist(&self, name: &str) -> BoxFuture<'_, Result<LocalPlaylistId, LibraryError>>;
    /// Used by playlist-import and listening-stats recaps. Returns how many were added.
    fn add_tracks(&self, id: LocalPlaylistId, tracks: Vec<Track>) -> BoxFuture<'_, Result<usize, LibraryError>>;
    fn import_m3u(&self, file: &Path) -> BoxFuture<'_, Result<M3uImport, LibraryError>>;
    fn export_m3u8(&self, id: LocalPlaylistId, to: &Path) -> BoxFuture<'_, Result<(), LibraryError>>;
    fn preview_tag_edit(&self, id: &TrackId, edit: &TagEdit) -> BoxFuture<'_, Result<Vec<TagDiff>, LibraryError>>;
    fn apply_tag_edit(&self, id: &TrackId, edit: TagEdit) -> BoxFuture<'_, Result<(), LibraryError>>;
    fn set_loudness_scan(&self, on: bool);
}
// Rename, reorder, delete playlist and add or remove entries follow the same shape.

// nemo-audio
pub struct AudioMeter;                      // impl LoudnessMeter
pub(crate) fn build_codecs() -> CodecRegistry;  // default decoders plus the Opus adapter
```

`PrepareDeps` gains `local: Option<Arc<dyn LocalTrackLookup>>`. New Cargo lines: `symphonia` features `alac`, `vorbis`, `ogg`, `aiff` added to the current seven; `symphonia-adapter-libopus = "=0.3.0"`; `ebur128 = "=0.1.11"`; in `nemo-ui`, `rfd = "=0.18.0"`.

## Error Handling

- `LibraryError` (`thiserror`): `Db`, `Io`, `Tag`, `Cue`, `M3u`, `Corrupt`, `Busy`. No `unwrap`. Messages never carry paths or tags at info level or above (criterion 28).
- Corrupt file: `PRAGMA quick_check` at open. On failure the file and its `-wal` and `-shm` are renamed to `library.db.broken-<unix time>`, a new file is created, a rescan runs, and one `ScanEvent::Rebuilt` tells the user. Playlists live in the same file, so the kept copy is how the user gets them back; the message says so.
- A file that cannot be decoded is stored as `Unplayable` with a reason; the reason text shows on request. Playlist entries to missing files stay and the player skips them.
- Tag write: copy to a temp file beside the original, write the tag with `lofty`, re-read it, then replace the original. Any failure removes the temp file and leaves the original unchanged (criterion 24). A read-only file or a file another program holds ends as `LibraryError::Tag` with a plain reason.
- M3U: unresolved lines are counted in `skipped`, not raised. Entries without a path (streamed songs) are not written to M3U8; the export reports how many were left out. CUE with a missing audio file or bad index lists the audio file as one track and records the problem.
- Opus adapter or ReplayGain stage failing to build: the track plays at unity gain; the error is logged without paths.

## Testing Strategy

- `cargo test -p nemo-audio --test bit_exact`: must pass with ReplayGain off. New fixtures: ALAC and AIFF files made from the existing WAV fixture decode to identical samples. Vorbis and Opus compare to a reference decode within a stated tolerance, not bit-exact.
- Counting-allocator test `cargo test -p nemo-audio --test rt_no_alloc`: add a local-file run and a run with the gain stage on; the callback allocates nothing in both.
- Gapless CUE: a FLAC fixture with a 3-track CUE; the concatenated output of the three tracks equals the output of the whole file, frame for frame.
- `nemo-library`: CUE parser with `proptest` (UTF-8 BOM, Windows-1252 fallback, `INDEX 00` and `01`); M3U8 relative and absolute paths, skipped lines; SQLite migration, reopen, WAL, and a truncated file that triggers the rebuild; incremental scan with a temp folder; 50 000-row load time against the 1 s budget; tag edit on a read-only file leaves the bytes identical.
- Loudness: a calibrated tone fixture gives the expected integrated loudness within 0.1 LU (fixture source unverified).
- Badge: unit test that `is_bit_perfect` is false for `OutputMode::Shared`, which is all `cpal` reports on Linux.
- Manual: Windows and Linux Mint, open-with, drop, second launch hand-off, folder dialog without a desktop portal.

## Risks and Alternatives

- Opus build (owner, 2026-11-14): the first Opus task tests the pure-Rust `opus-decoder` 0.1.1 (MIT OR Apache-2.0, March 2026, one maintainer) against the RFC 8251 test vectors. If it passes, it is used and no C toolchain is needed. If it fails, `symphonia-adapter-libopus` =0.3.0 is used; it builds libopus through `opusic-sys` 0.7.7 (BSD-3-Clause) and `cmake`, so builds then need CMake and a C compiler, which the owner accepted. Both sit behind one function, so either can be swapped. `audiopus` (named in `tech.md`) is rejected: last release 2021.
- Opus in Ogg depends on `symphonia-format-ogg` 0.6.1 mapping Opus streams. The spike below confirmed it.
- Opus spike result (local-library#4, 2026-11-14, Windows x64, Rust 1.97.1, release build): `opus-decoder` 0.1.1 **fails (a)**, so task 6 uses the fallback `symphonia-adapter-libopus` =0.3.0.
  - (a) Conformance. The RFC 8251 vectors (`opus_testvectors-rfc8251.tar.gz` from opus-codec.org, SHA-256 `6b26a22f9ba87b2b836906a9bb7afec5f8e54d49553b1200382520ee6fedfa55`) were decoded at 48 kHz in mono and stereo, as libopus `tests/run_vectors.sh` does, and checked with `opus_compare` built from opus 1.6.1 (`opus-1.6.1.tar.gz`, SHA-256 matches xiph.org `SHA256SUMS.txt`). Stereo output passes all 12 vectors, and the range-coder final state matches every packet. Mono output fails 6 of 12 (vectors 02, 03, 04, 05, 06 and 10) against both reference sets; vectors 02 to 04 are plain mono SILK streams, so a mono Opus file would decode wrong. libopus 1.6.1 `opus_demo` passes all 24 runs with the same harness. The crate's "all 12 pass" claim covers stereo output only.
  - Speed. `opus-decoder` decodes CELT and hybrid audio at about 2 to 4 times real time in a release build (one 160 kbps stereo minute: 2.1x, about half a core). libopus decodes the same file at about 330x. The cause is a direct O(N^2) DFT in its `celt/kiss_fft.rs`. Its `decode_float` also rounds through `i16`.
  - (b) Containers. `symphonia-format-ogg` 0.6.1 demuxes Ogg Opus: it states the pre-skip (`Track::delay`, and the OpusHead in `extra_data`) and trims the end padding from the granule position, but does not trim the pre-skip; the decoder must. The `mkv` reader demuxes WebM Opus and passes the OpusHead in `extra_data`, but ignores `DiscardPadding` (648 extra frames, 13.5 ms, at the end of a 3 s ffmpeg file) and only shifts the timestamps by `CodecDelay` at millisecond precision. With a wrapper that drops the OpusHead pre-skip, both files gave the exact length (Ogg) and 89.4 dB SNR against an ffmpeg libopus decode (both). `symphonia-adapter-libopus` 0.3.0 drops the OpusHead pre-skip itself (`lib.rs`, read, not run).
  - (c) Build and licence. `opus-decoder` 0.1.1 builds on 1.97.1 (`rust-version` 1.85), MIT OR Apache-2.0, `#![forbid(unsafe_code)]`, `cargo deny check licenses bans sources` passes. Not the deciding point.
  - For task 6: CMake 4.3.1 ships with Visual Studio Build Tools 18 on the build PC (`Common7\IDE\CommonExtensions\Microsoft\CMake`) and built opus 1.6.1 there. WebM end padding stays untrimmed unless task 6 reads `DiscardPadding` itself; streams are not joined gaplessly, so this is recorded, not fixed.
- Tag writes replace the file, which changes its file id and may drop hard links or custom permissions. In-place `lofty` save is simpler but can corrupt the file on a crash. I chose replace.
- The single-instance hand-off uses a named pipe and a Unix socket, so it needs a little platform code (or a small crate, picked in the task; licence and `rust-version` checked as usual). Only the same user can open either, and no token exists.
- `rusqlite` 0.42.0 is out (2026-11-11). Staying on 0.41.1 keeps one `libsqlite3-sys` with `nemo-lyrics`; both move together later.
- Departure from ADR 0004: it lists "folder watching" under `nemo-library`; this design does not build it. No other departure.
- The ReplayGain scan costs CPU on large libraries. It is off by default, one file at a time, and cancellable.

## Steering and README changes

- `tech.md`: Decode row: add features `alac`, `vorbis`, `ogg`, `aiff`. Opus decode row: `symphonia-adapter-libopus` =0.3.0, MIT OR Apache-2.0, plus bundled libopus (via `opusic-sys` 0.7.7, BSD-3-Clause); needs CMake; replaces the `opus` or `audiopus` wording. Library DB row: `rusqlite` 0.41.1 from "proposed" to "decided". New rows: `rfd` 0.18.0 (MIT, Windows and Linux portal), `ebur128` 0.1.11 (MIT), `getrandom` 0.3.4 (MIT OR Apache-2.0, already locked) is not needed for the hand-off any more. Record `notify` as rejected (CC0-1.0, rc).
- `structure.md`: `nemo-library` owns `library.db`; `nemo-audio` implements `LoudnessMeter` and `nemo-app` wires it; no new crate-to-crate edge.
- README: no new network host. Add libopus (BSD-3-Clause) to the third-party licence notice if one exists. Note that the folder dialog on Linux uses the desktop portal.
- ADR 0004: add `library.rs` to the expected `nemo-core` additions; record that folder watching was not built.
- `deny.toml`: no exception needed for any crate named here.

<!-- Sources:
.kiro/specs/local-library/requirements.md; docs/adr/0004-feature-wave-crates.md; .kiro/steering/tech.md, structure.md
crates/nemo-audio/src/{prepare,source,stream_decoder,decoder,feeder,chain}.rs; crates/nemo-library/src/{library,scan,tags,catalog}.rs; crates/nemo-core/src/{audio,services,track}.rs; crates/nemo-lyrics/src/cache.rs
https://crates.io/api/v1/crates/symphonia/0.6.1 (features alac, vorbis, ogg, aiff; no opus feature; MPL-2.0; MSRV 1.85)
https://crates.io/crates/symphonia-adapter-libopus 0.3.0 (MIT OR Apache-2.0, MSRV 1.89, 2026-05-16); https://crates.io/crates/opusic-sys 0.7.7 (BSD-3-Clause, cmake build dependency)
https://crates.io/crates/opus-decoder 0.1.1; https://crates.io/crates/rfd 0.18.0 (MIT, MSRV 1.92); https://crates.io/crates/notify (8.2.2 CC0-1.0; 9.0.0-rc.8)
https://crates.io/crates/ebur128 0.1.11; https://crates.io/crates/rusqlite 0.42.0; https://github.com/pdeljanov/Symphonia README
-->
