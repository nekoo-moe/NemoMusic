//! Discord presence logs state names and connect state only (discord-presence spec, criterion
//! 18). This test binary has one global subscriber, so it needs no scoped one.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use nemo_core::audio::Origin;
use nemo_core::hooks::PlayHookEvent;
use nemo_core::player::PlayState;
use nemo_core::track::{Artwork, Thumbnail, Track, TrackId};
use nemo_integrations::discord::ipc::{Connector, Incoming, Link};
use nemo_integrations::discord::{Activity, Config, Input, Timing, Worker};

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Sets(Arc<AtomicBool>);

impl Link for Sets {
    fn set_activity(&mut self, activity: Option<&Activity>) -> io::Result<Incoming> {
        if activity.is_some_and(|a| a.details == "Secret Title ZZZ") {
            self.0.store(true, Ordering::SeqCst);
        }
        Ok(Incoming::Other)
    }

    fn close(self: Box<Self>) {}
}

struct Fake(Arc<AtomicBool>);

impl Connector for Fake {
    fn open(&self, _client_id: &str) -> io::Result<Box<dyn Link>> {
        Ok(Box::new(Sets(Arc::clone(&self.0))))
    }
}

#[test]
fn the_log_names_operations_and_never_the_song() {
    let capture = Capture::default();
    let writer = capture.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("first and only subscriber");

    let shown = Arc::new(AtomicBool::new(false));
    let worker = Worker::start(Config {
        client_id: "1".into(),
        timing: Timing {
            min_gap: Duration::from_millis(10),
            retry: Duration::from_millis(50),
            away_after: Duration::from_mins(10),
        },
        connector: Arc::new(Fake(Arc::clone(&shown))),
    })
    .expect("start");
    let track = Track {
        id: TrackId("secret-id-123".into()),
        title: "Secret Title ZZZ".into(),
        artists: vec!["Secret Artist".into()],
        album: Some("Secret Album".into()),
        duration: Some(Duration::from_secs(200)),
        origin: Origin::YoutubeMusic,
        video_id: None,
        local_path: None,
        artwork: Artwork::Remote(vec![Thumbnail {
            url: "https://i.ytimg.com/vi/secretcover/hq.jpg".into(),
            width: 544,
            height: 544,
        }]),
        source_url: None,
    };
    std::thread::sleep(Duration::from_millis(50));
    worker.send(Input::Play(Box::new(PlayHookEvent::TrackStarted {
        track,
        started_at: SystemTime::UNIX_EPOCH,
    })));
    worker.send(Input::Play(Box::new(PlayHookEvent::StateChanged {
        state: PlayState::Playing,
        position: Duration::ZERO,
    })));
    let until = Instant::now() + Duration::from_secs(5);
    while !shown.load(Ordering::SeqCst) {
        assert!(Instant::now() < until, "the song was never sent");
        std::thread::sleep(Duration::from_millis(5));
    }
    worker.stop(Duration::from_secs(5));

    let log = String::from_utf8_lossy(&capture.0.lock().expect("lock")).into_owned();
    for op in ["connect", "set_activity", "clear"] {
        assert!(
            log.contains(&format!("op=\"{op}\"")) || log.contains(&format!("op={op}")),
            "{op}: {log}"
        );
    }
    assert!(log.contains("nemo_trace"), "{log}");
    assert!(log.contains("provider"), "{log}");
    for secret in [
        "Secret",
        "ZZZ",
        "secret-id",
        "secretcover",
        "ytimg",
        "i.ytimg",
    ] {
        assert!(!log.contains(secret), "{secret} leaked: {log}");
    }
}
