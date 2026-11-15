//! The Explore page against a counting catalog: no network, no window.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use nemo_core::BoxFuture;
use nemo_core::catalog::{
    AlbumDetail, ArtistDetail, BrowseId, Catalog, CatalogError, Explore, FeedShelf, PlaylistDetail,
    SearchResults,
};
use nemo_core::services::Backend;

use super::super::{App, ExploreMsg, Message};
use crate::explore_feed::{ExplorePhase, FALLBACK_NOTICE, MoodPage, STALE_NOTICE};
use crate::gui::headless::{Scenario, build_app};
use crate::mock::{MockAccount, MockCatalog, mock_backend};
use crate::route::Route;
use crate::settings::UiSettings;
use crate::state::Event;

static MOCK: MockCatalog = MockCatalog;

/// The mock catalog, counting the Explore and mood calls; it can be told to fail them.
#[derive(Default)]
struct Counting {
    explore_calls: AtomicUsize,
    mood_calls: Mutex<Vec<BrowseId>>,
    fail: AtomicBool,
}

impl Counting {
    fn failing(&self) -> bool {
        self.fail.load(Ordering::SeqCst)
    }
}

impl Catalog for Counting {
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        MOCK.search(query)
    }

    fn album<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        MOCK.album(id)
    }

    fn artist<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        MOCK.artist(id)
    }

    fn playlist<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        MOCK.playlist(id)
    }

    fn explore(&self) -> BoxFuture<'_, Result<Explore, CatalogError>> {
        self.explore_calls.fetch_add(1, Ordering::SeqCst);
        if self.failing() {
            return Box::pin(async { Err(CatalogError::Network("offline".to_owned())) });
        }
        MOCK.explore()
    }

    fn mood_playlists<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<Vec<FeedShelf>, CatalogError>> {
        self.mood_calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(id.clone());
        if self.failing() {
            return Box::pin(async { Err(CatalogError::Network("offline".to_owned())) });
        }
        MOCK.mood_playlists(id)
    }
}

struct Rig {
    app: App,
    catalog: Arc<Counting>,
}

impl Rig {
    fn new(accepted: bool, signed_in: bool, fail: bool) -> Self {
        let catalog = Arc::new(Counting::default());
        catalog.fail.store(fail, Ordering::SeqCst);
        let mut backend = Backend {
            catalog: Arc::clone(&catalog) as Arc<dyn Catalog>,
            ..mock_backend()
        };
        if signed_in {
            backend.account = Arc::new(MockAccount::signed_in());
        }
        let app = build_app(
            &backend,
            &Scenario {
                route: Route::Explore,
                settings: UiSettings {
                    tos_acknowledged: accepted,
                    ..UiSettings::default()
                },
                ..Scenario::default()
            },
        );
        Self { app, catalog }
    }

    fn settle(&mut self, message: Message) {
        let jobs = self.app.update_core(message);
        self.app.settle_blocking(jobs);
    }

    fn explore_calls(&self) -> usize {
        self.catalog.explore_calls.load(Ordering::SeqCst)
    }

    fn mood_calls(&self) -> usize {
        self.catalog
            .mood_calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

fn mood_route(rig: &Rig, n: usize) -> Route {
    let chip = rig.app.explore.data().expect("explore loaded").moods[n].clone();
    Route::Mood {
        id: chip.id,
        title: chip.title,
    }
}

#[test]
fn explore_shows_the_catalogs_explore_page_not_the_seed_shelves() {
    let rig = Rig::new(true, false, false);
    assert_eq!(rig.explore_calls(), 1);
    assert_eq!(rig.app.explore.phase(), ExplorePhase::Ready);
    assert!(!rig.app.explore.shows_fallback());
    let e = rig.app.explore.data().expect("explore loaded");
    assert_eq!(e.new_releases.len(), 8);
    assert_eq!(e.moods.len(), 4);
    assert_eq!(
        rig.app.daily.new_releases.len(),
        8,
        "daily inputs use it too"
    );
    assert_eq!(rig.app.daily.moods.len(), 4);
}

#[test]
fn explore_is_asked_once_and_again_after_a_sign_in() {
    let mut rig = Rig::new(true, false, false);
    rig.settle(Message::TogglePane);
    rig.settle(Message::Ui(Event::Go(Route::Library)));
    rig.settle(Message::Ui(Event::Go(Route::Explore)));
    assert_eq!(rig.explore_calls(), 1);
    let mut signed = Rig::new(true, true, false);
    assert_eq!(signed.explore_calls(), 1, "start-up asks with the session");
    signed.settle(Message::Refresh);
    assert_eq!(signed.explore_calls(), 2, "Refresh reloads Explore");
}

#[test]
fn nothing_is_asked_before_the_notice_is_accepted() {
    let rig = Rig::new(false, false, false);
    assert_eq!(rig.explore_calls(), 0);
    assert_eq!(rig.app.explore.phase(), ExplorePhase::Idle);
    assert!(!rig.app.explore.shows_fallback());
}

#[test]
fn a_failed_explore_falls_back_to_the_seed_shelves_with_one_notice_and_retries() {
    let mut rig = Rig::new(true, false, true);
    assert!(rig.app.explore.shows_fallback());
    assert_eq!(rig.app.explore.notice(), Some(FALLBACK_NOTICE));
    assert!(rig.app.ui.toast.is_none(), "a line on the page, no toast");
    rig.settle(Message::TogglePane);
    assert_eq!(rig.explore_calls(), 1, "no retry loop");
    rig.catalog.fail.store(false, Ordering::SeqCst);
    rig.settle(Message::Explore(ExploreMsg::Retry));
    assert_eq!(rig.explore_calls(), 2);
    assert_eq!(rig.app.explore.phase(), ExplorePhase::Ready);
    assert_eq!(rig.app.explore.notice(), None);
}

#[test]
fn a_failed_reload_keeps_the_explore_page_on_screen() {
    let mut rig = Rig::new(true, false, false);
    rig.catalog.fail.store(true, Ordering::SeqCst);
    rig.settle(Message::Explore(ExploreMsg::Retry));
    assert_eq!(rig.explore_calls(), 2);
    assert!(!rig.app.explore.shows_fallback());
    assert_eq!(rig.app.explore.notice(), Some(STALE_NOTICE));
    assert_eq!(
        rig.app.explore.data().map(|e| e.new_releases.len()),
        Some(8)
    );
    assert_eq!(rig.app.daily.new_releases.len(), 8);
}

#[test]
fn a_mood_tile_opens_its_page_from_mood_playlists_once() {
    let mut rig = Rig::new(true, false, false);
    let route = mood_route(&rig, 1);
    let Route::Mood { id, .. } = route.clone() else {
        unreachable!("a mood route");
    };
    rig.settle(Message::Ui(Event::Go(route.clone())));
    assert_eq!(rig.mood_calls(), 1);
    assert!(matches!(rig.app.moods.get(&id), Some(MoodPage::Ready(_))));
    assert_eq!(
        rig.app.ui.route().entry(),
        Some(crate::route::NavEntry::Explore)
    );
    rig.settle(Message::Ui(Event::Back));
    rig.settle(Message::Ui(Event::Go(route)));
    assert_eq!(rig.mood_calls(), 1, "a loaded page is kept");
}

#[test]
fn a_failed_mood_page_can_be_retried() {
    let mut rig = Rig::new(true, false, false);
    let route = mood_route(&rig, 0);
    let Route::Mood { id, .. } = route.clone() else {
        unreachable!("a mood route");
    };
    rig.catalog.fail.store(true, Ordering::SeqCst);
    rig.settle(Message::Ui(Event::Go(route)));
    assert!(matches!(rig.app.moods.get(&id), Some(MoodPage::Failed(_))));
    rig.catalog.fail.store(false, Ordering::SeqCst);
    rig.settle(Message::Explore(ExploreMsg::RetryMood(id.clone())));
    assert_eq!(rig.mood_calls(), 2);
    assert!(matches!(rig.app.moods.get(&id), Some(MoodPage::Ready(_))));
}
