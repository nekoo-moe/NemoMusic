//! The Sort control with the mock backend: what the page shows, what Play and Shuffle queue, and
//! what is remembered (collection-sort-meta spec).

use std::sync::{Arc, Mutex};

use nemo_core::catalog::BrowseId;

use crate::collection_sort::{CollectionSort, page_key};
use crate::gui::app::{App, DetailKey, DetailKind, ListSource, Message};
use crate::gui::headless::{Scenario, build_app};
use crate::mock::mock_backend;
use crate::route::Route;
use crate::settings::UiSettings;

fn key() -> DetailKey {
    DetailKey {
        kind: DetailKind::Playlist,
        id: BrowseId("mock-playlist-0".to_owned()),
    }
}

fn open(settings: UiSettings) -> App {
    build_app(
        &mock_backend(),
        &Scenario {
            route: Route::Playlist(key().id),
            settings,
            ..Scenario::default()
        },
    )
}

fn send(app: &mut App, message: Message) {
    let jobs = app.update_core(message);
    app.settle_blocking(jobs);
}

fn titles(app: &App) -> Vec<String> {
    app.tracks_of(&ListSource::Detail(key()))
        .into_iter()
        .map(|t| t.title)
        .collect()
}

#[test]
fn a_sort_reorders_the_page_and_play_follows_it() {
    let mut app = open(UiSettings {
        tos_acknowledged: true,
        ..UiSettings::default()
    });
    let page = titles(&app);
    assert!(page.len() > 3, "the mock playlist loaded");
    send(&mut app, Message::SortMenu(Some(key())));
    assert_eq!(app.sort_menu, Some(key()));
    send(
        &mut app,
        Message::SortCollection {
            key: key(),
            sort: CollectionSort::Title,
        },
    );
    assert_eq!(app.sort_menu, None, "a pick closes the menu");
    let shown = titles(&app);
    let mut expected = page.clone();
    expected.sort_by_key(|t| t.to_lowercase());
    assert_eq!(shown, expected);
    // Row 2 of the sorted page plays row 2 of the sorted list, and the queue is that order.
    send(
        &mut app,
        Message::PlayAt {
            source: ListSource::Detail(key()),
            index: 2,
        },
    );
    let snap = &app.ui.snapshot;
    assert_eq!(
        snap.current.as_ref().map(|t| t.title.clone()),
        Some(shown[2].clone())
    );
    let queued: Vec<String> = snap.queue.iter().map(|t| t.title.clone()).collect();
    assert_eq!(queued, shown);
    // Shuffle draws from the same songs.
    send(&mut app, Message::Shuffle(ListSource::Detail(key())));
    let mut shuffled: Vec<String> = app
        .ui
        .snapshot
        .queue
        .iter()
        .map(|t| t.title.clone())
        .collect();
    shuffled.sort();
    let mut all = shown.clone();
    all.sort();
    assert_eq!(shuffled, all);
}

#[test]
fn the_choice_is_saved_per_page_and_comes_back() {
    let mut app = open(UiSettings {
        tos_acknowledged: true,
        ..UiSettings::default()
    });
    let saved: Arc<Mutex<Vec<UiSettings>>> = Arc::default();
    let sink = Arc::clone(&saved);
    app.on_settings = Some(Arc::new(move |s| {
        if let Ok(mut v) = sink.lock() {
            v.push(s);
        }
    }));
    send(
        &mut app,
        Message::SortCollection {
            key: key(),
            sort: CollectionSort::Duration,
        },
    );
    let last = saved
        .lock()
        .ok()
        .and_then(|v| v.last().cloned())
        .expect("the settings were saved");
    let page = page_key(false, "mock-playlist-0");
    assert_eq!(
        last.collection_sorts.get(&page),
        Some(&CollectionSort::Duration)
    );
    // Another page keeps its own order.
    let other = DetailKey {
        kind: DetailKind::Album,
        id: BrowseId("mock-album-1".to_owned()),
    };
    assert_eq!(app.collection_sort(&other), CollectionSort::Default);
    // A new start with the saved settings shows the page sorted again.
    let reopened = open(last);
    assert_eq!(reopened.collection_sort(&key()), CollectionSort::Duration);
    assert_eq!(titles(&reopened), titles(&app));
    // Back to the default order forgets the entry.
    send(
        &mut app,
        Message::SortCollection {
            key: key(),
            sort: CollectionSort::Default,
        },
    );
    assert!(app.ui.settings.collection_sorts.is_empty());
}
