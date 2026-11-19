//! The hint rules, with no window and no backend.

use super::*;

fn vid(text: &str) -> VideoId {
    VideoId::parse(text).expect("valid")
}

#[test]
fn a_pointer_that_rests_starts_the_hint_once_the_delay_ended() {
    let mut s = HintState::default();
    let acts = s.enter(Some(vid("aaaaaaaaaaa")));
    let [Act::Delay(generation)] = acts[..] else {
        panic!("one delay, got {acts:?}");
    };
    assert_eq!(s.due(generation), [Act::Tell(Some(vid("aaaaaaaaaaa")))]);
    assert!(s.due(generation).is_empty(), "told once per hover");
}

#[test]
fn a_pointer_that_leaves_before_the_delay_ended_starts_nothing() {
    let mut s = HintState::default();
    let [Act::Delay(generation)] = s.enter(Some(vid("aaaaaaaaaaa")))[..] else {
        panic!("one delay");
    };
    assert!(s.leave().is_empty(), "nothing was told, nothing to cancel");
    assert!(s.due(generation).is_empty(), "the late timer is ignored");
}

#[test]
fn leaving_after_the_hint_started_cancels_it() {
    let mut s = HintState::default();
    s.press_on(vid("aaaaaaaaaaa"));
    assert_eq!(s.leave(), [Act::Tell(None)]);
    assert!(s.leave().is_empty());
}

#[test]
fn a_press_starts_the_hint_at_once_and_a_second_press_adds_nothing() {
    let mut s = HintState::default();
    let _ = s.enter(Some(vid("aaaaaaaaaaa")));
    assert_eq!(s.press(), [Act::Tell(Some(vid("aaaaaaaaaaa")))]);
    assert!(s.press().is_empty());
}

#[test]
fn a_press_with_the_pointer_on_nothing_does_nothing() {
    let mut s = HintState::default();
    assert!(s.press().is_empty());
    let _ = s.enter(None);
    assert!(s.press().is_empty(), "a target with no song");
}

#[test]
fn moving_to_another_song_cancels_the_first_and_waits_again() {
    let mut s = HintState::default();
    s.press_on(vid("aaaaaaaaaaa"));
    let acts = s.enter(Some(vid("bbbbbbbbbbb")));
    assert_eq!(acts[0], Act::Tell(None));
    assert!(matches!(acts[1], Act::Delay(_)));
    assert_eq!(
        acts.len(),
        2,
        "one cancel and one delay, no second hint yet"
    );
}

impl HintState {
    /// Test helper: the pointer entered a song's target and went down on it.
    fn press_on(&mut self, video: VideoId) {
        let _ = self.enter(Some(video));
        let _ = self.press();
    }
}

mod app {
    //! The messages of the hint through the app, with a recording hook.

    use std::sync::{Arc, Mutex};

    use nemo_core::player::PlayerCommand;
    use nemo_core::track::Track;

    use super::*;
    use crate::gui::app::App;
    use crate::gui::app::message::{Job, Message};
    use crate::gui::headless::{Scenario, build_app};
    use crate::mock::mock_backend;
    use crate::state::Event;

    type Told = Arc<Mutex<Vec<Option<VideoId>>>>;

    fn app_with_hook() -> (App, Told) {
        let mut app = build_app(&mock_backend(), &Scenario::default());
        let told = Told::default();
        let sink = Arc::clone(&told);
        app.hooks.on_hint_song = Some(Arc::new(move |video| {
            sink.lock().expect("lock").push(video);
        }));
        (app, told)
    }

    fn play_of(id: &str, local: bool) -> Message {
        let mut track: Track = crate::mock::track(1);
        track.video_id = VideoId::parse(id).ok();
        track.local_path = local.then(|| std::path::PathBuf::from("song.flac"));
        Message::Ui(Event::Command(PlayerCommand::Play(track)))
    }

    fn delay_of(jobs: &[Job]) -> u64 {
        match jobs {
            [Job::HoverDelay(generation)] => *generation,
            other => panic!("one hover delay, got {other:?}"),
        }
    }

    #[test]
    fn the_hook_hears_a_song_the_pointer_rested_on_and_then_its_cancel() {
        let (mut app, told) = app_with_hook();
        let jobs = app.handle(Message::HoverTarget(Some(Box::new(play_of(
            "aaaaaaaaaaa",
            false,
        )))));
        assert!(
            told.lock().expect("lock").is_empty(),
            "nothing before the delay"
        );
        let generation = delay_of(&jobs);
        assert!(app.handle(Message::HoverDue(generation)).is_empty());
        assert_eq!(*told.lock().expect("lock"), [Some(vid("aaaaaaaaaaa"))]);
        let _ = app.handle(Message::HoverTarget(None));
        assert_eq!(told.lock().expect("lock").len(), 2);
        assert_eq!(told.lock().expect("lock")[1], None, "the cancel");
    }

    #[test]
    fn a_press_tells_the_hook_at_once() {
        let (mut app, told) = app_with_hook();
        let _ = app.handle(Message::HoverTarget(Some(Box::new(play_of(
            "aaaaaaaaaaa",
            false,
        )))));
        let _ = app.handle(Message::PointerDown);
        assert_eq!(*told.lock().expect("lock"), [Some(vid("aaaaaaaaaaa"))]);
    }

    #[test]
    fn a_pointer_that_left_before_the_delay_tells_nothing() {
        let (mut app, told) = app_with_hook();
        let jobs = app.handle(Message::HoverTarget(Some(Box::new(play_of(
            "aaaaaaaaaaa",
            false,
        )))));
        let _ = app.handle(Message::HoverTarget(None));
        let _ = app.handle(Message::HoverDue(delay_of(&jobs)));
        assert!(told.lock().expect("lock").is_empty());
    }

    #[test]
    fn a_local_file_and_a_list_are_never_hinted() {
        let (mut app, told) = app_with_hook();
        let local = app.handle(Message::HoverTarget(Some(Box::new(play_of(
            "aaaaaaaaaaa",
            true,
        )))));
        assert!(local.is_empty(), "a local file has no stream to resolve");
        let list = app.handle(Message::HoverTarget(Some(Box::new(Message::Shuffle(
            crate::gui::app::ListSource::Search,
        )))));
        assert!(list.is_empty(), "a whole list is not a song");
        let _ = app.handle(Message::PointerDown);
        assert!(told.lock().expect("lock").is_empty());
    }

    #[test]
    fn without_a_hook_the_pointer_costs_nothing() {
        let mut app = build_app(&mock_backend(), &Scenario::default());
        let jobs = app.handle(Message::HoverTarget(Some(Box::new(play_of(
            "aaaaaaaaaaa",
            false,
        )))));
        assert!(jobs.is_empty(), "no timer when nobody listens");
        assert!(app.handle(Message::PointerDown).is_empty());
    }
}
