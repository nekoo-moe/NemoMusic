//! The local library seen through the shared [`Catalog`] contract, so the UI can search local
//! files the same way it searches YouTube Music.
//!
//! Albums are grouped from tags; there are no artist or playlist pages for local files yet.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use nemo_core::BoxFuture;
use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistDetail, BrowseId, Catalog, CatalogError, PlaylistDetail,
    SearchResults,
};
use nemo_core::track::Track;

use crate::LocalLibrary;

/// Prefix of local album ids, so they never clash with YouTube Music ids.
pub const LOCAL_ALBUM_PREFIX: &str = "local-album:";

/// A shared, read-mostly local library behind the [`Catalog`] trait.
#[derive(Clone, Default)]
pub struct LocalCatalog {
    library: Arc<RwLock<LocalLibrary>>,
}

impl LocalCatalog {
    /// Wrap a library. Rescan it through [`LocalCatalog::library`] on a worker thread.
    #[must_use]
    pub fn new(library: LocalLibrary) -> Self {
        Self {
            library: Arc::new(RwLock::new(library)),
        }
    }

    /// The shared library.
    #[must_use]
    pub fn library(&self) -> Arc<RwLock<LocalLibrary>> {
        Arc::clone(&self.library)
    }

    fn with_tracks<T>(&self, f: impl FnOnce(&[Track]) -> T) -> Result<T, CatalogError> {
        self.library
            .read()
            .map(|lib| f(lib.tracks()))
            .map_err(|_| CatalogError::Unavailable("local library is busy".to_owned()))
    }

    fn search_now(&self, query: &str) -> Result<SearchResults, CatalogError> {
        let songs: Vec<Track> = self
            .library
            .read()
            .map_err(|_| CatalogError::Unavailable("local library is busy".to_owned()))?
            .search(query)
            .into_iter()
            .take(100)
            .cloned()
            .collect();
        let albums = group_albums(&songs).into_values().map(|(s, _)| s).collect();
        Ok(SearchResults {
            songs,
            albums,
            ..SearchResults::default()
        })
    }

    fn album_now(&self, id: &BrowseId) -> Result<AlbumDetail, CatalogError> {
        let key =
            id.0.strip_prefix(LOCAL_ALBUM_PREFIX)
                .ok_or(CatalogError::NotFound)?;
        self.with_tracks(|tracks| group_albums(tracks).remove(key))?
            .map(|(summary, tracks)| AlbumDetail { summary, tracks })
            .ok_or(CatalogError::NotFound)
    }
}

/// Group tracks by `artist + album`. Tracks without an album tag are left out.
fn group_albums(tracks: &[Track]) -> BTreeMap<String, (AlbumSummary, Vec<Track>)> {
    let mut map: BTreeMap<String, (AlbumSummary, Vec<Track>)> = BTreeMap::new();
    for t in tracks {
        let Some(album) = &t.album else { continue };
        let artist = t.artists.first().cloned().unwrap_or_default();
        let key = format!("{}\u{1f}{}", artist.to_lowercase(), album.to_lowercase());
        let entry = map.entry(key.clone()).or_insert_with(|| {
            (
                AlbumSummary {
                    id: BrowseId(format!("{LOCAL_ALBUM_PREFIX}{key}")),
                    title: album.clone(),
                    artists: t.artists.clone(),
                    year: None,
                    thumbnails: Vec::new(),
                },
                Vec::new(),
            )
        });
        entry.1.push(t.clone());
    }
    map
}

impl Catalog for LocalCatalog {
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        let result = self.search_now(query);
        Box::pin(async move { result })
    }

    fn album<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        let result = self.album_now(id);
        Box::pin(async move { result })
    }

    fn artist<'a>(
        &'a self,
        _id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        Box::pin(async { Err(CatalogError::Unavailable("local artist pages".to_owned())) })
    }

    fn playlist<'a>(
        &'a self,
        _id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        Box::pin(async { Err(CatalogError::Unavailable("local playlists".to_owned())) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::track_from_path;

    fn track(title: &str, artist: &str, album: Option<&str>) -> Track {
        let mut t = track_from_path(format!("C:/Music/{title}.flac").into());
        t.title = title.to_owned();
        t.artists = vec![artist.to_owned()];
        t.album = album.map(str::to_owned);
        t
    }

    #[test]
    fn albums_group_by_artist_and_album_ignoring_case() {
        let tracks = [
            track("One", "Band", Some("First")),
            track("Two", "band", Some("FIRST")),
            track("Three", "Other", Some("First")),
            track("Loose", "Band", None),
        ];
        let groups = group_albums(&tracks);
        assert_eq!(groups.len(), 2);
        let sizes: Vec<usize> = groups.values().map(|(_, t)| t.len()).collect();
        assert_eq!(sizes, [2, 1]);
        assert!(
            groups
                .values()
                .all(|(s, _)| s.id.0.starts_with(LOCAL_ALBUM_PREFIX))
        );
    }

    #[test]
    fn unknown_album_ids_are_not_found() {
        let catalog = LocalCatalog::default();
        assert_eq!(
            catalog.album_now(&BrowseId("MPREb_x".into())).err(),
            Some(CatalogError::NotFound)
        );
        assert_eq!(
            catalog
                .album_now(&BrowseId(format!("{LOCAL_ALBUM_PREFIX}none")))
                .err(),
            Some(CatalogError::NotFound)
        );
    }

    #[test]
    fn empty_search_returns_nothing() {
        let results = LocalCatalog::default()
            .search_now("anything")
            .expect("search");
        assert_eq!(results, SearchResults::default());
    }
}
