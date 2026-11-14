//! Tests of the defaulted [`Catalog`] methods.

use super::*;

/// A catalog that implements only the required methods.
struct Bare;

impl Catalog for Bare {
    fn search<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        Box::pin(async { Ok(SearchResults::default()) })
    }
    fn album<'a>(&'a self, _: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        Box::pin(async { Err(CatalogError::NotFound) })
    }
    fn artist<'a>(&'a self, _: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        Box::pin(async { Err(CatalogError::NotFound) })
    }
    fn playlist<'a>(
        &'a self,
        _: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        Box::pin(async { Err(CatalogError::NotFound) })
    }
}

fn ready<T>(mut future: BoxFuture<'_, T>) -> T {
    use std::task::{Context, Poll, Waker};
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("default catalog methods answer at once"),
    }
}

fn unavailable<T>(what: &str) -> Result<T, CatalogError> {
    Err(CatalogError::Unavailable(what.to_owned()))
}

#[test]
fn every_added_page_defaults_to_unavailable() {
    let id = BrowseId("x".to_owned());
    assert_eq!(ready(Bare.home_feed()), unavailable("home feed"));
    assert_eq!(
        ready(Bare.library_playlists()),
        unavailable("library playlists")
    );
    assert_eq!(ready(Bare.liked_songs()), unavailable("liked songs"));
    assert_eq!(ready(Bare.library_albums()), unavailable("library albums"));
    assert_eq!(
        ready(Bare.library_artists()),
        unavailable("library artists")
    );
    assert_eq!(ready(Bare.history()), unavailable("history"));
    assert_eq!(ready(Bare.explore()), unavailable("explore"));
    assert_eq!(
        ready(Bare.mood_playlists(&id)),
        unavailable("mood playlists")
    );
    assert_eq!(ready(Bare.podcast(&id)), unavailable("podcasts"));
}

#[test]
fn a_catalog_without_rating_methods_errors_instead_of_looping() {
    let video = VideoId::parse("dQw4w9WgXcQ").expect("valid id");
    let likes = "likes need a signed-in account";
    assert_eq!(ready(Bare.rate_song(&video, true)), unavailable(likes));
    assert_eq!(ready(Bare.rate_song(&video, false)), unavailable(likes));
    assert_eq!(
        ready(Bare.set_rating(&video, LikeStatus::Disliked)),
        unavailable(likes)
    );
}

#[test]
fn shelf_kind_defaults_to_cards() {
    assert_eq!(ShelfKind::default(), ShelfKind::Cards);
}

#[test]
fn youtube_extras_default_to_unavailable() {
    let id = BrowseId("UCx".to_owned());
    assert_eq!(
        ready(Bare.search_scoped("q", SearchScope::AllOfYoutube)),
        unavailable("search scope")
    );
    assert_eq!(ready(Bare.channel(&id)), unavailable("channels"));
    assert_eq!(ready(Bare.live_streams()), unavailable("live streams"));
    assert_eq!(SearchScope::default(), SearchScope::Music);
    let r = SearchResults::default();
    assert!(r.videos.is_empty() && r.channels.is_empty());
}
