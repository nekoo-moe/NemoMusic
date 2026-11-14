//! Fetch the account and discovery pages: one place for both the anonymous and the signed-in
//! client, because the requests are the same and only the credential differs.
//!
//! Long lists are read part by part with a short pause between parts and a hard page cap, so a
//! big library is fetched at human pace. A failing first part is an error; a failing later part
//! only ends the list early.

use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistSummary, BrowseId, CatalogError, Explore, FeedShelf,
    HistorySection, HomeFeed, PlaylistDetail, PlaylistSummary, PodcastDetail,
};
use nemo_core::track::Track;
use ytmapi_rs::YtMusic;
use ytmapi_rs::auth::AuthToken;

use crate::catalog::valid_browse_id;
use crate::feeds::{
    PAGE_PAUSE, Page, parse_charts, parse_explore, parse_history, parse_home_page,
    parse_library_albums, parse_library_artists, parse_library_playlists, parse_mood_page,
    parse_playlist_header, parse_podcast, parse_song_rows, parse_watch_playlist, split_mood_id,
};
use crate::raw::{BrowseQuery, WatchQuery, catalog_error};

/// Parts of the home feed read (the first page shows about three shelves).
const HOME_PAGES: usize = 3;
/// Parts of a library list read.
pub(crate) const LIST_PAGES: usize = 8;
/// Parts of a playlist read (about 100 tracks each).
pub(crate) const PLAYLIST_PAGES: usize = 12;

/// Ask one page.
pub(crate) async fn fetch<A: AuthToken>(
    api: &YtMusic<A>,
    query: &BrowseQuery,
) -> Result<String, CatalogError> {
    api.raw_json_query::<BrowseQuery>(query)
        .await
        .map_err(catalog_error)
}

/// Read a list part by part.
async fn paged<A: AuthToken, T>(
    api: &YtMusic<A>,
    first: BrowseQuery,
    parse: fn(&str) -> Page<T>,
    max_pages: usize,
) -> Result<Vec<T>, CatalogError> {
    let mut out = Vec::new();
    let mut query = first;
    for index in 0..max_pages {
        if index > 0 {
            tokio::time::sleep(PAGE_PAUSE).await;
        }
        let json = match fetch(api, &query).await {
            Ok(json) => json,
            Err(e) if index == 0 => return Err(e),
            Err(_) => break,
        };
        let page = parse(&json);
        out.extend(page.items);
        match page.next {
            Some(token) => query = BrowseQuery::continued(&token),
            None => break,
        }
    }
    Ok(out)
}

/// The home feed: chips from the first part, shelves from the first parts.
pub(crate) async fn home_feed<A: AuthToken>(api: &YtMusic<A>) -> Result<HomeFeed, CatalogError> {
    let mut feed = HomeFeed::default();
    let mut query = BrowseQuery::page("FEmusic_home");
    for index in 0..HOME_PAGES {
        if index > 0 {
            tokio::time::sleep(PAGE_PAUSE).await;
        }
        let json = match fetch(api, &query).await {
            Ok(json) => json,
            Err(e) if index == 0 => return Err(e),
            Err(_) => break,
        };
        let (page, next) = parse_home_page(&json);
        if index == 0 {
            feed.chips = page.chips;
        }
        feed.shelves.extend(page.shelves);
        match next {
            Some(token) => query = BrowseQuery::continued(&token),
            None => break,
        }
    }
    Ok(feed)
}

/// New releases, moods, other shelves and (best effort) the charts.
pub(crate) async fn explore<A: AuthToken>(api: &YtMusic<A>) -> Result<Explore, CatalogError> {
    let json = fetch(api, &BrowseQuery::page("FEmusic_explore")).await?;
    let mut explore = parse_explore(&json);
    tokio::time::sleep(PAGE_PAUSE).await;
    if let Ok(json) = fetch(api, &BrowseQuery::page("FEmusic_charts")).await {
        explore.charts = parse_charts(&json);
    }
    Ok(explore)
}

/// The shelves of a mood or genre.
pub(crate) async fn mood_playlists<A: AuthToken>(
    api: &YtMusic<A>,
    id: &BrowseId,
) -> Result<Vec<FeedShelf>, CatalogError> {
    let (page, params) = split_mood_id(id).ok_or(CatalogError::NotFound)?;
    let query = match params {
        Some(params) => BrowseQuery::with_params(page, params),
        None => BrowseQuery::page(page),
    };
    let json = fetch(api, &query).await?;
    Ok(parse_mood_page(&json))
}

/// A podcast page.
pub(crate) async fn podcast<A: AuthToken>(
    api: &YtMusic<A>,
    id: &BrowseId,
) -> Result<PodcastDetail, CatalogError> {
    if !valid_browse_id(&id.0) {
        return Err(CatalogError::NotFound);
    }
    let json = fetch(api, &BrowseQuery::page(&id.0)).await?;
    parse_podcast(&json, id).ok_or(CatalogError::NotFound)
}

/// The account's playlists.
pub(crate) async fn library_playlists<A: AuthToken>(
    api: &YtMusic<A>,
    max_pages: usize,
) -> Result<Vec<PlaylistSummary>, CatalogError> {
    paged(
        api,
        BrowseQuery::page("FEmusic_liked_playlists"),
        parse_library_playlists,
        max_pages,
    )
    .await
}

/// The account's saved albums.
pub(crate) async fn library_albums<A: AuthToken>(
    api: &YtMusic<A>,
) -> Result<Vec<AlbumSummary>, CatalogError> {
    paged(
        api,
        BrowseQuery::page("FEmusic_liked_albums"),
        parse_library_albums,
        LIST_PAGES,
    )
    .await
}

/// The account's saved artists.
pub(crate) async fn library_artists<A: AuthToken>(
    api: &YtMusic<A>,
) -> Result<Vec<ArtistSummary>, CatalogError> {
    paged(
        api,
        BrowseQuery::page("FEmusic_library_corpus_artists"),
        parse_library_artists,
        LIST_PAGES,
    )
    .await
}

/// The songs the account liked: the `LM` auto playlist.
pub(crate) async fn liked_songs<A: AuthToken>(
    api: &YtMusic<A>,
    max_pages: usize,
) -> Result<Vec<Track>, CatalogError> {
    paged(api, BrowseQuery::page("VLLM"), parse_song_rows, max_pages).await
}

/// The listening history (first part only: it is the recent past).
pub(crate) async fn history<A: AuthToken>(
    api: &YtMusic<A>,
) -> Result<Vec<HistorySection>, CatalogError> {
    let json = fetch(api, &BrowseQuery::page("FEmusic_history")).await?;
    Ok(parse_history(&json))
}

/// A playlist page, public or private, with all its parts (up to the page cap). Radios and
/// mixes (`RD...`) that have no browse page, or whose page lists no tracks, are read as a
/// watch playlist instead.
pub(crate) async fn playlist<A: AuthToken>(
    api: &YtMusic<A>,
    id: &BrowseId,
) -> Result<PlaylistDetail, CatalogError> {
    if !valid_browse_id(&id.0) {
        return Err(CatalogError::NotFound);
    }
    let bare = id.0.strip_prefix("VL").unwrap_or(&id.0).to_owned();
    if bare.starts_with("MPRE") {
        // An album card opened as a playlist.
        let album = album_browse(api, &BrowseId(bare)).await?;
        return Ok(PlaylistDetail {
            summary: PlaylistSummary {
                id: id.clone(),
                title: album.summary.title,
                author: album.summary.artists.first().cloned(),
                thumbnails: album.summary.thumbnails,
            },
            tracks: album.tracks,
            edit: None,
        });
    }
    let browsed = browse_playlist(api, id).await;
    match browsed {
        Ok(detail) if !detail.tracks.is_empty() => Ok(detail),
        other if bare.starts_with("RD") => {
            tokio::time::sleep(PAGE_PAUSE).await;
            let json = fetch_watch(api, &bare).await;
            let tracks = json
                .as_deref()
                .map(parse_watch_playlist)
                .unwrap_or_default();
            match (other, tracks.is_empty()) {
                (Ok(mut detail), false) => {
                    detail.tracks = tracks;
                    Ok(detail)
                }
                (Err(_), false) => Ok(PlaylistDetail {
                    summary: PlaylistSummary {
                        id: id.clone(),
                        title: "Mix".to_owned(),
                        author: None,
                        thumbnails: tracks
                            .first()
                            .and_then(|t| match &t.artwork {
                                nemo_core::track::Artwork::Remote(list) => Some(list.clone()),
                                _ => None,
                            })
                            .unwrap_or_default(),
                    },
                    tracks,
                    edit: None,
                }),
                (Ok(detail), true) => Ok(detail),
                (Err(e), true) => Err(e),
            }
        }
        other => other,
    }
}

/// Ask a watch playlist (`next`).
async fn fetch_watch<A: AuthToken>(api: &YtMusic<A>, bare: &str) -> Result<String, CatalogError> {
    api.raw_json_query::<WatchQuery>(&WatchQuery::playlist(bare))
        .await
        .map_err(catalog_error)
}

/// An album page read from raw JSON: `MPRE...` ids through their browse page, playlist-style
/// ids (`OLAK5uy...`, `VL...`, `PL...`) through [`playlist`]. Tracks without their own artists
/// or cover take the album's.
pub(crate) async fn album<A: AuthToken>(
    api: &YtMusic<A>,
    id: &BrowseId,
) -> Result<AlbumDetail, CatalogError> {
    if !valid_browse_id(&id.0) {
        return Err(CatalogError::NotFound);
    }
    if id.0.starts_with("MPRE") {
        return album_browse(api, id).await;
    }
    let detail = playlist(api, id).await?;
    let header = crate::feeds::PlaylistHeader {
        title: Some(detail.summary.title),
        author: detail.summary.author,
        year: None,
        thumbnails: detail.summary.thumbnails,
    };
    album_from_parts(id, header, detail.tracks).ok_or(CatalogError::NotFound)
}

/// The browse page of an `MPRE...` album.
async fn album_browse<A: AuthToken>(
    api: &YtMusic<A>,
    id: &BrowseId,
) -> Result<AlbumDetail, CatalogError> {
    let json = fetch(api, &BrowseQuery::page(&id.0)).await?;
    let header = parse_playlist_header(&json);
    album_from_parts(id, header, parse_song_rows(&json).items).ok_or(CatalogError::NotFound)
}

/// Build an album from a page header and its rows.
pub(crate) fn album_from_parts(
    id: &BrowseId,
    header: crate::feeds::PlaylistHeader,
    mut tracks: Vec<Track>,
) -> Option<AlbumDetail> {
    let title = header.title?;
    let artists: Vec<String> = header.author.into_iter().collect();
    for track in &mut tracks {
        if track.artists.is_empty() {
            track.artists.clone_from(&artists);
        }
        if track.album.is_none() {
            track.album = Some(title.clone());
        }
        if track.artwork == nemo_core::track::Artwork::None && !header.thumbnails.is_empty() {
            track.artwork = nemo_core::track::Artwork::Remote(header.thumbnails.clone());
        }
    }
    Some(AlbumDetail {
        summary: AlbumSummary {
            id: id.clone(),
            title,
            artists,
            year: header.year,
            thumbnails: header.thumbnails,
        },
        tracks,
    })
}

/// The browse page of a playlist with all its parts (up to the page cap).
async fn browse_playlist<A: AuthToken>(
    api: &YtMusic<A>,
    id: &BrowseId,
) -> Result<PlaylistDetail, CatalogError> {
    let browse = if id.0.starts_with("VL") {
        id.0.clone()
    } else {
        format!("VL{}", id.0)
    };
    let json = fetch(api, &BrowseQuery::page(&browse)).await?;
    let header = parse_playlist_header(&json);
    let first = parse_song_rows(&json);
    let mut tracks = first.items;
    let mut next = first.next;
    for _ in 1..PLAYLIST_PAGES {
        let Some(token) = next.take() else { break };
        tokio::time::sleep(PAGE_PAUSE).await;
        let Ok(json) = fetch(api, &BrowseQuery::continued(&token)).await else {
            break;
        };
        let page = parse_song_rows(&json);
        tracks.extend(page.items);
        next = page.next;
    }
    if header.title.is_none() && tracks.is_empty() {
        return Err(CatalogError::NotFound);
    }
    Ok(PlaylistDetail {
        summary: PlaylistSummary {
            id: id.clone(),
            title: header.title.unwrap_or_else(|| "Playlist".to_owned()),
            author: header.author,
            thumbnails: header.thumbnails,
        },
        tracks,
        edit: None,
    })
}

#[cfg(test)]
mod tests {
    use nemo_core::audio::Origin;
    use nemo_core::track::{Artwork, Thumbnail, TrackId, VideoId};

    use super::*;
    use crate::feeds::PlaylistHeader;

    fn track(id: &str, artists: &[&str]) -> Track {
        Track {
            id: TrackId(id.to_owned()),
            title: "T".to_owned(),
            artists: artists.iter().map(|a| (*a).to_owned()).collect(),
            album: None,
            duration: None,
            origin: Origin::YoutubeMusic,
            video_id: VideoId::parse(id).ok(),
            local_path: None,
            artwork: Artwork::None,
        }
    }

    #[test]
    fn album_rows_inherit_artist_title_and_cover() {
        let cover = vec![Thumbnail {
            url: "https://img.invalid/a".to_owned(),
            width: 1,
            height: 1,
        }];
        let header = PlaylistHeader {
            title: Some("Album".to_owned()),
            author: Some("Band".to_owned()),
            year: Some(2019),
            thumbnails: cover.clone(),
        };
        let id = BrowseId("OLAK5uy_synthetic".to_owned());
        let detail = album_from_parts(
            &id,
            header,
            vec![track("abcdefghijk", &[]), track("bbbbbbbbbbb", &["Guest"])],
        )
        .expect("album");
        assert_eq!(detail.summary.id, id);
        assert_eq!(detail.summary.year, Some(2019));
        assert_eq!(detail.summary.artists, vec!["Band".to_owned()]);
        assert_eq!(detail.tracks[0].artists, vec!["Band".to_owned()]);
        assert_eq!(detail.tracks[1].artists, vec!["Guest".to_owned()]);
        assert_eq!(detail.tracks[0].album.as_deref(), Some("Album"));
        assert_eq!(detail.tracks[0].artwork, Artwork::Remote(cover));
        assert!(album_from_parts(&id, PlaylistHeader::default(), Vec::new()).is_none());
    }
}
