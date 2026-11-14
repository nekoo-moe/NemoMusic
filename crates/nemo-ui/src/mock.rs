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

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nemo_core::audio::Origin;
use nemo_core::catalog::{AlbumSummary, ArtistSummary, BrowseId, PlaylistSummary};
use nemo_core::integrations::NoIntegrations;
use nemo_core::notify::NoNotifier;
use nemo_core::services::Backend;
use nemo_core::stats::NoStats;
use nemo_core::track::{Artwork, Thumbnail, Track, TrackId, VideoId};

pub use crate::mock_lyrics::{MockLyrics, lyrics_for};

mod account;
mod catalog;
mod cover;
mod player;
#[cfg(test)]
mod tests;

pub use account::MockAccount;
pub use catalog::{MockCatalog, SeededCatalog, mock_backend_seeded, signal_path};
pub use cover::{MockImages, cover_png};
pub use player::MockPlayer;

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

/// Invented lyric lines; two are long, so wrapping shows in every snapshot.
pub(crate) const LYRIC_PHRASES: [&str; 16] = [
    "la la test line",
    "na na not a real lyric",
    "this line was made up",
    "và tâm hồn tôi là một bản nhạc chưa ai hát hết, dù đêm nay rất dài",
    "counting clocks that do not tick",
    "oh, the fake chorus starts",
    "we are only test data",
    "la la la, still synthetic",
    "pretend the music is loud",
    "nothing here is copyrighted",
    "hold the note for the demo while the long invented chorus keeps on going",
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
    // Two tracks carry Vietnamese text, so every snapshot and the live mock window exercise the
    // bundled font's Vietnamese glyphs (stacked diacritics, D with stroke).
    let (title, artists) = match n {
        1 => (
            "M\u{1edb}i Quen Nhau \u{110}\u{e3} R\u{1ee7}".to_owned(),
            vec!["HIEUTHUHAI".to_owned(), "Ph\u{1b0}\u{1a1}ng Ly".to_owned()],
        ),
        2 => (
            "\u{110}\u{ea}m M\u{1b0}a Nh\u{1edb} Ph\u{1ed1}".to_owned(),
            vec!["Lam Khu\u{ea}".to_owned()],
        ),
        _ => (title, vec![ARTIST_NAMES[artist].to_owned()]),
    };
    Track {
        id: TrackId(format!("mock-track-{n}")),
        title,
        artists,
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
        source_url: None,
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

/// A complete fake [`Backend`].
#[must_use]
pub fn mock_backend() -> Backend {
    Backend {
        catalog: Arc::new(MockCatalog),
        player: Arc::new(MockPlayer::new()),
        lyrics: Arc::new(MockLyrics::default()),
        images: Arc::new(MockImages::default()),
        account: Arc::new(MockAccount::new()),
        integrations: Arc::new(NoIntegrations),
        notifier: Arc::new(NoNotifier),
        soundcloud: None,
        stats: Arc::new(NoStats),
    }
}

/// Like [`mock_backend`], but already signed in.
#[must_use]
pub fn mock_backend_signed_in() -> Backend {
    Backend {
        account: Arc::new(MockAccount::signed_in()),
        ..mock_backend()
    }
}
