//! Adapters from `ytmapi-rs` types to the raw structs.

use ytmapi_rs::common::{Thumbnail as ApiThumbnail, YoutubeID};
use ytmapi_rs::parse::{
    AlbumResult, ArtistSong, GetAlbum, GetArtist, GetPlaylistDetails, ParsedSongArtist,
    PlaylistItem, SearchResultAlbum, SearchResultArtist, SearchResultFeaturedPlaylist,
    SearchResultPlaylist, SearchResultSong,
};

use super::{
    RawAlbum, RawAlbumPage, RawAlbumTrack, RawArtist, RawArtistPage, RawPlaylist, RawPlaylistPage,
    RawSong, Thumbnail, map_thumbnails,
};

pub fn thumbs(raw: &[ApiThumbnail]) -> Vec<Thumbnail> {
    let list: Vec<(String, u64, u64)> = raw
        .iter()
        .map(|t| (t.url.clone(), t.width, t.height))
        .collect();
    map_thumbnails(&list)
}

fn names(artists: &[ParsedSongArtist]) -> Vec<String> {
    artists.iter().map(|a| a.name.clone()).collect()
}

pub fn search_song(s: &SearchResultSong) -> RawSong {
    RawSong {
        video_id: s.video_id.get_raw().to_owned(),
        title: s.title.clone(),
        artists: vec![s.artist.clone()],
        album: s.album.as_ref().map(|a| a.name.clone()),
        duration: Some(s.duration.clone()),
        thumbnails: thumbs(&s.thumbnails),
    }
}

pub fn search_album(a: &SearchResultAlbum) -> RawAlbum {
    RawAlbum {
        id: a.album_id.get_raw().to_owned(),
        title: a.title.clone(),
        artists: vec![a.artist.clone()],
        year: Some(a.year.clone()),
        thumbnails: thumbs(&a.thumbnails),
    }
}

pub fn search_artist(a: &SearchResultArtist) -> RawArtist {
    RawArtist {
        id: a.browse_id.get_raw().to_owned(),
        name: a.artist.clone(),
        thumbnails: thumbs(&a.thumbnails),
    }
}

pub fn featured_playlist(p: &SearchResultFeaturedPlaylist) -> RawPlaylist {
    RawPlaylist {
        id: p.playlist_id.get_raw().to_owned(),
        title: p.title.clone(),
        author: Some(p.author.clone()),
        thumbnails: thumbs(&p.thumbnails),
    }
}

/// Featured and community playlists. Podcasts, which the playlist filter can return, are
/// dropped.
pub fn search_playlist(p: &SearchResultPlaylist) -> Option<RawPlaylist> {
    match p {
        SearchResultPlaylist::Featured(p) => Some(featured_playlist(p)),
        SearchResultPlaylist::Community(p) => Some(RawPlaylist {
            id: p.playlist_id.get_raw().to_owned(),
            title: p.title.clone(),
            author: Some(p.author.clone()),
            thumbnails: thumbs(&p.thumbnails),
        }),
        _ => None,
    }
}

fn album_result(a: &AlbumResult) -> RawAlbum {
    RawAlbum {
        id: a.album_id.get_raw().to_owned(),
        title: a.title.clone(),
        artists: Vec::new(),
        year: Some(a.year.clone()),
        thumbnails: thumbs(&a.thumbnails),
    }
}

fn artist_song(s: &ArtistSong) -> RawSong {
    RawSong {
        video_id: s.video_id.get_raw().to_owned(),
        title: s.title.clone(),
        artists: names(&s.artists),
        album: Some(s.album.name.clone()),
        duration: None,
        thumbnails: Vec::new(),
    }
}

pub fn album_page(a: &GetAlbum) -> RawAlbumPage {
    RawAlbumPage {
        album: RawAlbum {
            // The caller puts the requested id here; the page does not repeat it.
            id: "page".to_owned(),
            title: a.title.clone(),
            artists: names(&a.artists),
            year: Some(a.year.clone()),
            thumbnails: thumbs(&a.thumbnails),
        },
        tracks: a
            .tracks
            .iter()
            .map(|t| RawAlbumTrack {
                video_id: t.video_id.get_raw().to_owned(),
                title: t.title.clone(),
                duration: Some(t.duration.clone()),
            })
            .collect(),
    }
}

pub fn artist_page(a: &GetArtist) -> RawArtistPage {
    let releases = &a.top_releases;
    let albums = releases
        .albums
        .iter()
        .chain(releases.singles.iter())
        .flat_map(|group| group.results.iter())
        .map(album_result)
        .collect();
    RawArtistPage {
        artist: RawArtist {
            id: "page".to_owned(),
            name: a.name.clone(),
            thumbnails: thumbs(&a.thumbnails),
        },
        description: a.description.clone(),
        songs: releases
            .songs
            .iter()
            .flat_map(|group| group.results.iter())
            .map(artist_song)
            .collect(),
        albums,
    }
}

/// Songs and music videos that are available. Episodes, uploads and greyed-out items are
/// dropped.
pub fn playlist_items(items: &[PlaylistItem]) -> Vec<RawSong> {
    items
        .iter()
        .filter_map(|item| match item {
            PlaylistItem::Song(s) if s.is_available => Some(RawSong {
                video_id: s.video_id.get_raw().to_owned(),
                title: s.title.clone(),
                artists: names(&s.artists),
                album: Some(s.album.name.clone()),
                duration: Some(s.duration.clone()),
                thumbnails: thumbs(&s.thumbnails),
            }),
            PlaylistItem::Video(v) if v.is_available => Some(RawSong {
                video_id: v.video_id.get_raw().to_owned(),
                title: v.title.clone(),
                artists: vec![v.channel_name.clone()],
                album: None,
                duration: Some(v.duration.clone()),
                thumbnails: thumbs(&v.thumbnails),
            }),
            _ => None,
        })
        .collect()
}

pub fn playlist_page(d: &GetPlaylistDetails, items: &[PlaylistItem]) -> RawPlaylistPage {
    RawPlaylistPage {
        playlist: RawPlaylist {
            id: "page".to_owned(),
            title: d.title.clone(),
            author: Some(d.author.clone()),
            thumbnails: thumbs(&d.thumbnails),
        },
        songs: playlist_items(items),
    }
}
