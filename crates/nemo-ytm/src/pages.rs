//! Fetch the account and discovery pages: one place for both the anonymous and the signed-in
//! client, because the requests are the same and only the credential differs.
//!
//! Long lists are read part by part with a short pause between parts and a hard page cap, so a
//! big library is fetched at human pace. A failing first part is an error; a failing later part
//! only ends the list early.

use nemo_core::catalog::{
    AlbumSummary, ArtistSummary, BrowseId, CatalogError, Explore, FeedShelf, HistorySection,
    HomeFeed, PlaylistDetail, PlaylistSummary, PodcastDetail,
};
use nemo_core::track::Track;
use ytmapi_rs::YtMusic;
use ytmapi_rs::auth::AuthToken;

use crate::catalog::valid_browse_id;
use crate::feeds::{
    PAGE_PAUSE, Page, parse_charts, parse_explore, parse_history, parse_home_page,
    parse_library_albums, parse_library_artists, parse_library_playlists, parse_mood_page,
    parse_playlist_header, parse_podcast, parse_song_rows, split_mood_id,
};
use crate::raw::{BrowseQuery, catalog_error};

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

/// A playlist page, public or private, with all its parts (up to the page cap).
pub(crate) async fn playlist<A: AuthToken>(
    api: &YtMusic<A>,
    id: &BrowseId,
) -> Result<PlaylistDetail, CatalogError> {
    if !valid_browse_id(&id.0) {
        return Err(CatalogError::NotFound);
    }
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
    })
}
