//! The catalog that follows the signed-in session.

use std::sync::Arc;

use nemo_core::BoxFuture;
use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, BrowseId, Catalog, CatalogError,
    Explore, FeedShelf, HistorySection, HomeFeed, PlaylistDetail, PlaylistSummary, PodcastDetail,
    SearchResults, Shelf,
};
use nemo_core::track::Track;

use super::SessionSlot;

// ---------------------------------------------------------------------------------------------
// The account-aware catalog
// ---------------------------------------------------------------------------------------------

/// Uses the signed-in session while one is active and the anonymous catalog otherwise. The
/// choice is made per request, so signing in or out takes effect at once.
pub struct AccountCatalog {
    anonymous: Arc<dyn Catalog>,
    slot: SessionSlot,
}

impl AccountCatalog {
    /// `slot` is the one given to [`GoogleAccount::new`].
    #[must_use]
    pub fn new(anonymous: Arc<dyn Catalog>, slot: SessionSlot) -> Self {
        Self { anonymous, slot }
    }

    fn current(&self) -> Arc<dyn Catalog> {
        self.slot
            .read()
            .ok()
            .and_then(|s| s.clone())
            .unwrap_or_else(|| Arc::clone(&self.anonymous))
    }
}

impl Catalog for AccountCatalog {
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.search(query).await })
    }

    fn album<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.album(id).await })
    }

    fn artist<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.artist(id).await })
    }

    fn playlist<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.playlist(id).await })
    }

    fn home(&self) -> BoxFuture<'_, Result<Vec<Shelf>, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.home().await })
    }

    fn home_feed(&self) -> BoxFuture<'_, Result<HomeFeed, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.home_feed().await })
    }

    fn library_playlists(&self) -> BoxFuture<'_, Result<Vec<PlaylistSummary>, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.library_playlists().await })
    }

    fn liked_songs(&self) -> BoxFuture<'_, Result<Vec<Track>, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.liked_songs().await })
    }

    fn library_albums(&self) -> BoxFuture<'_, Result<Vec<AlbumSummary>, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.library_albums().await })
    }

    fn library_artists(&self) -> BoxFuture<'_, Result<Vec<ArtistSummary>, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.library_artists().await })
    }

    fn history(&self) -> BoxFuture<'_, Result<Vec<HistorySection>, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.history().await })
    }

    fn explore(&self) -> BoxFuture<'_, Result<Explore, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.explore().await })
    }

    fn mood_playlists<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<Vec<FeedShelf>, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.mood_playlists(id).await })
    }

    fn podcast<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PodcastDetail, CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.podcast(id).await })
    }
    fn rate_song<'a>(
        &'a self,
        video_id: &'a nemo_core::track::VideoId,
        liked: bool,
    ) -> BoxFuture<'a, Result<(), CatalogError>> {
        let catalog = self.current();
        Box::pin(async move { catalog.rate_song(video_id, liked).await })
    }
}
