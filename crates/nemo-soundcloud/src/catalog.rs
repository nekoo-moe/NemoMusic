//! [`SoundcloudCatalog`]: search, track, playlist, album and user pages, related tracks.
//!
//! Each call is a few reads through the paced [`ScClient`]: a search is one request, a playlist
//! page is one request plus one per 50 tracks the playlist lists only by id, a user page is three
//! requests. Nothing is fetched in the background. Unofficial access that may breach SoundCloud's
//! terms (`coc.md`); no address, id or token is logged.

use std::collections::HashMap;
use std::sync::Arc;

use nemo_core::BoxFuture;
use nemo_core::catalog::{
    AlbumDetail, ArtistDetail, BrowseId, Catalog, CatalogError, PlaylistDetail, SearchKind,
    SearchResults,
};
use nemo_core::track::{MediaUrl, Track, TrackId};
use serde_json::Value;

use crate::client::{ClientError, ScClient};
use crate::map::{self, Kind};
use crate::model::{Item, ScPlaylist, ScTrack, ScUser, parse_item, parse_list, parse_one};
use crate::records::{TrackRecord, TrackRecords};

/// Longest query sent, in characters.
const MAX_QUERY_CHARS: usize = 200;
/// Results asked for in a search.
const SEARCH_LIMIT: &str = "20";
/// Tracks of a user page.
const USER_TRACKS_LIMIT: &str = "50";
/// Related tracks asked for.
const RELATED_LIMIT: &str = "20";
/// Ids a `/tracks?ids=` call may carry.
const BATCH: usize = 50;
/// Most tracks read from one playlist: ten batches.
const MAX_PLAYLIST_TRACKS: usize = 500;

/// What a SoundCloud link points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// A track.
    Track(Box<Track>),
    /// A playlist, for [`Catalog::playlist`].
    Playlist(BrowseId),
    /// An album, for [`Catalog::album`]. The same page id as a playlist.
    Album(BrowseId),
    /// A user, for [`Catalog::artist`].
    Artist(BrowseId),
}

/// The SoundCloud catalog. Build it only when the switch is on and confirmed.
#[derive(Debug, Clone)]
pub struct SoundcloudCatalog {
    client: Arc<ScClient>,
}

/// A playlist track slot: a full track, or an id to fill from `/tracks?ids=`.
enum Slot {
    Full(Box<Track>),
    Stub(u64),
}

/// Plain texts for the user. They never hold an address, an id or a body.
fn to_catalog(e: ClientError) -> CatalogError {
    match e {
        ClientError::RateLimited => CatalogError::RateLimited,
        ClientError::NotFound => CatalogError::NotFound,
        ClientError::Refused => {
            CatalogError::Unavailable("SoundCloud refused the request".to_owned())
        }
        ClientError::Network => CatalogError::Network("could not reach SoundCloud".to_owned()),
        ClientError::Timeout => CatalogError::Network("the request timed out".to_owned()),
        ClientError::Parse => CatalogError::Parse("unexpected answer from SoundCloud".to_owned()),
        ClientError::NoClientId => {
            CatalogError::Unavailable("SoundCloud changed its web app".to_owned())
        }
    }
}

/// A full track, and a record of its streams for the stream lookup.
fn mapped(records: &TrackRecords, raw: &ScTrack) -> Option<Track> {
    let track = map::track(raw)?;
    if let (Some(page), Some(record)) = (&track.source_url, TrackRecord::of(raw)) {
        records.insert(page.as_str(), record);
    }
    Some(track)
}

fn tracks_of(items: Vec<Value>, records: &TrackRecords) -> Vec<Track> {
    items
        .into_iter()
        .filter_map(parse_item)
        .filter_map(|item| match item {
            Item::Track(t) => mapped(records, &t),
            _ => None,
        })
        .collect()
}

/// Sort a mixed list into the result groups. Albums are playlists with `is_album`.
fn results_of(items: Vec<Value>, records: &TrackRecords) -> SearchResults {
    let mut out = SearchResults::default();
    for item in items.into_iter().filter_map(parse_item) {
        match item {
            Item::Track(t) => out.songs.extend(mapped(records, &t)),
            Item::User(u) => out.artists.extend(map::artist(&u)),
            Item::Playlist(p) => {
                let art = first_track_art(&p);
                if p.is_album {
                    out.albums.extend(map::album(&p, art.as_deref()));
                } else {
                    out.playlists.extend(map::playlist(&p, art.as_deref()));
                }
            }
        }
    }
    out
}

/// The artwork address of the first full track of a playlist, for a cover when it has none.
fn first_track_art(p: &ScPlaylist) -> Option<String> {
    p.tracks
        .iter()
        .find_map(|v| serde_json::from_value::<ScTrack>(v.clone()).ok())
        .and_then(|t| t.artwork_url)
}

impl SoundcloudCatalog {
    /// A catalog over `client`.
    #[must_use]
    pub fn new(client: Arc<ScClient>) -> Self {
        Self { client }
    }

    /// Tracks SoundCloud suggests after `track` (the start of a radio). One request.
    ///
    /// # Errors
    /// [`CatalogError::NotFound`] for an id that is not a SoundCloud track id; otherwise as
    /// every catalog call.
    pub async fn related(&self, track: &TrackId) -> Result<Vec<Track>, CatalogError> {
        let id = map::parse_track_id(track).ok_or(CatalogError::NotFound)?;
        let body = self
            .client
            .get(
                &format!("/tracks/{id}/related"),
                &[("limit", RELATED_LIMIT)],
            )
            .await
            .map_err(to_catalog)?;
        Ok(tracks_of(
            parse_list(&body).map_err(to_catalog)?,
            self.client.records(),
        ))
    }

    /// What a soundcloud.com link points at. One request.
    ///
    /// # Errors
    /// [`CatalogError::NotFound`] when the link is gone or points at something else.
    pub async fn open_link(&self, url: &MediaUrl) -> Result<Link, CatalogError> {
        let body = self
            .client
            .get("/resolve", &[("url", url.as_str())])
            .await
            .map_err(to_catalog)?;
        let value: Value = parse_one(&body).map_err(to_catalog)?;
        match parse_item(value).ok_or(CatalogError::NotFound)? {
            Item::Track(t) => map::track(&t)
                .map(|track| Link::Track(Box::new(track)))
                .ok_or(CatalogError::NotFound),
            Item::Playlist(p) if p.is_album => {
                Ok(Link::Album(map::browse_id(Kind::Playlist, p.id)))
            }
            Item::Playlist(p) => Ok(Link::Playlist(map::browse_id(Kind::Playlist, p.id))),
            Item::User(u) => Ok(Link::Artist(map::browse_id(Kind::User, u.id))),
        }
    }

    async fn search_path(&self, path: &str, query: &str) -> Result<SearchResults, CatalogError> {
        let query: String = query.trim().chars().take(MAX_QUERY_CHARS).collect();
        if query.is_empty() {
            return Ok(SearchResults::default());
        }
        let body = self
            .client
            .get(path, &[("q", &query), ("limit", SEARCH_LIMIT)])
            .await
            .map_err(to_catalog)?;
        Ok(results_of(
            parse_list(&body).map_err(to_catalog)?,
            self.client.records(),
        ))
    }

    /// The playlist object and its tracks in order, stubs filled in batches of [`BATCH`].
    async fn load_playlist(&self, id: &BrowseId) -> Result<(ScPlaylist, Vec<Track>), CatalogError> {
        let (Kind::Playlist, number) = map::parse_browse_id(id).ok_or(CatalogError::NotFound)?
        else {
            return Err(CatalogError::NotFound);
        };
        let body = self
            .client
            .get(&format!("/playlists/{number}"), &[])
            .await
            .map_err(to_catalog)?;
        let playlist: ScPlaylist = parse_one(&body).map_err(to_catalog)?;
        let mut slots = Vec::new();
        for raw in playlist.tracks.iter().take(MAX_PLAYLIST_TRACKS) {
            if let Some(Item::Track(t)) = parse_item(raw.clone()) {
                match mapped(self.client.records(), &t) {
                    Some(full) => slots.push(Slot::Full(Box::new(full))),
                    None if t.title.is_none() => slots.push(Slot::Stub(t.id)),
                    None => {}
                }
            }
        }
        let tracks = self.fill_stubs(slots).await?;
        Ok((playlist, tracks))
    }

    async fn fill_stubs(&self, slots: Vec<Slot>) -> Result<Vec<Track>, CatalogError> {
        let stubs: Vec<u64> = slots
            .iter()
            .filter_map(|s| {
                if let Slot::Stub(id) = s {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut filled: HashMap<u64, Track> = HashMap::new();
        for batch in stubs.chunks(BATCH) {
            let ids = batch
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let body = self
                .client
                .get("/tracks", &[("ids", &ids)])
                .await
                .map_err(to_catalog)?;
            for track in tracks_of(
                parse_list(&body).map_err(to_catalog)?,
                self.client.records(),
            ) {
                if let Some(id) = map::parse_track_id(&track.id) {
                    filled.insert(id, track);
                }
            }
        }
        Ok(slots
            .into_iter()
            .filter_map(|slot| match slot {
                Slot::Full(track) => Some(*track),
                Slot::Stub(id) => filled.remove(&id),
            })
            .collect())
    }

    async fn playlist_page(&self, id: &BrowseId) -> Result<PlaylistDetail, CatalogError> {
        let (p, tracks) = self.load_playlist(id).await?;
        let summary =
            map::playlist(&p, tracks.first().and_then(art_of)).ok_or(CatalogError::NotFound)?;
        Ok(PlaylistDetail {
            summary,
            tracks,
            edit: None,
            meta: map::collection_meta(&p),
        })
    }

    async fn album_page(&self, id: &BrowseId) -> Result<AlbumDetail, CatalogError> {
        let (p, tracks) = self.load_playlist(id).await?;
        let summary =
            map::album(&p, tracks.first().and_then(art_of)).ok_or(CatalogError::NotFound)?;
        Ok(AlbumDetail {
            summary,
            tracks,
            meta: map::collection_meta(&p),
        })
    }

    async fn artist_page(&self, id: &BrowseId) -> Result<ArtistDetail, CatalogError> {
        let (Kind::User, number) = map::parse_browse_id(id).ok_or(CatalogError::NotFound)? else {
            return Err(CatalogError::NotFound);
        };
        let body = self
            .client
            .get(&format!("/users/{number}"), &[])
            .await
            .map_err(to_catalog)?;
        let user: ScUser = parse_one(&body).map_err(to_catalog)?;
        let summary = map::artist(&user).ok_or(CatalogError::NotFound)?;
        let tracks = self
            .client
            .get(
                &format!("/users/{number}/tracks"),
                &[("limit", USER_TRACKS_LIMIT)],
            )
            .await
            .map_err(to_catalog)?;
        let albums = self
            .client
            .get(
                &format!("/users/{number}/albums"),
                &[("limit", SEARCH_LIMIT)],
            )
            .await
            .map_err(to_catalog)?;
        Ok(ArtistDetail {
            summary,
            top_songs: tracks_of(
                parse_list(&tracks).map_err(to_catalog)?,
                self.client.records(),
            ),
            albums: results_of(
                parse_list(&albums).map_err(to_catalog)?,
                self.client.records(),
            )
            .albums,
            description: user.description.filter(|d| !d.trim().is_empty()),
        })
    }
}

/// The first cover address of a track, for a playlist without its own cover.
fn art_of(track: &Track) -> Option<&str> {
    match &track.artwork {
        nemo_core::track::Artwork::Remote(thumbs) => thumbs.first().map(|t| t.url.as_str()),
        _ => None,
    }
}

impl Catalog for SoundcloudCatalog {
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        Box::pin(self.search_path("/search", query))
    }

    fn album<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        Box::pin(self.album_page(id))
    }

    fn artist<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        Box::pin(self.artist_page(id))
    }

    fn playlist<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        Box::pin(self.playlist_page(id))
    }

    fn search_kind<'a>(
        &'a self,
        query: &'a str,
        kind: SearchKind,
    ) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        let path = match kind {
            SearchKind::Songs => "/search/tracks",
            SearchKind::Albums => "/search/albums",
            SearchKind::Artists => "/search/users",
            SearchKind::Playlists => "/search/playlists_without_albums",
            SearchKind::Videos | SearchKind::Channels => {
                return Box::pin(async {
                    Err(CatalogError::Unavailable("search filter".to_owned()))
                });
            }
        };
        Box::pin(self.search_path(path, query))
    }
}

#[cfg(test)]
mod tests;
