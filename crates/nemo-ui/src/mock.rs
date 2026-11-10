//! A fake backend for developing and testing the UI without a network, a player or a device.
//!
//! All names, lyrics and covers are invented here. Nothing touches the network or the disk.

#![allow(
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::manual_midpoint
)] // colour and easing maths: short names are the standard notation, casts are of small bounded values

use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use image::{ImageFormat, Rgb, RgbImage};
use nemo_core::BoxFuture;
use nemo_core::audio::{
    AudioFormat, Codec, Origin, OutputInfo, OutputMode, ProcessingStage, SignalPath, StreamInfo,
};
use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, BrowseId, Catalog, CatalogError,
    PlaylistDetail, PlaylistSummary, SearchResults, Shelf, ShelfItem,
};
use nemo_core::lyrics::{Granularity, LyricLine, LyricsDoc, LyricsQuery, ProviderId};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent, PlayerSnapshot};
use nemo_core::services::{
    Backend, EventReceiver, ImageError, ImageSource, LyricsLookup, LyricsService, PlayerControl,
};
use nemo_core::track::{Artwork, Thumbnail, Track, TrackId, VideoId, pick_thumbnail};

use crate::palette::{Oklch, oklch_to_rgb};

/// Number of generated tracks.
pub const TRACKS: usize = 40;
/// Tracks per album.
const PER_ALBUM: usize = 4;
/// Number of generated artists.
pub const ARTISTS: usize = 8;
/// Number of generated playlists.
pub const PLAYLISTS: usize = 6;
/// Scheme of mock cover addresses (`mock://cover/<n>`); there is no server behind it.
const COVER_PREFIX: &str = "mock://cover/";
/// Name of the mock lyrics provider.
pub const LYRICS_PROVIDER: &str = "Mock Lyrics";

const ARTIST_NAMES: [&str; ARTISTS] = [
    "Velvet Harbor",
    "Nova Lane",
    "The Quiet Orchard",
    "Mira Okonkwo",
    "Paper Satellites",
    "Juniper & Ash",
    "Kasumi Rowe",
    "Low Tide Club",
];

const ALBUM_TITLES: [&str; 10] = [
    "Afterglow Atlas",
    "Slow Weather",
    "Paper Moons",
    "Glass Garden",
    "Night Bus Home",
    "Small Hours",
    "Amber Static",
    "Field Notes",
    "Blue Hour Radio",
    "Soft Machines",
];

const TRACK_WORDS: [&str; 20] = [
    "Lanterns",
    "Harbor",
    "Echo",
    "Violet",
    "Drift",
    "Morning",
    "Signal",
    "Tangerine",
    "Midnight",
    "Compass",
    "Orchard",
    "Static",
    "Cinder",
    "Meadow",
    "Neon",
    "Paper",
    "Tidal",
    "Hollow",
    "Velvet",
    "Afterglow",
];

const PLAYLIST_TITLES: [&str; PLAYLISTS] = [
    "Focus Mix",
    "Late Night Drive",
    "Sunday Kitchen",
    "Rainy Window",
    "Warm-up",
    "Road Trip Echoes",
];

const LYRIC_PHRASES: [&str; 16] = [
    "la la test line",
    "na na not a real lyric",
    "this line was made up",
    "humming in the mock room",
    "counting clocks that do not tick",
    "oh, the fake chorus starts",
    "we are only test data",
    "la la la, still synthetic",
    "pretend the music is loud",
    "nothing here is copyrighted",
    "hold the note for the demo",
    "and the bridge goes nowhere",
    "da da dee, placeholder",
    "sing along to nothing",
    "the end is just a loop",
    "one more line for scrolling",
];

fn cover_artwork(n: usize) -> Artwork {
    Artwork::Remote(
        [(226_u32, 226_u32), (544, 544)]
            .into_iter()
            .map(|(width, height)| Thumbnail {
                url: format!("{COVER_PREFIX}{n}"),
                width,
                height,
            })
            .collect(),
    )
}

fn browse(kind: &str, n: usize) -> BrowseId {
    BrowseId(format!("mock-{kind}-{n}"))
}

/// The `n`-th generated track.
#[must_use]
pub fn track(n: usize) -> Track {
    let n = n % TRACKS;
    let album = n / PER_ALBUM;
    let artist = album % ARTISTS;
    let local = n % 5 == 3;
    let title = format!(
        "{} {}",
        TRACK_WORDS[n % TRACK_WORDS.len()],
        TRACK_WORDS[(n * 7 + 3) % TRACK_WORDS.len()]
    );
    Track {
        id: TrackId(format!("mock-track-{n}")),
        title,
        artists: vec![ARTIST_NAMES[artist].to_owned()],
        album: Some(ALBUM_TITLES[album].to_owned()),
        // 2:30 to 4:29; always known so the seek bar has a range.
        duration: Some(Duration::from_secs(150 + (n as u64 * 17) % 120)),
        origin: if local {
            Origin::Local
        } else {
            Origin::YoutubeMusic
        },
        video_id: (!local)
            .then(|| VideoId::parse(&format!("MockVid{n:04}")).ok())
            .flatten(),
        local_path: local.then(|| PathBuf::from(format!("mock/music/track-{n:02}.flac"))),
        artwork: cover_artwork(album),
    }
}

/// All generated tracks.
#[must_use]
pub fn tracks() -> Vec<Track> {
    (0..TRACKS).map(track).collect()
}

fn album_summary(a: usize) -> AlbumSummary {
    AlbumSummary {
        id: browse("album", a),
        title: ALBUM_TITLES[a % ALBUM_TITLES.len()].to_owned(),
        artists: vec![ARTIST_NAMES[a % ARTISTS].to_owned()],
        year: Some(2015 + u16::try_from(a).unwrap_or(0) * 2 % 10),
        thumbnails: thumbs(a),
    }
}

fn artist_summary(a: usize) -> ArtistSummary {
    ArtistSummary {
        id: browse("artist", a),
        name: ARTIST_NAMES[a % ARTISTS].to_owned(),
        // Offset the cover index so artist pictures differ from album covers.
        thumbnails: thumbs(a + 3),
    }
}

fn playlist_summary(p: usize) -> PlaylistSummary {
    PlaylistSummary {
        id: browse("playlist", p),
        title: PLAYLIST_TITLES[p % PLAYLISTS].to_owned(),
        author: Some("NemoMusic mock".to_owned()),
        thumbnails: thumbs(p + 5),
    }
}

/// Number of local albums in the mock library (their ids start with `local-album:`).
pub const LOCAL_ALBUMS: usize = 2;
const LOCAL_ALBUM_TITLES: [&str; LOCAL_ALBUMS] = ["Field Recordings", "Living Room Sessions"];

/// Local mock tracks (every fifth track), in order.
fn local_tracks() -> Vec<Track> {
    tracks()
        .into_iter()
        .filter(|t| t.origin == Origin::Local)
        .collect()
}

fn local_album_summary(a: usize) -> AlbumSummary {
    let first = local_tracks().get(a * PER_ALBUM).cloned();
    let artists = first
        .as_ref()
        .map(|t| t.artists.clone())
        .unwrap_or_default();
    let thumbnails = match first.map(|t| t.artwork) {
        Some(Artwork::Remote(t)) => t,
        _ => Vec::new(),
    };
    AlbumSummary {
        id: BrowseId(format!("{}{a}", crate::home::LOCAL_ALBUM_PREFIX)),
        title: LOCAL_ALBUM_TITLES[a % LOCAL_ALBUMS].to_owned(),
        artists,
        year: None,
        thumbnails,
    }
}

fn local_album_index(id: &BrowseId) -> Option<usize> {
    id.0.strip_prefix(crate::home::LOCAL_ALBUM_PREFIX)?
        .parse()
        .ok()
        .filter(|a| *a < LOCAL_ALBUMS)
}

fn thumbs(n: usize) -> Vec<Thumbnail> {
    match cover_artwork(n) {
        Artwork::Remote(t) => t,
        _ => Vec::new(),
    }
}

fn index_of(id: &BrowseId, kind: &str) -> Option<usize> {
    id.0.strip_prefix("mock-")?
        .strip_prefix(kind)?
        .strip_prefix('-')?
        .parse()
        .ok()
}

/// The fake catalog: 40 tracks, 10 albums, 8 artists, 6 playlists.
#[derive(Debug, Default, Clone, Copy)]
pub struct MockCatalog;

impl Catalog for MockCatalog {
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        let q = query.trim().to_lowercase();
        let hit = move |text: &str| q.is_empty() || text.to_lowercase().contains(&q);
        let songs = tracks()
            .into_iter()
            .filter(|t| hit(&t.title) || t.artists.iter().any(|a| hit(a)))
            .collect();
        let albums = (0..LOCAL_ALBUMS)
            .map(local_album_summary)
            .chain((0..TRACKS / PER_ALBUM).map(album_summary))
            .filter(|a| hit(&a.title) || a.artists.iter().any(|x| hit(x)))
            .collect();
        let artists = (0..ARTISTS)
            .map(artist_summary)
            .filter(|a| hit(&a.name))
            .collect();
        let playlists = (0..PLAYLISTS)
            .map(playlist_summary)
            .filter(|p| hit(&p.title))
            .collect();
        Box::pin(async move {
            Ok(SearchResults {
                songs,
                albums,
                artists,
                playlists,
            })
        })
    }

    fn album<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        let local = local_album_index(id).map(|a| AlbumDetail {
            summary: local_album_summary(a),
            tracks: local_tracks()
                .into_iter()
                .skip(a * PER_ALBUM)
                .take(PER_ALBUM)
                .collect(),
        });
        let result = local
            .or_else(|| {
                index_of(id, "album")
                    .filter(|a| *a < TRACKS / PER_ALBUM)
                    .map(|a| AlbumDetail {
                        summary: album_summary(a),
                        tracks: (a * PER_ALBUM..(a + 1) * PER_ALBUM).map(track).collect(),
                    })
            })
            .ok_or(CatalogError::NotFound);
        Box::pin(async move { result })
    }

    fn artist<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        let result = index_of(id, "artist")
            .filter(|a| *a < ARTISTS)
            .map(|a| {
                let albums: Vec<usize> = (0..TRACKS / PER_ALBUM)
                    .filter(|x| x % ARTISTS == a)
                    .collect();
                ArtistDetail {
                    summary: artist_summary(a),
                    top_songs: albums
                        .iter()
                        .flat_map(|al| (al * PER_ALBUM..(al + 1) * PER_ALBUM).map(track))
                        .take(5)
                        .collect(),
                    albums: albums.into_iter().map(album_summary).collect(),
                    description: Some("An invented artist for the mock catalog.".to_owned()),
                }
            })
            .ok_or(CatalogError::NotFound);
        Box::pin(async move { result })
    }

    fn playlist<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        let result = index_of(id, "playlist")
            .filter(|p| *p < PLAYLISTS)
            .map(|p| PlaylistDetail {
                summary: playlist_summary(p),
                tracks: (0..12).map(|i| track(p * 5 + i * 3)).collect(),
            })
            .ok_or(CatalogError::NotFound);
        Box::pin(async move { result })
    }

    fn home(&self) -> BoxFuture<'_, Result<Vec<Shelf>, CatalogError>> {
        Box::pin(async {
            Ok(vec![
                Shelf {
                    title: "Albums for you".to_owned(),
                    items: (0..TRACKS / PER_ALBUM)
                        .map(|a| ShelfItem::Album(album_summary(a)))
                        .collect(),
                },
                Shelf {
                    title: "Artists".to_owned(),
                    items: (0..ARTISTS)
                        .map(|a| ShelfItem::Artist(artist_summary(a)))
                        .collect(),
                },
                Shelf {
                    title: "Playlists".to_owned(),
                    items: (0..PLAYLISTS)
                        .map(|p| ShelfItem::Playlist(playlist_summary(p)))
                        .collect(),
                },
                Shelf {
                    title: "Songs".to_owned(),
                    items: (0..12).map(|n| ShelfItem::Song(track(n))).collect(),
                },
            ])
        })
    }
}

/// Signal path of a mock track: YouTube AAC 129 kbps resampled to a 48 kHz device in shared mode
/// (what the dev machine reports), or local FLAC 96 kHz / 24-bit to an exclusive device.
#[must_use]
pub fn signal_path(track: &Track) -> SignalPath {
    match track.origin {
        Origin::YoutubeMusic => {
            let out = AudioFormat {
                sample_rate: 48_000,
                channels: 2,
                bit_depth: Some(32),
            };
            SignalPath {
                source: StreamInfo {
                    origin: Origin::YoutubeMusic,
                    codec: Codec::Aac,
                    bitrate_kbps: Some(129),
                    format: Some(AudioFormat {
                        sample_rate: 44_100,
                        channels: 2,
                        bit_depth: None,
                    }),
                },
                processing: vec![ProcessingStage::Resample {
                    from_hz: 44_100,
                    to_hz: 48_000,
                }],
                output: OutputInfo {
                    device_name: "Speakers (Realtek Audio)".to_owned(),
                    mode: OutputMode::Shared,
                    format: out,
                },
            }
        }
        Origin::Local => {
            let format = AudioFormat {
                sample_rate: 96_000,
                channels: 2,
                bit_depth: Some(24),
            };
            SignalPath {
                source: StreamInfo {
                    origin: Origin::Local,
                    codec: Codec::Flac,
                    bitrate_kbps: None,
                    format: Some(format),
                },
                processing: Vec::new(),
                output: OutputInfo {
                    device_name: "Mock DAC".to_owned(),
                    mode: OutputMode::Exclusive,
                    format,
                },
            }
        }
    }
}

type Subscribers = Vec<Arc<Mutex<VecDeque<PlayerEvent>>>>;

struct PlayerState {
    snapshot: PlayerSnapshot,
    /// Position when the clock was last stopped or re-anchored.
    base: Duration,
    /// When the clock started running; `None` while not playing.
    since: Option<Instant>,
    subscribers: Subscribers,
}

/// A fake player: its clock advances while playing and every [`PlayerCommand`] changes its state
/// and emits the matching [`PlayerEvent`]s. Tracks end by themselves and move to the next one.
pub struct MockPlayer {
    state: Mutex<PlayerState>,
}

struct MockReceiver(Arc<Mutex<VecDeque<PlayerEvent>>>);

impl EventReceiver for MockReceiver {
    fn try_next(&mut self) -> Option<PlayerEvent> {
        self.0.lock().ok()?.pop_front()
    }
}

impl MockPlayer {
    /// A player with a 12-track queue, paused at the start of the first track.
    #[must_use]
    pub fn new() -> Self {
        let queue: Vec<Track> = (0..12).map(track).collect();
        let current = queue[0].clone();
        let snapshot = PlayerSnapshot {
            state: PlayState::Paused,
            duration: current.duration,
            signal_path: Some(signal_path(&current)),
            current: Some(current),
            queue,
            queue_index: Some(0),
            volume_percent: 80,
            ..PlayerSnapshot::default()
        };
        Self {
            state: Mutex::new(PlayerState {
                snapshot,
                base: Duration::ZERO,
                since: None,
                subscribers: Vec::new(),
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, PlayerState> {
        // A poisoned lock only means another test thread panicked; the data is still usable.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Default for MockPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerState {
    #[allow(clippy::needless_pass_by_value)] // events are built at the call site and moved in
    fn emit(&mut self, event: PlayerEvent) {
        self.subscribers.retain(|s| Arc::strong_count(s) > 1);
        for s in &self.subscribers {
            if let Ok(mut q) = s.lock() {
                q.push_back(event.clone());
            }
        }
    }

    fn position(&self) -> Duration {
        let run = self.since.map_or(Duration::ZERO, |t| t.elapsed());
        let pos = self.base + run;
        self.snapshot.duration.map_or(pos, |d| pos.min(d))
    }

    fn set_state(&mut self, state: PlayState) {
        if self.snapshot.state != state {
            self.snapshot.state = state;
            self.emit(PlayerEvent::StateChanged(state));
        }
    }

    /// Make queue entry `index` the current track and start it.
    fn start(&mut self, index: usize) {
        let Some(track) = self.snapshot.queue.get(index).cloned() else {
            return;
        };
        let path = signal_path(&track);
        self.snapshot.queue_index = Some(index);
        self.snapshot.duration = track.duration;
        self.snapshot.signal_path = Some(path.clone());
        let id = track.id.clone();
        self.snapshot.current = Some(track);
        self.base = Duration::ZERO;
        self.since = Some(Instant::now());
        self.emit(PlayerEvent::TrackChanged(id));
        if let Some(d) = self.snapshot.duration {
            self.emit(PlayerEvent::DurationKnown(d));
        }
        self.emit(PlayerEvent::SignalPathChanged(path));
        self.emit(PlayerEvent::QueueChanged);
        self.set_state(PlayState::Playing);
    }

    fn stop(&mut self) {
        self.snapshot.current = None;
        self.snapshot.queue_index = None;
        self.snapshot.duration = None;
        self.snapshot.signal_path = None;
        self.base = Duration::ZERO;
        self.since = None;
        self.set_state(PlayState::Stopped);
        self.emit(PlayerEvent::QueueChanged);
    }

    fn next(&mut self) {
        match self.snapshot.queue_index {
            Some(i) if i + 1 < self.snapshot.queue.len() => self.start(i + 1),
            _ => self.stop(),
        }
    }

    /// Move to the next track when the current one has run out.
    fn advance(&mut self) {
        if self.snapshot.state == PlayState::Playing
            && self.snapshot.duration.is_some_and(|d| self.position() >= d)
        {
            self.next();
        }
    }

    fn apply(&mut self, command: PlayerCommand) {
        match command {
            PlayerCommand::Play(track) => {
                self.snapshot.queue = vec![track];
                self.start(0);
            }
            PlayerCommand::SetQueue { tracks, start } => {
                self.snapshot.queue = tracks;
                self.start(start);
            }
            PlayerCommand::Enqueue(track) => {
                self.snapshot.queue.push(track);
                self.emit(PlayerEvent::QueueChanged);
            }
            PlayerCommand::PlayNext(track) => {
                let at = self.snapshot.queue_index.map_or(0, |i| i + 1);
                self.snapshot
                    .queue
                    .insert(at.min(self.snapshot.queue.len()), track);
                self.emit(PlayerEvent::QueueChanged);
            }
            PlayerCommand::RemoveAt(i) => self.remove_at(i),
            PlayerCommand::Pause => {
                if self.snapshot.state == PlayState::Playing {
                    self.base = self.position();
                    self.since = None;
                    self.set_state(PlayState::Paused);
                }
            }
            PlayerCommand::Resume => match self.snapshot.state {
                PlayState::Paused => {
                    self.since = Some(Instant::now());
                    self.set_state(PlayState::Playing);
                }
                PlayState::Stopped if !self.snapshot.queue.is_empty() => self.start(0),
                _ => {}
            },
            PlayerCommand::Stop => self.stop(),
            PlayerCommand::Seek(to) => {
                if self.snapshot.current.is_some() {
                    self.base = self.snapshot.duration.map_or(to, |d| to.min(d));
                    if self.since.is_some() {
                        self.since = Some(Instant::now());
                    }
                }
            }
            PlayerCommand::Next => self.next(),
            PlayerCommand::Previous => {
                let restart = self.position() > Duration::from_secs(3);
                match self.snapshot.queue_index {
                    Some(i) if !restart && i > 0 => self.start(i - 1),
                    Some(i) => self.start(i),
                    None => {}
                }
            }
            PlayerCommand::SetVolume(v) => {
                let v = v.min(100);
                self.snapshot.volume_percent = v;
                self.emit(PlayerEvent::VolumeChanged(v));
            }
            PlayerCommand::SetSkipSegments(on) => self.snapshot.skip_segments = on,
            PlayerCommand::UndoSkip => {}
        }
    }

    fn remove_at(&mut self, i: usize) {
        if i >= self.snapshot.queue.len() {
            return;
        }
        self.snapshot.queue.remove(i);
        match self.snapshot.queue_index {
            Some(cur) if cur == i => {
                if i < self.snapshot.queue.len() {
                    self.start(i);
                } else {
                    self.stop();
                }
            }
            Some(cur) if cur > i => {
                self.snapshot.queue_index = Some(cur - 1);
                self.emit(PlayerEvent::QueueChanged);
            }
            _ => self.emit(PlayerEvent::QueueChanged),
        }
    }
}

impl PlayerControl for MockPlayer {
    fn send(&self, command: PlayerCommand) {
        let mut s = self.lock();
        s.advance();
        s.apply(command);
    }

    fn position(&self) -> Duration {
        let mut s = self.lock();
        s.advance();
        s.position()
    }

    fn snapshot(&self) -> PlayerSnapshot {
        let mut s = self.lock();
        s.advance();
        s.snapshot.clone()
    }

    fn subscribe(&self) -> Box<dyn EventReceiver> {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        self.lock().subscribers.push(Arc::clone(&queue));
        Box::new(MockReceiver(queue))
    }
}

/// Fake lyrics: most tracks have line-synced lyrics, every 7th is unsynced and every 11th has
/// none. All text is invented.
#[derive(Default)]
pub struct MockLyrics {
    offsets: Mutex<HashMap<String, i64>>,
}

/// Index of a mock track from the lyrics query (matched by title and artist).
fn query_index(query: &LyricsQuery) -> Option<usize> {
    (0..TRACKS).find(|n| {
        let t = track(*n);
        t.title == query.title && t.artists == query.artists
    })
}

/// Synthetic lyrics for mock track `n`: one line every 4 s from 0:08.
#[must_use]
pub fn lyrics_for(n: usize, duration: Duration) -> Option<LyricsDoc> {
    if n % 11 == 5 {
        return None;
    }
    let unsynced = n % 7 == 3;
    let count = usize::try_from(duration.as_secs().saturating_sub(12) / 4).unwrap_or(0);
    let lines = (0..count)
        .map(|i| LyricLine {
            start_ms: (!unsynced).then(|| 8_000 + 4_000 * i as u64),
            end_ms: None,
            text: LYRIC_PHRASES[(i + n) % LYRIC_PHRASES.len()].to_owned(),
            words: Vec::new(),
        })
        .collect();
    Some(LyricsDoc {
        lines,
        granularity: if unsynced {
            Granularity::Unsynced
        } else {
            Granularity::Line
        },
        provider: ProviderId::new(LYRICS_PROVIDER),
        matched: None,
        instrumental: false,
    })
}

impl LyricsService for MockLyrics {
    fn lookup<'a>(&'a self, query: &'a LyricsQuery) -> BoxFuture<'a, LyricsLookup> {
        let doc = query_index(query).and_then(|n| {
            lyrics_for(
                n,
                query
                    .duration_ms
                    .map_or(Duration::from_secs(180), Duration::from_millis),
            )
        });
        Box::pin(async move {
            LyricsLookup {
                doc,
                tried: vec![
                    ProviderId::new(LYRICS_PROVIDER),
                    ProviderId::new("Mock Sidecar"),
                ],
            }
        })
    }

    fn offset_ms(&self, track_key: &str) -> i64 {
        self.offsets
            .lock()
            .ok()
            .and_then(|m| m.get(track_key).copied())
            .unwrap_or(0)
    }

    fn set_offset_ms(&self, track_key: &str, offset_ms: i64) {
        if let Ok(mut m) = self.offsets.lock() {
            m.insert(track_key.to_owned(), offset_ms);
        }
    }
}

/// Cover bytes by (cover index, size).
type CoverCache = HashMap<(usize, u32), Arc<Vec<u8>>>;

/// Procedurally generated covers: a diagonal two-hue gradient with a soft shape, as PNG.
#[derive(Default)]
pub struct MockImages {
    cache: Mutex<CoverCache>,
}

/// Render cover `n` at `px` x `px` as a PNG.
///
/// # Panics
/// Never in practice: encoding an in-memory RGB image to PNG cannot fail.
#[must_use]
pub fn cover_png(n: usize, px: u32) -> Vec<u8> {
    let px = px.clamp(16, 1024);
    // Hues spread by the golden angle so neighbours differ.
    let hue = (n as f64 * 137.5).rem_euclid(360.0);
    let hue2 = (hue + 50.0 + (n % 3) as f64 * 20.0).rem_euclid(360.0);
    let ink = |l: f64, c: f64, h: f64| {
        let rgb = oklch_to_rgb(Oklch { l, c, h });
        [rgb.r, rgb.g, rgb.b]
    };
    let (a, b, shape) = (
        ink(0.42, 0.13, hue),
        ink(0.72, 0.13, hue2),
        ink(0.90, 0.07, hue2),
    );
    let mut img = RgbImage::new(px, px);
    let size = px as f32;
    let centre = (0.3 + 0.1 * (n % 5) as f32, 0.7 - 0.08 * (n % 4) as f32);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let (u, v) = (x as f32 / size, y as f32 / size);
        let t = (0.5 * (u + v)).clamp(0.0, 1.0);
        let mut c = [0_u8; 3];
        for i in 0..3 {
            c[i] = (f32::from(a[i]) * (1.0 - t) + f32::from(b[i]) * t).round() as u8;
        }
        // A soft disc, different position per cover.
        let d = ((u - centre.0).powi(2) + (v - centre.1).powi(2)).sqrt();
        let m = (1.0 - ((d - 0.22) / 0.03).clamp(0.0, 1.0)) * 0.55;
        for i in 0..3 {
            c[i] = (f32::from(c[i]) * (1.0 - m) + f32::from(shape[i]) * m).round() as u8;
        }
        *pixel = Rgb(c);
    }
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .expect("invariant: encoding an in-memory RGB image cannot fail");
    out
}

impl ImageSource for MockImages {
    fn image<'a>(
        &'a self,
        artwork: &'a Artwork,
        target_px: u32,
    ) -> BoxFuture<'a, Result<Vec<u8>, ImageError>> {
        let result = match artwork {
            Artwork::Remote(thumbs) => pick_thumbnail(thumbs, target_px)
                .and_then(|t| t.url.strip_prefix(COVER_PREFIX))
                .and_then(|n| n.parse::<usize>().ok())
                .map(|n| {
                    // Never produce more pixels than the picked thumbnail has.
                    let px = target_px.clamp(64, 544);
                    let mut cache = self
                        .cache
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    Arc::clone(
                        cache
                            .entry((n, px))
                            .or_insert_with(|| Arc::new(cover_png(n, px))),
                    )
                })
                .map(|bytes| bytes.as_ref().clone())
                .ok_or(ImageError::Missing),
            Artwork::LocalFile(_) => Err(ImageError::Read("mock has no files".to_owned())),
            Artwork::None => Err(ImageError::Missing),
        };
        Box::pin(async move { result })
    }
}

/// A catalog that behaves like the real merged one: no home feed, an empty query lists only the
/// local library, and any other query answers with a slice of the mock catalog (a different slice
/// per query), so the seed shelves on Home fill up.
#[derive(Debug, Default, Clone, Copy)]
pub struct SeededCatalog;

impl Catalog for SeededCatalog {
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        let q = query.trim();
        let all = tracks();
        let result = if q.is_empty() {
            SearchResults {
                songs: all
                    .into_iter()
                    .filter(|t| t.origin == Origin::Local)
                    .collect(),
                albums: (0..LOCAL_ALBUMS).map(local_album_summary).collect(),
                ..SearchResults::default()
            }
        } else {
            let k = q.bytes().map(usize::from).sum::<usize>() % 4;
            SearchResults {
                songs: all.into_iter().skip(k * 7).take(8).collect(),
                albums: (0..TRACKS / PER_ALBUM)
                    .skip(k)
                    .take(5)
                    .map(album_summary)
                    .collect(),
                artists: (0..ARTISTS).skip(k).take(4).map(artist_summary).collect(),
                playlists: (0..PLAYLISTS)
                    .skip(k)
                    .take(3)
                    .map(playlist_summary)
                    .collect(),
            }
        };
        Box::pin(async move { Ok(result) })
    }

    fn album<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        MockCatalog.album(id)
    }

    fn artist<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        MockCatalog.artist(id)
    }

    fn playlist<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        MockCatalog.playlist(id)
    }
}

/// Like [`mock_backend`], but the catalog behaves like the real one (see [`SeededCatalog`]).
#[must_use]
pub fn mock_backend_seeded() -> Backend {
    Backend {
        catalog: Arc::new(SeededCatalog),
        ..mock_backend()
    }
}

/// A complete fake [`Backend`].
#[must_use]
pub fn mock_backend() -> Backend {
    Backend {
        catalog: Arc::new(MockCatalog),
        player: Arc::new(MockPlayer::new()),
        lyrics: Arc::new(MockLyrics::default()),
        images: Arc::new(MockImages::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    /// Run a future that is already ready (every mock future is).
    fn ready<T>(fut: impl Future<Output = T>) -> T {
        let mut fut = pin!(fut);
        let mut cx = Context::from_waker(Waker::noop());
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => v,
            Poll::Pending => unreachable!("mock futures complete at once"),
        }
    }

    fn drain(rx: &mut dyn EventReceiver) -> Vec<PlayerEvent> {
        std::iter::from_fn(|| rx.try_next()).collect()
    }

    #[test]
    fn catalog_has_forty_tracks_ten_albums_eight_artists() {
        let r = ready(MockCatalog.search("")).expect("ok");
        assert_eq!(r.songs.len(), 40);
        assert_eq!(r.albums.len(), 10 + LOCAL_ALBUMS);
        assert_eq!(r.artists.len(), 8);
        assert_eq!(r.playlists.len(), 6);
        let ids: std::collections::HashSet<_> = r.songs.iter().map(|t| t.id.0.clone()).collect();
        assert_eq!(ids.len(), 40, "unique ids");
    }

    #[test]
    fn search_filters_by_title_and_artist() {
        let r = ready(MockCatalog.search("velvet harbor")).expect("ok");
        assert!(!r.songs.is_empty() && r.songs.iter().all(|t| t.artists[0] == "Velvet Harbor"));
        assert_eq!(r.artists.len(), 1);
        assert_eq!(
            ready(MockCatalog.search("zzzz-nothing")).expect("ok").songs,
            vec![]
        );
    }

    #[test]
    fn details_resolve_and_unknown_ids_are_not_found() {
        let album = ready(MockCatalog.album(&browse("album", 2))).expect("album");
        assert_eq!(album.tracks.len(), 4);
        let artist = ready(MockCatalog.artist(&browse("artist", 1))).expect("artist");
        assert!(!artist.albums.is_empty() && !artist.top_songs.is_empty());
        assert_eq!(
            ready(MockCatalog.playlist(&browse("playlist", 0)))
                .expect("pl")
                .tracks
                .len(),
            12
        );
        assert_eq!(
            ready(MockCatalog.album(&BrowseId("nope".into()))),
            Err(CatalogError::NotFound)
        );
        assert_eq!(
            ready(MockCatalog.album(&browse("album", 99))),
            Err(CatalogError::NotFound)
        );
        assert!(ready(MockCatalog.home()).expect("home").len() >= 2);
    }

    #[test]
    fn covers_decode_and_differ() {
        let a = cover_png(0, 128);
        let b = cover_png(1, 128);
        assert_ne!(a, b);
        let img = image::load_from_memory(&a).expect("png");
        assert_eq!((img.width(), img.height()), (128, 128));
    }

    #[test]
    fn image_source_returns_png_and_reports_missing_art() {
        let images = MockImages::default();
        let t = track(5);
        let bytes = ready(images.image(&t.artwork, 200)).expect("cover");
        assert!(image::load_from_memory(&bytes).is_ok());
        assert_eq!(
            ready(images.image(&Artwork::None, 200)),
            Err(ImageError::Missing)
        );
        assert!(matches!(
            ready(images.image(&Artwork::LocalFile("x".into()), 200)),
            Err(ImageError::Read(_))
        ));
    }

    #[test]
    fn tracks_show_honest_signal_paths() {
        let yt = track(0);
        assert_eq!(yt.origin, Origin::YoutubeMusic);
        assert_eq!(signal_path(&yt).source.badge(), "AAC 129\u{2009}kbps");
        let local = track(3);
        assert_eq!(local.origin, Origin::Local);
        let p = signal_path(&local);
        assert_eq!(p.source.badge(), "FLAC 96\u{2009}kHz / 24-bit");
        assert!(p.is_bit_perfect());
        assert!(!signal_path(&yt).is_bit_perfect());
    }

    #[test]
    fn lyrics_are_synced_unsynced_or_missing() {
        let lyrics = MockLyrics::default();
        let q = |n| LyricsQuery::from_track(&track(n));
        let synced = ready(lyrics.lookup(&q(0))).doc.expect("synced");
        assert_eq!(synced.granularity, Granularity::Line);
        assert!(synced.lines.len() > 20 && synced.lines[0].start_ms == Some(8_000));
        let unsynced = ready(lyrics.lookup(&q(3))).doc.expect("unsynced");
        assert_eq!(unsynced.granularity, Granularity::Unsynced);
        assert!(unsynced.lines.iter().all(|l| l.start_ms.is_none()));
        let none = ready(lyrics.lookup(&q(5)));
        assert!(none.doc.is_none() && !none.tried.is_empty());
        assert_eq!(lyrics.offset_ms("k"), 0);
        lyrics.set_offset_ms("k", -300);
        assert_eq!(lyrics.offset_ms("k"), -300);
    }

    #[test]
    fn player_reacts_to_every_command() {
        let p = MockPlayer::new();
        assert_eq!(p.snapshot().state, PlayState::Paused);
        assert_eq!(p.position(), Duration::ZERO);

        p.send(PlayerCommand::Resume);
        assert_eq!(p.snapshot().state, PlayState::Playing);
        p.send(PlayerCommand::Pause);
        assert_eq!(p.snapshot().state, PlayState::Paused);

        p.send(PlayerCommand::Seek(Duration::from_secs(42)));
        assert_eq!(p.position(), Duration::from_secs(42));
        p.send(PlayerCommand::Seek(Duration::from_secs(99_999)));
        assert_eq!(
            Some(p.position()),
            p.snapshot().duration,
            "seek clamps to the end"
        );

        p.send(PlayerCommand::Next);
        assert_eq!(p.snapshot().queue_index, Some(1));
        assert_eq!(p.snapshot().state, PlayState::Playing);
        p.send(PlayerCommand::Previous);
        assert_eq!(
            p.snapshot().queue_index,
            Some(0),
            "early in a track, previous goes back"
        );
    }

    #[test]
    fn previous_goes_back_early_in_a_track_and_restarts_late() {
        let p = MockPlayer::new();
        p.send(PlayerCommand::Next);
        p.send(PlayerCommand::Pause);
        assert_eq!(p.snapshot().queue_index, Some(1));
        p.send(PlayerCommand::Previous);
        assert_eq!(p.snapshot().queue_index, Some(0));
        p.send(PlayerCommand::Seek(Duration::from_secs(30)));
        p.send(PlayerCommand::Previous);
        assert_eq!(p.snapshot().queue_index, Some(0));
        assert!(p.position() < Duration::from_secs(1));
    }

    #[test]
    fn queue_commands_volume_and_stop() {
        let p = MockPlayer::new();
        let before = p.snapshot().queue.len();
        p.send(PlayerCommand::Enqueue(track(30)));
        p.send(PlayerCommand::PlayNext(track(31)));
        let s = p.snapshot();
        assert_eq!(s.queue.len(), before + 2);
        assert_eq!(s.queue[1].id, track(31).id);
        p.send(PlayerCommand::RemoveAt(1));
        assert_eq!(p.snapshot().queue.len(), before + 1);
        p.send(PlayerCommand::SetVolume(250));
        assert_eq!(p.snapshot().volume_percent, 100);
        p.send(PlayerCommand::SetSkipSegments(true));
        assert!(p.snapshot().skip_segments);
        p.send(PlayerCommand::SetQueue {
            tracks: vec![track(3), track(4)],
            start: 1,
        });
        assert_eq!(
            p.snapshot().current.as_ref().map(|t| t.id.clone()),
            Some(track(4).id)
        );
        p.send(PlayerCommand::Play(track(3)));
        let s = p.snapshot();
        assert_eq!(s.queue.len(), 1);
        assert!(s.signal_path.expect("path").is_bit_perfect());
        p.send(PlayerCommand::Stop);
        let s = p.snapshot();
        assert_eq!(
            (s.state, s.current, s.queue_index),
            (PlayState::Stopped, None, None)
        );
        assert_eq!(p.position(), Duration::ZERO);
        p.send(PlayerCommand::UndoSkip);
        p.send(PlayerCommand::Resume);
        assert_eq!(
            p.snapshot().state,
            PlayState::Playing,
            "resume restarts a stopped queue"
        );
    }

    #[test]
    fn removing_the_current_track_plays_the_next_one() {
        let p = MockPlayer::new();
        p.send(PlayerCommand::RemoveAt(0));
        assert_eq!(p.snapshot().current.map(|t| t.id), Some(track(1).id));
        assert_eq!(p.snapshot().queue_index, Some(0));
    }

    #[test]
    fn clock_advances_only_while_playing() {
        let p = MockPlayer::new();
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(p.position(), Duration::ZERO, "paused clock stands still");
        p.send(PlayerCommand::Resume);
        std::thread::sleep(Duration::from_millis(60));
        let a = p.position();
        assert!(a >= Duration::from_millis(50), "{a:?}");
        p.send(PlayerCommand::Pause);
        let frozen = p.position();
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(p.position(), frozen);
    }

    #[test]
    fn a_finished_track_moves_to_the_next() {
        let p = MockPlayer::new();
        p.send(PlayerCommand::Resume);
        let d = p.snapshot().duration.expect("duration");
        p.send(PlayerCommand::Seek(
            d.saturating_sub(Duration::from_millis(20)),
        ));
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(p.snapshot().queue_index, Some(1));
    }

    #[test]
    fn events_follow_commands_and_each_receiver_gets_its_own_copy() {
        let p = MockPlayer::new();
        let mut a = p.subscribe();
        let mut b = p.subscribe();
        p.send(PlayerCommand::Resume);
        p.send(PlayerCommand::SetVolume(30));
        let ea = drain(a.as_mut());
        assert!(ea.contains(&PlayerEvent::StateChanged(PlayState::Playing)));
        assert!(ea.contains(&PlayerEvent::VolumeChanged(30)));
        assert_eq!(ea, drain(b.as_mut()));
        assert!(drain(a.as_mut()).is_empty(), "drained");
        p.send(PlayerCommand::Next);
        let e = drain(a.as_mut());
        assert!(e.iter().any(|x| matches!(x, PlayerEvent::TrackChanged(_))));
        assert!(
            e.iter()
                .any(|x| matches!(x, PlayerEvent::SignalPathChanged(_)))
        );
        assert!(e.iter().any(|x| matches!(x, PlayerEvent::DurationKnown(_))));
    }

    #[test]
    fn mock_backend_is_wired() {
        let b = mock_backend();
        assert_eq!(b.player.snapshot().queue.len(), 12);
        assert!(ready(b.catalog.search("")).is_ok());
    }
}
