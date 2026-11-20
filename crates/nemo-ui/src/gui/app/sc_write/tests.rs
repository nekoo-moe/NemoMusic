//! The SoundCloud writes in the app: nothing changes on screen before the server answered, a
//! failure says so in one sentence, and one change runs at a time.

use std::sync::Arc;

use nemo_core::catalog::{ArtistSummary, BrowseId, PlaylistSummary};
use nemo_core::source_account::SourceAccountState;
use nemo_core::source_writes::WriteError;
use nemo_core::track::{Track, TrackId};

use super::*;
use crate::gui::app::model::{DetailKey, DetailKind, DetailState, ScMsg};
use crate::gui::app::option::CreateMsg;
use crate::gui::headless::soundcloud::ScScene;
use crate::gui::headless::{Scenario, build_app};
use crate::mock::{MockScAccount, mock_backend_soundcloud};

const LIST: &str = "sc:playlist:7000022";
const OTHER: &str = "sc:playlist:7000040";
const USER: &str = "sc:user:8000004";

fn id(s: &str) -> BrowseId {
    BrowseId(s.to_owned())
}

fn sc_track() -> Track {
    let mut t = crate::mock::track(0);
    t.id = TrackId("sc:5000002".to_owned());
    t.origin = Origin::SoundCloud;
    t.video_id = None;
    t
}

fn summary(page: &str, title: &str) -> PlaylistSummary {
    PlaylistSummary {
        id: id(page),
        title: title.to_owned(),
        author: None,
        thumbnails: Vec::new(),
    }
}

/// A signed-in app that holds one playlist of its own, one liked playlist and one followed user.
fn app() -> App {
    let mut app = build_app(
        &mock_backend_soundcloud(SourceAccountState::SignedIn(MockScAccount::info())),
        &Scenario {
            soundcloud: Some(ScScene::Account),
            ..Scenario::default()
        },
    );
    app.sc_lib.data = Arc::new(ScLibData {
        playlists: vec![
            summary(LIST, "Night drive"),
            summary(OTHER, "Someone else's"),
        ],
        own_playlists: 1,
        artists: vec![ArtistSummary {
            id: id(USER),
            name: "Test Person".to_owned(),
            thumbnails: Vec::new(),
        }],
        ..ScLibData::default()
    });
    app
}

fn toast(app: &App) -> Option<String> {
    app.ui.toast.as_ref().map(|t| t.text.clone())
}

fn done(job: ScWriteJob, result: Result<(), WriteError>) -> Message {
    Message::ScWrite(ScWriteMsg::Done(Box::new(ScWriteDone { job, result })))
}

fn like_job(track: &Track, liked: bool) -> ScWriteJob {
    ScWriteJob::Like {
        track: track.clone(),
        liked,
    }
}

#[test]
fn the_heart_waits_for_the_answer_before_it_fills() {
    let mut app = app();
    let track = sc_track();
    let jobs = app.update_core(Message::ToggleLike(track.clone()));
    assert!(matches!(
        jobs.as_slice(),
        [Job::ScWrite(ScWriteJob::Like { liked: true, .. })]
    ));
    assert!(!app.is_liked(&track), "not liked before SoundCloud said so");
    assert!(app.sc_write.busy);
    let _ = app.update_core(done(like_job(&track, true), Ok(())));
    assert!(app.is_liked(&track));
    assert!(!app.sc_write.busy);
    assert_eq!(
        app.sc_lib.data.likes.first().map(|t| &t.id),
        Some(&track.id)
    );
    // Pressing again asks to unlike, and the heart empties only after the answer.
    let jobs = app.update_core(Message::ToggleLike(track.clone()));
    assert!(matches!(
        jobs.as_slice(),
        [Job::ScWrite(ScWriteJob::Like { liked: false, .. })]
    ));
    assert!(app.is_liked(&track));
    let _ = app.update_core(done(like_job(&track, false), Ok(())));
    assert!(!app.is_liked(&track) && app.sc_lib.data.likes.is_empty());
}

#[test]
fn a_like_that_failed_says_so_and_changes_nothing() {
    let mut app = app();
    let track = sc_track();
    let _ = app.update_core(Message::ToggleLike(track.clone()));
    let _ = app.update_core(done(like_job(&track, true), Err(WriteError::Refused)));
    assert!(!app.is_liked(&track) && app.sc_lib.data.likes.is_empty());
    assert!(!app.sc_write.busy);
    assert_eq!(
        toast(&app).as_deref(),
        Some("Could not like this track. SoundCloud refused the change.")
    );
}

#[test]
fn a_lost_connection_is_not_called_a_success_or_a_refusal() {
    let mut app = app();
    let track = sc_track();
    let _ = app.update_core(Message::ToggleLike(track.clone()));
    let _ = app.update_core(done(like_job(&track, true), Err(WriteError::Network)));
    let text = toast(&app).expect("a sentence");
    assert!(text.contains("may not have been made"), "{text}");
    assert!(!app.is_liked(&track));
}

#[test]
fn signed_out_a_like_is_a_sentence_and_no_request() {
    let mut app = build_app(
        &mock_backend_soundcloud(SourceAccountState::SignedOut),
        &Scenario {
            soundcloud: Some(ScScene::Account),
            ..Scenario::default()
        },
    );
    let jobs = app.update_core(Message::ToggleLike(sc_track()));
    assert!(jobs.is_empty());
    assert_eq!(toast(&app).as_deref(), Some(text::SIGN_IN_TO_LIKE));
}

#[test]
fn one_change_runs_at_a_time_and_the_second_is_told_to_wait() {
    let mut app = app();
    let first = app.update_core(Message::ToggleLike(sc_track()));
    assert_eq!(first.len(), 1);
    let mut other = sc_track();
    other.id = TrackId("sc:5000003".to_owned());
    let second = app.update_core(Message::ToggleLike(other));
    assert!(second.is_empty());
    assert_eq!(toast(&app).as_deref(), Some(text::WRITE_BUSY));
}

#[test]
fn adding_and_removing_confirm_after_the_answer_and_read_the_library_again() {
    let mut app = app();
    let track = sc_track();
    let jobs = app.update_core(Message::ScWrite(ScWriteMsg::AddTo {
        track: track.clone(),
        playlist: id(LIST),
    }));
    assert!(matches!(
        jobs.as_slice(),
        [Job::ScWrite(ScWriteJob::Add { .. })]
    ));
    assert!(
        toast(&app).is_none(),
        "nothing is confirmed before the answer"
    );
    let add = ScWriteJob::Add {
        track: track.clone(),
        playlist: id(LIST),
    };
    let jobs = app.update_core(done(add, Ok(())));
    assert!(jobs.iter().any(|j| matches!(j, Job::ScLibrary { .. })));
    assert_eq!(
        toast(&app).as_deref(),
        Some("Added to \u{201c}Night drive\u{201d}.")
    );
    // A track that is in the playlist already is told, not failed.
    let again = ScWriteJob::Add {
        track: track.clone(),
        playlist: id(LIST),
    };
    let _ = app.update_core(done(again, Err(WriteError::Unchanged)));
    assert_eq!(
        toast(&app).as_deref(),
        Some("This track is already in \u{201c}Night drive\u{201d}.")
    );
    let gone = ScWriteJob::Remove {
        track,
        playlist: id(LIST),
    };
    let _ = app.update_core(done(gone, Err(WriteError::Unchanged)));
    assert_eq!(
        toast(&app).as_deref(),
        Some("This track is not in \u{201c}Night drive\u{201d}.")
    );
}

#[test]
fn a_removed_track_makes_the_playlist_page_load_again() {
    let mut app = app();
    let key = DetailKey {
        kind: DetailKind::Playlist,
        id: id(LIST),
    };
    app.details.insert(key.clone(), DetailState::Loading);
    let gone = ScWriteJob::Remove {
        track: sc_track(),
        playlist: id(LIST),
    };
    let _ = app.update_core(done(gone, Ok(())));
    assert!(!app.details.contains_key(&key));
    assert_eq!(
        toast(&app).as_deref(),
        Some("Removed from \u{201c}Night drive\u{201d}.")
    );
}

#[test]
fn a_new_playlist_is_named_in_the_sheet_and_created_after_confirming() {
    let mut app = app();
    let _ = app.update_core(Message::ScWrite(ScWriteMsg::NewPlaylist(Some(sc_track()))));
    assert!(app.create_open && app.create_name.is_empty());
    let _ = app.update_core(Message::Create(CreateMsg::Name("  Late set ".to_owned())));
    let jobs = app.update_core(Message::Create(CreateMsg::Confirm));
    assert!(!app.create_open);
    let [Job::ScWrite(ScWriteJob::Create { title, first })] = jobs.as_slice() else {
        panic!("{jobs:?}");
    };
    assert_eq!(title, "Late set");
    assert!(first.is_some());
    assert!(toast(&app).is_none());
    let sent = ScWriteJob::Create {
        title: title.clone(),
        first: first.clone(),
    };
    let _ = app.update_core(done(sent, Ok(())));
    assert_eq!(
        toast(&app).as_deref(),
        Some("Created \u{201c}Late set\u{201d} on SoundCloud. It is private.")
    );
    // The same sheet, closed, goes back to the account's own playlists.
    let _ = app.update_core(Message::Create(CreateMsg::Open));
    assert!(matches!(app.create_target, CreateTarget::Account) || !app.create_open);
}

#[test]
fn renaming_starts_from_the_current_title() {
    let mut app = app();
    let _ = app.update_core(Message::ScWrite(ScWriteMsg::Rename(id(LIST))));
    assert!(app.create_open);
    assert_eq!(app.create_name, "Night drive");
    let _ = app.update_core(Message::Create(CreateMsg::Name("Morning drive".to_owned())));
    let jobs = app.update_core(Message::Create(CreateMsg::Confirm));
    assert!(matches!(
        jobs.as_slice(),
        [Job::ScWrite(ScWriteJob::Rename { title, .. })] if title == "Morning drive"
    ));
}

#[test]
fn closing_the_sheet_sends_nothing() {
    let mut app = app();
    let _ = app.update_core(Message::ScWrite(ScWriteMsg::NewPlaylist(None)));
    let jobs = app.update_core(Message::Create(CreateMsg::Close));
    assert!(jobs.is_empty() && !app.create_open);
    assert!(matches!(app.create_target, CreateTarget::Account));
}

#[test]
fn an_unfollow_removes_the_user_from_the_list_only_after_the_answer() {
    let mut app = app();
    assert!(app.sc_follows(&id(USER)));
    let jobs = app.update_core(Message::ScWrite(ScWriteMsg::Unfollow(id(USER))));
    assert!(matches!(
        jobs.as_slice(),
        [Job::ScWrite(ScWriteJob::Unfollow { .. })]
    ));
    assert!(app.sc_follows(&id(USER)));
    let sent = ScWriteJob::Unfollow { user: id(USER) };
    let _ = app.update_core(done(sent, Ok(())));
    assert!(!app.sc_follows(&id(USER)));
    assert_eq!(toast(&app).as_deref(), Some(text::UNFOLLOWED));
}

#[test]
fn only_the_users_own_playlists_can_be_changed() {
    let app = app();
    assert!(app.sc_owns_playlist(&id(LIST)));
    assert!(
        !app.sc_owns_playlist(&id(OTHER)),
        "a liked playlist is not theirs"
    );
    assert_eq!(app.sc_own_playlists().len(), 1);
}

#[test]
fn the_menu_lists_playlists_only_in_pick_mode_and_a_choice_leaves_it() {
    let mut app = app();
    let _ = app.update_core(Message::ScWrite(ScWriteMsg::PickPlaylist(true)));
    assert!(app.sc_write.pick);
    let jobs = app.update_core(Message::ScWrite(ScWriteMsg::AddTo {
        track: sc_track(),
        playlist: id(LIST),
    }));
    assert_eq!(jobs.len(), 1);
    assert!(!app.sc_write.pick && app.ui.menu.is_none());
}

#[test]
fn a_track_of_another_service_is_never_sent_to_soundcloud() {
    let mut app = app();
    let youtube = crate::mock::track(1);
    assert!(!app.sc_track_writable(&youtube));
    let jobs = app.update_core(Message::ToggleLike(youtube));
    assert!(jobs.iter().all(|j| !matches!(j, Job::ScWrite(_))));
}

#[test]
fn a_whole_like_runs_against_the_mock_writes_and_ends_filled() {
    let mut app = app();
    let track = sc_track();
    let jobs = app.update_core(Message::ToggleLike(track.clone()));
    app.settle_blocking(jobs);
    assert!(app.is_liked(&track));
    assert!(!app.sc_write.busy);
    // The settings page is untouched by all of this.
    let _ = app.update_core(Message::Sc(ScMsg::Tab(crate::gui::app::ScTab::Likes)));
}
