//! One catalog for the UI: local files first, then YouTube Music.
//!
//! Search asks both and keeps whatever answers; a failing source never hides the other one.
//! Pages go to the source that owns the id.

use std::sync::Arc;

use nemo_core::BoxFuture;
use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, BrowseId, Catalog, CatalogError,
    Explore, FeedShelf, HistorySection, HomeFeed, PlaylistDetail, PlaylistSummary, PodcastDetail,
    SearchResults, Shelf,
};
use nemo_core::track::Track;

/// Local and remote catalogs behind one [`Catalog`].
pub struct MergedCatalog {
    local: Arc<dyn Catalog>,
    remote: Arc<dyn Catalog>,
    local_prefix: &'static str,
}

impl MergedCatalog {
    /// `local_prefix` marks ids that belong to the local catalog (for example `local-album:`).
    pub fn new(
        local: Arc<dyn Catalog>,
        remote: Arc<dyn Catalog>,
        local_prefix: &'static str,
    ) -> Self {
        Self {
            local,
            remote,
            local_prefix,
        }
    }

    fn owner(&self, id: &BrowseId) -> &dyn Catalog {
        if id.0.starts_with(self.local_prefix) {
            self.local.as_ref()
        } else {
            self.remote.as_ref()
        }
    }
}

/// Put local results before remote ones. If both failed, report the remote error (the one the
/// user is more likely able to act on, such as no network).
fn merge(
    local: Result<SearchResults, CatalogError>,
    remote: Result<SearchResults, CatalogError>,
) -> Result<SearchResults, CatalogError> {
    match (local, remote) {
        (Ok(mut l), Ok(r)) => {
            l.songs.extend(r.songs);
            l.albums.extend(r.albums);
            l.artists.extend(r.artists);
            l.playlists.extend(r.playlists);
            Ok(l)
        }
        (Ok(l), Err(_)) => Ok(l),
        (Err(_), Ok(r)) => Ok(r),
        (Err(_), Err(e)) => Err(e),
    }
}

impl Catalog for MergedCatalog {
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        Box::pin(async move {
            let local = self.local.search(query).await;
            // An empty query means "list the local library"; never send it to YouTube Music.
            if query.trim().is_empty() {
                return local;
            }
            let remote = self.remote.search(query).await;
            merge(local, remote)
        })
    }

    fn album<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        self.owner(id).album(id)
    }

    fn artist<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        self.owner(id).artist(id)
    }

    fn playlist<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        self.owner(id).playlist(id)
    }

    fn home(&self) -> BoxFuture<'_, Result<Vec<Shelf>, CatalogError>> {
        self.remote.home()
    }

    fn home_feed(&self) -> BoxFuture<'_, Result<HomeFeed, CatalogError>> {
        self.remote.home_feed()
    }

    fn library_playlists(&self) -> BoxFuture<'_, Result<Vec<PlaylistSummary>, CatalogError>> {
        self.remote.library_playlists()
    }

    fn liked_songs(&self) -> BoxFuture<'_, Result<Vec<Track>, CatalogError>> {
        self.remote.liked_songs()
    }

    fn library_albums(&self) -> BoxFuture<'_, Result<Vec<AlbumSummary>, CatalogError>> {
        self.remote.library_albums()
    }

    fn library_artists(&self) -> BoxFuture<'_, Result<Vec<ArtistSummary>, CatalogError>> {
        self.remote.library_artists()
    }

    fn history(&self) -> BoxFuture<'_, Result<Vec<HistorySection>, CatalogError>> {
        self.remote.history()
    }

    fn explore(&self) -> BoxFuture<'_, Result<Explore, CatalogError>> {
        self.remote.explore()
    }

    fn mood_playlists<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<Vec<FeedShelf>, CatalogError>> {
        self.remote.mood_playlists(id)
    }

    fn podcast<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PodcastDetail, CatalogError>> {
        self.remote.podcast(id)
    }
    fn rate_song<'a>(
        &'a self,
        video_id: &'a nemo_core::track::VideoId,
        liked: bool,
    ) -> BoxFuture<'a, Result<(), CatalogError>> {
        self.remote.rate_song(video_id, liked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn results(names: &[&str]) -> SearchResults {
        SearchResults {
            artists: names
                .iter()
                .map(|n| ArtistSummary {
                    id: BrowseId((*n).to_owned()),
                    name: (*n).to_owned(),
                    thumbnails: Vec::new(),
                })
                .collect(),
            ..SearchResults::default()
        }
    }

    #[test]
    fn local_results_come_first() {
        let merged = merge(Ok(results(&["local"])), Ok(results(&["remote"]))).expect("ok");
        let names: Vec<_> = merged.artists.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["local", "remote"]);
    }

    #[test]
    fn one_failing_source_does_not_hide_the_other() {
        let net = CatalogError::Network("offline".into());
        let merged = merge(Ok(results(&["local"])), Err(net.clone())).expect("ok");
        assert_eq!(merged.artists.len(), 1);
        assert_eq!(
            merge(Err(CatalogError::NotFound), Err(net.clone())),
            Err(net)
        );
    }

    struct Fixed(&'static str);
    impl Catalog for Fixed {
        fn search<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
            Box::pin(async { Ok(SearchResults::default()) })
        }
        fn album<'a>(
            &'a self,
            id: &'a BrowseId,
        ) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
            let summary = AlbumSummary {
                id: id.clone(),
                title: self.0.to_owned(),
                artists: Vec::new(),
                year: None,
                thumbnails: Vec::new(),
            };
            Box::pin(async move {
                Ok(AlbumDetail {
                    summary,
                    tracks: Vec::new(),
                })
            })
        }
        fn artist<'a>(
            &'a self,
            _: &'a BrowseId,
        ) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
            Box::pin(async { Err(CatalogError::NotFound) })
        }
        fn playlist<'a>(
            &'a self,
            _: &'a BrowseId,
        ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
            Box::pin(async { Err(CatalogError::NotFound) })
        }
        fn library_playlists(&self) -> BoxFuture<'_, Result<Vec<PlaylistSummary>, CatalogError>> {
            let title = self.0.to_owned();
            Box::pin(async move {
                Ok(vec![PlaylistSummary {
                    id: BrowseId("VLPLx".to_owned()),
                    title,
                    author: None,
                    thumbnails: Vec::new(),
                }])
            })
        }
    }

    #[test]
    fn pages_go_to_the_source_that_owns_the_id() {
        let m = MergedCatalog::new(
            Arc::new(Fixed("local")),
            Arc::new(Fixed("remote")),
            "local-album:",
        );
        let title = |id: &str| {
            let id = BrowseId(id.to_owned());
            poll_ready(m.album(&id)).expect("album").summary.title
        };
        assert_eq!(title("local-album:x"), "local");
        assert_eq!(title("MPREb_abc"), "remote");
    }

    #[test]
    fn account_pages_come_from_the_remote_catalog_only() {
        let m = MergedCatalog::new(
            Arc::new(Fixed("local")),
            Arc::new(Fixed("remote")),
            "local-album:",
        );
        let lists = poll_ready(m.library_playlists()).expect("lists");
        assert_eq!(lists[0].title, "remote");
        // A source that does not know the page leaves it unavailable.
        assert_eq!(
            poll_ready(m.history()),
            Err(CatalogError::Unavailable("history".to_owned()))
        );
    }

    /// Poll a future that is ready at once (these test catalogs never wait).
    fn poll_ready<T>(mut f: BoxFuture<'_, T>) -> T {
        use std::task::{Context, Poll, Waker};
        let mut cx = Context::from_waker(Waker::noop());
        match f.as_mut().poll(&mut cx) {
            Poll::Ready(v) => v,
            Poll::Pending => panic!("test future was not ready"),
        }
    }
}
