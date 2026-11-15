//! The reload against a scripted catalog and account: no network, no window.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use nemo_core::BoxFuture;
use nemo_core::account::{AccountError, AccountInfo, AccountService, AccountState, SignOutReport};
use nemo_core::catalog::{
    AlbumDetail, ArtistDetail, BrowseId, Catalog, CatalogError, PlaylistDetail, PlaylistSummary,
    SearchResults, Shelf, ShelfItem,
};
use nemo_core::secret::Secret;
use nemo_core::services::Backend;
use nemo_core::track::TrackId;

use super::super::model::AccountMsg;
use super::super::{App, Job, Message};
use crate::gui::headless::{Scenario, build_app};
use crate::mock::{self, MockCatalog, mock_backend};
use crate::refresh::FAILED_NOTICE;
use crate::settings::UiSettings;

static MOCK: MockCatalog = MockCatalog;

/// What the scripted catalog answers, and how often it was asked.
struct Script {
    home: Result<Vec<Shelf>, CatalogError>,
    playlists: Result<Vec<PlaylistSummary>, CatalogError>,
    home_calls: usize,
    playlist_calls: usize,
}

/// The mock catalog with a scripted home page and playlist list.
struct Scripted(Arc<Mutex<Script>>);

impl Scripted {
    fn script(&self) -> std::sync::MutexGuard<'_, Script> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Catalog for Scripted {
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

    fn home(&self) -> BoxFuture<'_, Result<Vec<Shelf>, CatalogError>> {
        let mut s = self.script();
        s.home_calls += 1;
        let answer = s.home.clone();
        Box::pin(async move { answer })
    }

    fn library_playlists(&self) -> BoxFuture<'_, Result<Vec<PlaylistSummary>, CatalogError>> {
        let mut s = self.script();
        s.playlist_calls += 1;
        let answer = s.playlists.clone();
        Box::pin(async move { answer })
    }
}

/// An account whose state the test sets.
struct TestAccount(Mutex<AccountState>);

impl TestAccount {
    fn set(&self, state: AccountState) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = state;
    }
}

impl AccountService for TestAccount {
    fn state(&self) -> AccountState {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn sign_in(&self) -> BoxFuture<'_, Result<AccountInfo, AccountError>> {
        Box::pin(async { Err(AccountError::Unavailable("test".to_owned())) })
    }

    fn sign_in_with_cookie(
        &self,
        _cookie: Secret<String>,
    ) -> BoxFuture<'_, Result<AccountInfo, AccountError>> {
        Box::pin(async { Err(AccountError::Unavailable("test".to_owned())) })
    }

    fn sign_out(&self) -> BoxFuture<'_, Result<SignOutReport, AccountError>> {
        Box::pin(async { Ok(SignOutReport::default()) })
    }
}

fn info(name: &str) -> AccountInfo {
    AccountInfo {
        name: name.to_owned(),
        handle: None,
        avatar: Vec::new(),
    }
}

fn shelf(title: &str) -> Shelf {
    Shelf {
        title: title.to_owned(),
        items: vec![ShelfItem::Song(mock::track(0))],
    }
}

fn playlist(title: &str) -> PlaylistSummary {
    PlaylistSummary {
        id: BrowseId(format!("pl-{title}")),
        title: title.to_owned(),
        author: None,
        thumbnails: Vec::new(),
    }
}

struct Rig {
    app: App,
    script: Arc<Mutex<Script>>,
    account: Arc<TestAccount>,
}

impl Rig {
    /// The app after its start-up loads, with the notice accepted.
    fn new(state: AccountState, home: &str, playlists: Vec<PlaylistSummary>) -> Self {
        let script = Arc::new(Mutex::new(Script {
            home: Ok(vec![shelf(home)]),
            playlists: Ok(playlists),
            home_calls: 0,
            playlist_calls: 0,
        }));
        let account = Arc::new(TestAccount(Mutex::new(state)));
        let backend = Backend {
            catalog: Arc::new(Scripted(Arc::clone(&script))),
            account: Arc::clone(&account) as Arc<dyn AccountService>,
            ..mock_backend()
        };
        let app = build_app(
            &backend,
            &Scenario {
                settings: UiSettings {
                    tos_acknowledged: true,
                    ..UiSettings::default()
                },
                ..Scenario::default()
            },
        );
        Self {
            app,
            script,
            account,
        }
    }

    fn calls(&self) -> (usize, usize) {
        let s = self.script.lock().unwrap_or_else(PoisonError::into_inner);
        (s.home_calls, s.playlist_calls)
    }

    fn answer(
        &self,
        home: Result<Vec<Shelf>, CatalogError>,
        playlists: Result<Vec<PlaylistSummary>, CatalogError>,
    ) {
        let mut s = self.script.lock().unwrap_or_else(PoisonError::into_inner);
        s.home = home;
        s.playlists = playlists;
    }

    /// Handle a message and run every job it leads to.
    fn settle(&mut self, message: Message) {
        let jobs = self.app.update_core(message);
        self.app.settle_blocking(jobs);
    }

    fn home_titles(&self) -> Vec<String> {
        self.app
            .home
            .shelves
            .iter()
            .map(|s| s.title.clone())
            .collect()
    }

    fn library_titles(&self) -> Vec<String> {
        self.app
            .account_library
            .as_ref()
            .map(|l| l.playlists.iter().map(|p| p.title.clone()).collect())
            .unwrap_or_default()
    }

    fn toast(&self) -> Option<String> {
        self.app.ui.toast.as_ref().map(|t| t.text.clone())
    }
}

fn refresh_jobs(jobs: &[Job]) -> usize {
    jobs.iter().filter(|j| matches!(j, Job::Refresh(_))).count()
}

#[test]
fn a_sign_in_reloads_home_and_library_once() {
    let mut rig = Rig::new(AccountState::SignedOut, "Anonymous", Vec::new());
    assert_eq!(rig.calls(), (1, 1), "start-up loads");
    assert_eq!(rig.home_titles(), ["Anonymous"]);
    rig.answer(
        Ok(vec![shelf("Your playlists")]),
        Ok(vec![playlist("Road trip")]),
    );
    rig.account.set(AccountState::SignedIn(info("Mira")));
    rig.settle(Message::Account(AccountMsg::SignedIn(Ok(info("Mira")))));
    assert_eq!(rig.calls(), (2, 2), "one home and one library call");
    assert_eq!(rig.home_titles(), ["Your playlists"]);
    assert_eq!(rig.library_titles(), ["Road trip"]);
    assert!(!rig.app.refresh.busy());
    assert_eq!(rig.app.refresh.generation(), 1);
    // More updates with the same account start nothing.
    rig.settle(Message::TogglePane);
    rig.settle(Message::Frame(Instant::now()));
    assert_eq!(rig.calls(), (2, 2));
    assert_eq!(rig.toast(), None);
}

#[test]
fn an_account_switch_reloads_and_forgets_session_likes() {
    let mut rig = Rig::new(
        AccountState::SignedIn(info("Mira")),
        "Mira home",
        vec![playlist("Mira mix")],
    );
    assert_eq!(rig.library_titles(), ["Mira mix"]);
    rig.app.likes.insert(TrackId("t".to_owned()), true);
    rig.answer(Ok(vec![shelf("Kasumi home")]), Ok(Vec::new()));
    rig.account.set(AccountState::SignedIn(info("Kasumi")));
    rig.settle(Message::TogglePane);
    assert_eq!(rig.home_titles(), ["Kasumi home"]);
    assert!(rig.library_titles().is_empty(), "Kasumi has no playlists");
    assert!(rig.app.likes.is_empty());
}

#[test]
fn a_sign_in_that_ends_in_the_background_is_seen_on_the_next_frame() {
    let mut rig = Rig::new(AccountState::SigningIn, "Anonymous", Vec::new());
    assert_eq!(rig.app.refresh.generation(), 0);
    rig.answer(Ok(vec![shelf("Your playlists")]), Ok(vec![playlist("Mix")]));
    rig.account.set(AccountState::SignedIn(info("Mira")));
    rig.settle(Message::Frame(Instant::now()));
    assert_eq!(rig.app.refresh.generation(), 1);
    assert_eq!(rig.home_titles(), ["Your playlists"]);
}

#[test]
fn a_second_refresh_while_one_runs_does_nothing() {
    let mut rig = Rig::new(AccountState::SignedIn(info("Mira")), "Home", Vec::new());
    let first = rig.app.update_core(Message::Refresh);
    assert_eq!(refresh_jobs(&first), 1);
    assert!(rig.app.refresh.busy());
    for _ in 0..3 {
        let again = rig.app.update_core(Message::Refresh);
        assert_eq!(refresh_jobs(&again), 0);
    }
    rig.app.settle_blocking(first);
    assert!(!rig.app.refresh.busy());
    assert_eq!(rig.calls(), (2, 2), "one reload ran");
}

#[test]
fn a_failed_refresh_keeps_the_old_data_and_says_so_once() {
    let mut rig = Rig::new(
        AccountState::SignedIn(info("Mira")),
        "Mira home",
        vec![playlist("Mira mix")],
    );
    rig.answer(
        Err(CatalogError::Network("offline".to_owned())),
        Err(CatalogError::Network("offline".to_owned())),
    );
    let liked_before = rig.app.daily.liked.len();
    rig.settle(Message::Refresh);
    assert_eq!(rig.home_titles(), ["Mira home"]);
    assert_eq!(rig.library_titles(), ["Mira mix"]);
    assert_eq!(rig.app.daily.liked.len(), liked_before);
    assert_eq!(rig.toast().as_deref(), Some(FAILED_NOTICE));
    assert!(!rig.app.refresh.busy(), "a failure ends the run");
}

#[test]
fn refresh_waits_for_the_notice() {
    let backend = mock_backend();
    let mut app = build_app(
        &backend,
        &Scenario {
            settings: UiSettings {
                tos_acknowledged: false,
                ..UiSettings::default()
            },
            ..Scenario::default()
        },
    );
    assert!(!app.ui.youtube_enabled());
    assert_eq!(refresh_jobs(&app.update_core(Message::Refresh)), 0);
}

#[test]
fn the_refresh_control_names_its_key_and_its_busy_state() {
    use crate::refresh::tooltip as refresh_tip;
    assert_eq!(refresh_tip(false), "Refresh (F5)");
    assert!(refresh_tip(true).starts_with("Refreshing"));
    let mut rig = Rig::new(AccountState::SignedIn(info("Mira")), "Home", Vec::new());
    let jobs = rig.app.update_core(Message::Refresh);
    assert!(rig.app.refresh.busy());
    // The busy top bar builds (the control is dimmed and does nothing).
    drop(rig.app.view());
    rig.app.settle_blocking(jobs);
}
