//! The Discord switches of the service against a fake Discord (no socket, no real client).

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime};

use nemo_core::hooks::{PlayHookEvent, PlayObserver};
use nemo_core::integrations::{BrowsingSection, IntegrationsService, WindowContext};
use nemo_core::player::PlayState;
use nemo_core::track::{Artwork, Track, TrackId};

use super::{Integrations, build_with_connector};
use crate::discord::Timing;
use crate::discord::ipc::Connector;
use crate::discord::worker::tests::Fake;
use crate::store::tests::TempDir;
use crate::store::{META_DISCORD_CONFIRMED, Store};

fn timing() -> Timing {
    Timing {
        min_gap: Duration::from_millis(30),
        retry: Duration::from_millis(60),
        away_after: Duration::from_mins(10),
    }
}

fn service(fake: &Arc<Fake>, store: Option<Store>) -> Integrations {
    build_with_connector(store, Arc::clone(fake) as Arc<dyn Connector>, timing())
}

fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(5);
    while !ok() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn seen(fake: &Fake) -> Vec<Option<String>> {
    fake.seen.lock().expect("lock").clone()
}

fn song() -> [PlayHookEvent; 2] {
    let track = Track {
        id: TrackId("t".into()),
        title: "Song".into(),
        artists: vec!["Artist".into()],
        album: None,
        duration: None,
        origin: nemo_core::audio::Origin::YoutubeMusic,
        video_id: None,
        local_path: None,
        artwork: Artwork::None,
        source_url: None,
    };
    [
        PlayHookEvent::TrackStarted {
            track,
            started_at: SystemTime::UNIX_EPOCH,
        },
        PlayHookEvent::StateChanged {
            state: PlayState::Playing,
            position: Duration::ZERO,
        },
    ]
}

#[test]
fn while_the_switch_is_off_nothing_runs_and_nothing_connects() {
    let fake = Arc::new(Fake::default());
    let s = service(&fake, None);
    for event in song() {
        s.on_event(&event);
    }
    s.window_context(WindowContext {
        focused: false,
        ..WindowContext::default()
    });
    s.set_discord_private(true);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(fake.opens.load(Ordering::SeqCst), 0);
    assert!(seen(&fake).is_empty());
}

#[test]
fn turning_on_shows_idle_then_the_song_and_turning_off_clears() {
    let fake = Arc::new(Fake::default());
    let s = service(&fake, None);
    s.set_discord(true);
    wait_for("idle", || seen(&fake) == [Some("Idle".to_owned())]);
    for event in song() {
        s.on_event(&event);
    }
    wait_for("the song", || {
        seen(&fake).last() == Some(&Some("Song".to_owned()))
    });
    s.set_discord(false);
    wait_for("the clear", || seen(&fake).last() == Some(&None));
    assert!(!s.discord());
}

#[test]
fn the_private_switch_is_kept_and_hides_the_song() {
    let fake = Arc::new(Fake::default());
    let s = service(&fake, None);
    assert!(!s.discord_private());
    s.set_discord_private(true);
    assert!(s.discord_private());
    s.set_discord(true);
    for event in song() {
        s.on_event(&event);
    }
    wait_for("the private form", || {
        seen(&fake).contains(&Some("Listening to NemoMusic".to_owned()))
    });
    assert!(!seen(&fake).contains(&Some("Song".to_owned())));
}

#[test]
fn the_window_context_reaches_the_worker_once_per_change() {
    let fake = Arc::new(Fake::default());
    let s = service(&fake, None);
    s.set_discord(true);
    wait_for("idle", || seen(&fake).len() == 1);
    let browsing = WindowContext {
        section: Some(BrowsingSection::Search),
        ..WindowContext::default()
    };
    s.window_context(browsing);
    s.window_context(browsing);
    wait_for("searching", || {
        seen(&fake).last() == Some(&Some("Searching".to_owned()))
    });
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(seen(&fake).len(), 2);
}

#[test]
fn the_switch_is_kept_with_the_notice_version_it_was_confirmed_for() {
    let dir = TempDir::new("discord-switch");
    {
        let fake = Arc::new(Fake::default());
        let s = service(&fake, Some(Store::open(&dir.0).expect("store")));
        s.set_discord(true);
        s.set_discord_private(true);
        wait_for("idle", || seen(&fake).len() == 1);
    }
    // A new run: on, private, and the worker starts by itself.
    let fake = Arc::new(Fake::default());
    let s = service(&fake, Some(Store::open(&dir.0).expect("store")));
    assert!(s.discord() && s.discord_private());
    wait_for("idle after a restart", || !seen(&fake).is_empty());
    drop(s);

    // The notice changed since: the stored switch no longer counts and nothing starts.
    Store::open(&dir.0)
        .expect("store")
        .meta_set(META_DISCORD_CONFIRMED, "0")
        .expect("meta");
    let fake = Arc::new(Fake::default());
    let s = service(&fake, Some(Store::open(&dir.0).expect("store")));
    assert!(!s.discord());
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(fake.opens.load(Ordering::SeqCst), 0);
}

#[test]
fn dropping_the_service_clears_the_presence() {
    let fake = Arc::new(Fake::default());
    let s = service(&fake, None);
    s.set_discord(true);
    wait_for("idle", || seen(&fake).len() == 1);
    drop(s);
    assert_eq!(seen(&fake).last(), Some(&None));
}
