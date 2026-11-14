//! Pure mapping from the raw structs to `nemo-core` types.

use std::time::Duration;

use nemo_core::audio::Origin;
use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, BrowseId, PlaylistDetail,
    PlaylistSummary, SearchResults,
};
use nemo_core::track::{Artwork, Thumbnail, Track, TrackId, VideoId};

use super::{
    MAX_ID_BYTES, RawAlbum, RawAlbumPage, RawArtist, RawArtistPage, RawPlaylist, RawPlaylistPage,
    RawSong, adapt,
};

// ---------------------------------------------------------------------------------------------
// Pure mapping
// ---------------------------------------------------------------------------------------------

fn clean(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn clean_all(texts: &[String]) -> Vec<String> {
    texts.iter().filter_map(|t| clean(t)).collect()
}

/// `m:ss`, `mm:ss` or `h:mm:ss` to a duration. Anything else (also `0:00`) gives `None`.
pub(crate) fn parse_duration(text: &str) -> Option<Duration> {
    let mut seconds: u64 = 0;
    let mut parts = 0_u32;
    for part in text.trim().split(':') {
        if part.is_empty() || part.len() > 2 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        seconds = seconds * 60 + part.parse::<u64>().ok()?;
        parts += 1;
    }
    ((2..=3).contains(&parts) && seconds > 0).then(|| Duration::from_secs(seconds))
}

/// The first run of four digits in `text` as a year, for example `"2019"` or `"Album • 2019"`.
pub(crate) fn parse_year(text: &str) -> Option<u16> {
    let bytes = text.as_bytes();
    bytes
        .windows(4)
        .enumerate()
        .find(|(index, window)| {
            window.iter().all(u8::is_ascii_digit)
                && !bytes
                    .get(index.wrapping_sub(1))
                    .is_some_and(u8::is_ascii_digit)
                && !bytes.get(index + 4).is_some_and(u8::is_ascii_digit)
        })
        .and_then(|(index, _)| text[index..index + 4].parse().ok())
}

/// Keep thumbnails with a non-empty URL; sizes that do not fit `u32` become `0` (unknown).
pub(crate) fn map_thumbnails(raw: &[(String, u64, u64)]) -> Vec<Thumbnail> {
    raw.iter()
        .filter(|(url, _, _)| !url.trim().is_empty())
        .map(|(url, width, height)| Thumbnail {
            url: url.trim().to_owned(),
            width: u32::try_from(*width).unwrap_or(0),
            height: u32::try_from(*height).unwrap_or(0),
        })
        .collect()
}

fn artwork(thumbnails: &[Thumbnail]) -> Artwork {
    if thumbnails.is_empty() {
        Artwork::None
    } else {
        Artwork::Remote(thumbnails.to_vec())
    }
}

fn build_track(
    video_id: &str,
    title: &str,
    artists: Vec<String>,
    album: Option<String>,
    duration: Option<&str>,
    thumbnails: &[Thumbnail],
) -> Option<Track> {
    let video = VideoId::parse(video_id).ok()?;
    let title = clean(title)?;
    Some(Track {
        id: TrackId(video.as_str().to_owned()),
        title,
        artists,
        album,
        duration: duration.and_then(parse_duration),
        origin: Origin::YoutubeMusic,
        video_id: Some(video),
        local_path: None,
        artwork: artwork(thumbnails),
    })
}

/// A song, or `None` when the id or the title is unusable.
pub(crate) fn map_song(raw: &RawSong) -> Option<Track> {
    build_track(
        &raw.video_id,
        &raw.title,
        clean_all(&raw.artists),
        raw.album.as_deref().and_then(clean),
        raw.duration.as_deref(),
        &raw.thumbnails,
    )
}

fn map_songs(raw: &[RawSong]) -> Vec<Track> {
    raw.iter().filter_map(map_song).collect()
}

/// An album summary, or `None` without an id or a title.
pub(crate) fn map_album(raw: &RawAlbum) -> Option<AlbumSummary> {
    let id = clean(&raw.id)?;
    Some(AlbumSummary {
        id: BrowseId(id),
        title: clean(&raw.title)?,
        artists: clean_all(&raw.artists),
        year: raw.year.as_deref().and_then(parse_year),
        thumbnails: raw.thumbnails.clone(),
    })
}

/// An artist summary, or `None` without an id or a name.
pub(crate) fn map_artist(raw: &RawArtist) -> Option<ArtistSummary> {
    Some(ArtistSummary {
        id: BrowseId(clean(&raw.id)?),
        name: clean(&raw.name)?,
        thumbnails: raw.thumbnails.clone(),
    })
}

/// A playlist summary, or `None` without an id or a title.
pub(crate) fn map_playlist(raw: &RawPlaylist) -> Option<PlaylistSummary> {
    Some(PlaylistSummary {
        id: BrowseId(clean(&raw.id)?),
        title: clean(&raw.title)?,
        author: raw.author.as_deref().and_then(clean),
        thumbnails: raw.thumbnails.clone(),
    })
}

/// Album page: the tracks get the album's artists, title and cover.
pub(crate) fn map_album_page(raw: &RawAlbumPage, requested: &BrowseId) -> Option<AlbumDetail> {
    let mut summary = map_album(&raw.album)?;
    summary.id = requested.clone();
    let artists = summary.artists.clone();
    let tracks = raw
        .tracks
        .iter()
        .filter_map(|t| {
            build_track(
                &t.video_id,
                &t.title,
                artists.clone(),
                Some(summary.title.clone()),
                t.duration.as_deref(),
                &raw.album.thumbnails,
            )
        })
        .collect();
    Some(AlbumDetail { summary, tracks })
}

/// Artist page.
pub(crate) fn map_artist_page(raw: &RawArtistPage, requested: &BrowseId) -> Option<ArtistDetail> {
    let mut summary = map_artist(&raw.artist)?;
    summary.id = requested.clone();
    Some(ArtistDetail {
        summary,
        top_songs: map_songs(&raw.songs),
        albums: raw.albums.iter().filter_map(map_album).collect(),
        description: raw.description.as_deref().and_then(clean),
    })
}

/// Playlist page.
pub(crate) fn map_playlist_page(
    raw: &RawPlaylistPage,
    requested: &BrowseId,
) -> Option<PlaylistDetail> {
    let mut summary = map_playlist(&raw.playlist)?;
    summary.id = requested.clone();
    Some(PlaylistDetail {
        summary,
        tracks: map_songs(&raw.songs),
        edit: None,
    })
}

/// Browse and playlist ids come from the network. Accept only short ids made of letters, digits,
/// `-` and `_`, so nothing odd reaches a request body.
pub(crate) fn valid_browse_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The four filtered search answers, ready to map.
pub(crate) struct SearchParts {
    pub(crate) songs: Vec<ytmapi_rs::parse::SearchResultSong>,
    pub(crate) albums: Vec<ytmapi_rs::parse::SearchResultAlbum>,
    pub(crate) artists: Vec<ytmapi_rs::parse::SearchResultArtist>,
    pub(crate) playlists: Vec<ytmapi_rs::parse::SearchResultPlaylist>,
}

/// Turn the search answers into core types.
pub(crate) fn map_search(parts: &SearchParts) -> SearchResults {
    SearchResults {
        songs: parts
            .songs
            .iter()
            .map(adapt::search_song)
            .filter_map(|s| map_song(&s))
            .collect(),
        albums: parts
            .albums
            .iter()
            .map(adapt::search_album)
            .filter_map(|a| map_album(&a))
            .collect(),
        artists: parts
            .artists
            .iter()
            .map(adapt::search_artist)
            .filter_map(|a| map_artist(&a))
            .collect(),
        playlists: parts
            .playlists
            .iter()
            .filter_map(adapt::search_playlist)
            .filter_map(|p| map_playlist(&p))
            .collect(),
        ..SearchResults::default()
    }
}
