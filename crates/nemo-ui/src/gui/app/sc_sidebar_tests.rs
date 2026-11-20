//! The sidebar list with SoundCloud on (soundcloud-account, PR 4): the source switch, the rows of
//! each source, the sentences when there is nothing, the track page and the sort of the
//! SoundCloud library.

use std::sync::Arc;

use nemo_core::catalog::{ArtistSummary, BrowseId, PlaylistSummary};
use nemo_core::source_account::SourceAccountState;
use nemo_core::track::{Track, TrackId};

use super::*;
use crate::explore::LibSort;
use crate::gui::headless::soundcloud::ScScene;
use crate::gui::headless::{Scenario, build_app};
use crate::gui::views::shell::{sidebar_hint, sidebar_titles};
use crate::mock::{MockScAccount, mock_backend_soundcloud};

fn id(s: &str) -> BrowseId {
    BrowseId(s.to_owned())
}

fn playlist(page: &str, title: &str) -> PlaylistSummary {
    PlaylistSummary {
        id: id(page),
        title: title.to_owned(),
        author: Some("Velvet Harbor".to_owned()),
        thumbnails: Vec::new(),
    }
}

fn sc_track(n: u64, title: &str) -> Track {
    let mut t = crate::mock::track(0);
    t.id = TrackId(format!("sc:{n}"));
    t.title = title.to_owned();
    t.origin = nemo_core::audio::Origin::SoundCloud;
    t.video_id = None;
    t
}

/// An app with SoundCloud on, the account in `state`, and a small SoundCloud library loaded.
fn app(state: SourceAccountState) -> App {
    let signed_in = matches!(state, SourceAccountState::SignedIn(_));
    let mut app = build_app(
        &mock_backend_soundcloud(state),
        &Scenario {
            soundcloud: Some(ScScene::Account),
            ..Scenario::default()
        },
    );
    if signed_in {
        app.sc_lib.data = Arc::new(ScLibData {
            likes: vec![
                sc_track(3, "Charlie"),
                sc_track(1, "Alpha"),
                sc_track(2, "Bravo"),
            ],
            playlists: vec![playlist("sc:playlist:7000022", "Night drive")],
            artists: vec![ArtistSummary {
                id: id("sc:user:8000004"),
                name: "Velvet Harbor".to_owned(),
                thumbnails: Vec::new(),
            }],
            ..ScLibData::default()
        });
        app.sc_lib.phase = ScPhase::Done;
        app.sc_lib.owner = true;
    }
    app
}

fn signed_in() -> SourceAccountState {
    SourceAccountState::SignedIn(MockScAccount::info())
}

fn pick(app: &mut App, source: Source) {
    let _ = app.update_core(Message::Sc(ScMsg::Source(source)));
}

#[test]
fn each_source_lists_its_own_rows_and_all_lists_both() {
    let mut app = app(signed_in());
    pick(&mut app, Source::YouTube);
    let youtube = sidebar_titles(&app);
    assert!(!youtube.iter().any(|t| t == "Night drive"), "{youtube:?}");
    pick(&mut app, Source::SoundCloud);
    assert_eq!(
        sidebar_titles(&app),
        ["Night drive", "Velvet Harbor"],
        "playlists first, then artists"
    );
    pick(&mut app, Source::All);
    let both = sidebar_titles(&app);
    assert_eq!(
        both.len(),
        youtube.len() + 2,
        "{both:?} against {youtube:?}"
    );
    let ours = ["Night drive".to_owned(), "Velvet Harbor".to_owned()];
    for title in youtube.iter().chain(ours.iter()) {
        assert!(both.contains(title), "{title} in {both:?}");
    }
}

#[test]
fn a_soundcloud_row_goes_to_its_own_page() {
    let mut app = app(signed_in());
    pick(&mut app, Source::SoundCloud);
    let _ = app.update_core(Message::Ui(Event::Go(Route::Playlist(id(
        "sc:playlist:7000022",
    )))));
    assert!(matches!(app.ui.route(), Route::Playlist(p) if p.0 == "sc:playlist:7000022"));
}

#[test]
fn the_switch_shows_only_while_soundcloud_is_on_and_running() {
    let on = app(signed_in());
    assert!(on.sc_open());
    let plain = build_app(&crate::mock::mock_backend(), &Scenario::default());
    assert!(!plain.sc_open());
    // The chips are the same three the Search page has.
    assert_eq!(
        Source::ALL.map(Source::label),
        ["YouTube Music", "SoundCloud", "All"]
    );
}

#[test]
fn soundcloud_alone_says_why_it_is_empty() {
    let mut out = app(SourceAccountState::SignedOut);
    pick(&mut out, Source::SoundCloud);
    assert!(sidebar_titles(&out).is_empty());
    assert!(sidebar_hint(&out).starts_with("Sign in to SoundCloud"));

    let mut loading = app(signed_in());
    loading.sc_lib.data = Arc::default();
    loading.sc_lib.phase = ScPhase::Loading;
    pick(&mut loading, Source::SoundCloud);
    assert!(sidebar_hint(&loading).starts_with("Loading your SoundCloud"));
    assert!(loading.sidebar_loading(), "the skeleton may stand in");

    loading.sc_lib.phase = ScPhase::Done;
    assert!(sidebar_hint(&loading).starts_with("Nothing saved on SoundCloud"));
    assert!(!loading.sidebar_loading());

    loading.sc_lib.phase = ScPhase::Failed;
    assert!(sidebar_hint(&loading).starts_with("Could not load your SoundCloud"));
}

#[test]
fn choosing_soundcloud_in_the_sidebar_reads_its_library_once() {
    let mut app = app(signed_in());
    app.sc_lib.phase = ScPhase::Idle;
    app.sc_lib.data = Arc::default();
    let jobs = app.update_core(Message::Sc(ScMsg::Source(Source::SoundCloud)));
    assert_eq!(
        jobs.iter()
            .filter(|j| matches!(j, Job::ScLibrary { .. }))
            .count(),
        1
    );
}

#[test]
fn a_track_page_id_is_made_from_a_soundcloud_track_id_only() {
    assert_eq!(
        sc_track_page(&sc_track(5_000_002, "x")),
        Some(id("sc:track:5000002"))
    );
    let youtube = crate::mock::track(1);
    assert_eq!(sc_track_page(&youtube), None);
    let mut odd = sc_track(1, "x");
    odd.id = TrackId("sc:12/../me".to_owned());
    assert_eq!(sc_track_page(&odd), None);
    odd.id = TrackId("sc:".to_owned());
    assert_eq!(sc_track_page(&odd), None);
}

#[test]
fn a_track_page_is_titled_track_and_a_playlist_page_playlist() {
    assert_eq!(Route::Playlist(id("sc:track:7")).title(), "Track");
    assert_eq!(Route::Playlist(id("sc:playlist:7")).title(), "Playlist");
    assert_eq!(Route::Playlist(id("mock-playlist-0")).title(), "Playlist");
}

#[test]
fn the_likes_tab_and_the_rows_play_in_the_order_they_are_sorted() {
    let mut app = app(signed_in());
    let names = |app: &App| -> Vec<String> {
        app.tracks_of(&ListSource::ScLikes)
            .into_iter()
            .map(|t| t.title)
            .collect()
    };
    assert_eq!(
        names(&app),
        ["Charlie", "Alpha", "Bravo"],
        "SoundCloud's order"
    );
    app.lib_sort = LibSort::Title;
    assert_eq!(names(&app), ["Alpha", "Bravo", "Charlie"]);
    // The library data itself is not reordered, so Recent comes back.
    app.lib_sort = LibSort::Recent;
    assert_eq!(names(&app), ["Charlie", "Alpha", "Bravo"]);
}
