//! Mapping SoundCloud objects to `nemo-core` types.
//!
//! Titles, uploader names, artwork and dates are kept as the source gives them: no attribution is
//! removed (`coc.md`). A track is the uploader's (`user.username`), with the publisher's artist
//! first when it differs. Every image and page address is checked against [`HOSTS`].

use std::time::Duration;

use nemo_core::audio::Origin;
use nemo_core::catalog::{AlbumSummary, ArtistSummary, BrowseId, CollectionMeta, PlaylistSummary};
use nemo_core::track::{Artwork, MediaUrl, Thumbnail, Track, TrackId};

use crate::hosts::HOSTS;
use crate::model::{ScPlaylist, ScTrack, ScUser};

/// The object kinds a [`BrowseId`] can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Track,
    Playlist,
    User,
}

impl Kind {
    const fn word(self) -> &'static str {
        match self {
            Self::Track => "track",
            Self::Playlist => "playlist",
            Self::User => "user",
        }
    }
}

/// `sc:<kind>:<id>`, the id of a page.
pub fn browse_id(kind: Kind, id: u64) -> BrowseId {
    BrowseId(format!("sc:{}:{id}", kind.word()))
}

/// The kind and numeric id of a `sc:<kind>:<id>` page id. The id is digits only, so it is safe
/// inside a request path.
pub fn parse_browse_id(id: &BrowseId) -> Option<(Kind, u64)> {
    let mut parts = id.0.split(':');
    if parts.next()? != "sc" {
        return None;
    }
    let kind = match parts.next()? {
        "track" => Kind::Track,
        "playlist" => Kind::Playlist,
        "user" => Kind::User,
        _ => return None,
    };
    let digits = parts.next()?;
    if parts.next().is_some() || digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().map(|n| (kind, n))
}

/// The numeric id of a track id (`sc:<id>`), or `None` for any other text.
pub fn parse_track_id(id: &TrackId) -> Option<u64> {
    let digits = id.0.strip_prefix("sc:")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Sizes a cover can be asked for. The API gives `-large` (100 px); the web app swaps the size
/// word in the file name.
const SIZES: [(&str, u32); 3] = [("large", 100), ("t300x300", 300), ("t500x500", 500)];

/// Cover sizes of an artwork or avatar address on an allowed host. Empty when the address is
/// missing or not on [`HOSTS`].
pub fn thumbnails(url: Option<&str>) -> Vec<Thumbnail> {
    let Some(url) = url.filter(|u| MediaUrl::parse(u, HOSTS).is_ok()) else {
        return Vec::new();
    };
    let Some(at) = url.find("-large.") else {
        return vec![Thumbnail {
            url: url.to_owned(),
            width: 0,
            height: 0,
        }];
    };
    SIZES
        .iter()
        .map(|(word, px)| Thumbnail {
            url: format!("{}-{word}.{}", &url[..at], &url[at + "-large.".len()..]),
            width: *px,
            height: *px,
        })
        .collect()
}

/// The year in a date such as `2026-11-13T00:00:00Z`.
fn year(date: Option<&str>) -> Option<u16> {
    date?.get(..4)?.parse().ok()
}

/// The page address of a track, if it is on an allowed host.
fn page_url(track: &ScTrack) -> Option<MediaUrl> {
    MediaUrl::parse(track.permalink_url.as_deref()?, HOSTS).ok()
}

/// A full track. `None` for a stub (no title), a track without a page address, or a track that
/// is not on an allowed host.
pub fn track(t: &ScTrack) -> Option<Track> {
    let title = t.title.clone().filter(|s| !s.is_empty())?;
    let source_url = page_url(t)?;
    let uploader = t.user.as_ref().and_then(|u| u.username.clone());
    let publisher = t.publisher_metadata.as_ref();
    let mut artists = Vec::new();
    if let Some(artist) = publisher
        .and_then(|p| p.artist.clone())
        .filter(|a| !a.is_empty())
    {
        artists.push(artist);
    }
    if let Some(name) = uploader.filter(|n| !n.is_empty())
        && !artists.contains(&name)
    {
        artists.push(name);
    }
    let art = t
        .artwork_url
        .as_deref()
        .or_else(|| t.user.as_ref().and_then(|u| u.avatar_url.as_deref()));
    let thumbs = thumbnails(art);
    Some(Track {
        id: TrackId(format!("sc:{}", t.id)),
        title,
        artists,
        album: publisher
            .and_then(|p| p.album_title.clone())
            .filter(|a| !a.is_empty()),
        duration: t.full_duration.or(t.duration).map(Duration::from_millis),
        origin: Origin::SoundCloud,
        video_id: None,
        local_path: None,
        artwork: if thumbs.is_empty() {
            Artwork::None
        } else {
            Artwork::Remote(thumbs)
        },
        source_url: Some(source_url),
    })
}

/// A user as an artist in a list.
pub fn artist(u: &ScUser) -> Option<ArtistSummary> {
    Some(ArtistSummary {
        id: browse_id(Kind::User, u.id),
        name: u.username.clone().filter(|n| !n.is_empty())?,
        thumbnails: thumbnails(u.avatar_url.as_deref()),
    })
}

/// The cover of a playlist: its own, else its first track's, else its owner's picture.
fn playlist_cover(p: &ScPlaylist, first_track_art: Option<&str>) -> Vec<Thumbnail> {
    let art = p
        .artwork_url
        .as_deref()
        .or(first_track_art)
        .or_else(|| p.user.as_ref().and_then(|u| u.avatar_url.as_deref()));
    thumbnails(art)
}

/// A playlist in a list. `None` when it has no title.
pub fn playlist(p: &ScPlaylist, first_track_art: Option<&str>) -> Option<PlaylistSummary> {
    Some(PlaylistSummary {
        id: browse_id(Kind::Playlist, p.id),
        title: p.title.clone().filter(|t| !t.is_empty())?,
        author: p.user.as_ref().and_then(|u| u.username.clone()),
        thumbnails: playlist_cover(p, first_track_art),
    })
}

/// An album (a playlist with `is_album`) in a list. `None` when it has no title.
pub fn album(p: &ScPlaylist, first_track_art: Option<&str>) -> Option<AlbumSummary> {
    Some(AlbumSummary {
        id: browse_id(Kind::Playlist, p.id),
        title: p.title.clone().filter(|t| !t.is_empty())?,
        artists: p
            .user
            .as_ref()
            .and_then(|u| u.username.clone())
            .into_iter()
            .collect(),
        year: year(p.release_date.as_deref().or(p.display_date.as_deref())),
        thumbnails: playlist_cover(p, first_track_art),
    })
}

/// Header facts of a playlist or album page.
pub fn collection_meta(p: &ScPlaylist) -> CollectionMeta {
    CollectionMeta {
        owner: p.user.as_ref().and_then(|u| u.username.clone()),
        owner_avatar: thumbnails(p.user.as_ref().and_then(|u| u.avatar_url.as_deref())),
        year: year(p.release_date.as_deref().or(p.display_date.as_deref())),
        track_count: p.track_count,
        ..CollectionMeta::default()
    }
}

#[cfg(test)]
mod tests;
