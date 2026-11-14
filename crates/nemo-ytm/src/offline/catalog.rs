//! The offline catalog: what the app can browse while offline mode is on (criterion 4).
//!
//! It answers from the index only, so it never makes a request: saved songs, saved albums and
//! saved playlists. Everything else (artist pages, the home feed, account lists) says it is not
//! available offline. `nemo-app` swaps it in for the online catalog while offline mode is on.

use std::sync::Arc;
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::audio::Origin;
use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistDetail, BrowseId, Catalog, CatalogError, PlaylistDetail,
    PlaylistSummary, SearchResults,
};
use nemo_core::track::{Artwork, Thumbnail, Track, TrackId, VideoId};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{OfflineCache, OfflineError};

/// The text of every "not available offline" answer.
const NOT_OFFLINE: &str = "not available in offline mode";

/// What `song.meta` holds: the song as the user saw it when saving, as JSON.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct SongMeta {
    pub(super) title: String,
    pub(super) artists: Vec<String>,
    pub(super) album: Option<String>,
    pub(super) duration_ms: Option<u64>,
    /// Cover sizes; the offline image source answers them from `art/`.
    pub(super) thumbnails: Vec<(String, u32, u32)>,
}

impl From<&Track> for SongMeta {
    fn from(track: &Track) -> Self {
        let thumbnails = match &track.artwork {
            Artwork::Remote(all) => all
                .iter()
                .map(|t| (t.url.clone(), t.width, t.height))
                .collect(),
            Artwork::None | Artwork::LocalFile(_) => Vec::new(),
        };
        Self {
            title: track.title.clone(),
            artists: track.artists.clone(),
            album: track.album.clone(),
            duration_ms: track
                .duration
                .and_then(|d| u64::try_from(d.as_millis()).ok()),
            thumbnails,
        }
    }
}

impl SongMeta {
    fn into_track(self, video: VideoId) -> Track {
        let artwork = if self.thumbnails.is_empty() {
            Artwork::None
        } else {
            Artwork::Remote(
                self.thumbnails
                    .into_iter()
                    .map(|(url, width, height)| Thumbnail { url, width, height })
                    .collect(),
            )
        };
        Track {
            id: TrackId(video.as_str().to_owned()),
            title: self.title,
            artists: self.artists,
            album: self.album,
            duration: self.duration_ms.map(Duration::from_millis),
            origin: Origin::YoutubeMusic,
            video_id: Some(video),
            local_path: None,
            artwork,
        }
    }
}

/// Turn index rows `(video_id, meta)` into tracks; rows that do not parse are left out.
fn tracks(rows: Vec<(String, String)>) -> Vec<Track> {
    rows.into_iter()
        .filter_map(|(id, meta)| {
            let video = VideoId::parse(&id).ok()?;
            let meta: SongMeta = serde_json::from_str(&meta).ok()?;
            Some(meta.into_track(video))
        })
        .collect()
}

/// The [`Catalog`] of saved songs, albums and playlists. Makes no request.
#[derive(Debug, Clone)]
pub struct OfflineCatalog {
    cache: Arc<OfflineCache>,
}

impl OfflineCatalog {
    /// A catalog over the index of `cache`.
    #[must_use]
    pub fn new(cache: Arc<OfflineCache>) -> Self {
        Self { cache }
    }

    fn saved_songs(&self) -> Result<Vec<Track>, OfflineError> {
        let rows = self.cache.store.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT video_id, meta FROM song ORDER BY saved_at")?;
            let rows = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_, _>>()?;
            Ok(rows)
        })?;
        Ok(tracks(rows))
    }

    /// Saved collections of `kind` as `(browse id, title)`.
    fn collections(&self, kind: &str) -> Result<Vec<(String, String)>, OfflineError> {
        self.cache.store.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT browse_id, title FROM collection WHERE kind = ?1 ORDER BY saved_at",
            )?;
            let rows = stmt
                .query_map([kind], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_, _>>()?;
            Ok(rows)
        })
    }

    /// The title and saved songs of one collection, in its order; `None` when it is not saved.
    fn collection(
        &self,
        kind: &str,
        id: &BrowseId,
    ) -> Result<Option<(String, Vec<Track>)>, OfflineError> {
        let found = self.cache.store.with_conn(|conn| {
            let head: Option<(i64, String)> = conn
                .query_row(
                    "SELECT id, title FROM collection WHERE kind = ?1 AND browse_id = ?2",
                    params![kind, id.0],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((key, title)) = head else {
                return Ok(None);
            };
            let mut stmt = conn.prepare(
                "SELECT s.video_id, s.meta FROM collection_song c
                 JOIN song s ON s.video_id = c.video_id
                 WHERE c.collection_id = ?1 ORDER BY c.position",
            )?;
            let rows: Vec<(String, String)> = stmt
                .query_map([key], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_, _>>()?;
            Ok(Some((title, rows)))
        })?;
        Ok(found.map(|(title, rows)| (title, tracks(rows))))
    }

    fn albums(&self) -> Result<Vec<AlbumSummary>, OfflineError> {
        Ok(self
            .collections("album")?
            .into_iter()
            .map(|(id, title)| album_summary(BrowseId(id), title))
            .collect())
    }

    fn playlists(&self) -> Result<Vec<PlaylistSummary>, OfflineError> {
        Ok(self
            .collections("playlist")?
            .into_iter()
            .map(|(id, title)| playlist_summary(BrowseId(id), title))
            .collect())
    }

    fn search_now(&self, query: &str) -> Result<SearchResults, OfflineError> {
        let needle = query.trim().to_lowercase();
        let hit = |text: &str| text.to_lowercase().contains(&needle);
        let songs = self
            .saved_songs()?
            .into_iter()
            .filter(|t| {
                hit(&t.title)
                    || t.artists.iter().any(|a| hit(a))
                    || t.album.as_deref().is_some_and(hit)
            })
            .collect();
        let mut albums = self.albums()?;
        albums.retain(|a| hit(&a.title));
        let mut playlists = self.playlists()?;
        playlists.retain(|p| hit(&p.title));
        Ok(SearchResults {
            songs,
            albums,
            playlists,
            ..SearchResults::default()
        })
    }
}

fn album_summary(id: BrowseId, title: String) -> AlbumSummary {
    AlbumSummary {
        id,
        title,
        artists: Vec::new(),
        year: None,
        thumbnails: Vec::new(),
    }
}

fn playlist_summary(id: BrowseId, title: String) -> PlaylistSummary {
    PlaylistSummary {
        id,
        title,
        author: None,
        thumbnails: Vec::new(),
    }
}

/// Index failures become a plain catalog error; the text names no path or id.
fn db(err: OfflineError) -> CatalogError {
    CatalogError::Unavailable(err.to_string())
}

impl Catalog for OfflineCatalog {
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        let answer = self.search_now(query).map_err(db);
        Box::pin(async move { answer })
    }

    fn album<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        let answer = match self.collection("album", id) {
            Ok(Some((title, tracks))) => Ok(AlbumDetail {
                summary: album_summary(id.clone(), title),
                tracks,
            }),
            Ok(None) => Err(CatalogError::NotFound),
            Err(err) => Err(db(err)),
        };
        Box::pin(async move { answer })
    }

    fn artist<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        let _ = id;
        Box::pin(async { Err(CatalogError::Unavailable(NOT_OFFLINE.to_owned())) })
    }

    fn playlist<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        let answer = match self.collection("playlist", id) {
            Ok(Some((title, tracks))) => Ok(PlaylistDetail {
                summary: playlist_summary(id.clone(), title),
                tracks,
                edit: None,
            }),
            Ok(None) => Err(CatalogError::NotFound),
            Err(err) => Err(db(err)),
        };
        Box::pin(async move { answer })
    }

    fn library_playlists(&self) -> BoxFuture<'_, Result<Vec<PlaylistSummary>, CatalogError>> {
        let answer = self.playlists().map_err(db);
        Box::pin(async move { answer })
    }

    fn library_albums(&self) -> BoxFuture<'_, Result<Vec<AlbumSummary>, CatalogError>> {
        let answer = self.albums().map_err(db);
        Box::pin(async move { answer })
    }
}
