//! Resolver hints from the player: the next queue entries are warmed, and a stream that cannot
//! be opened is reported and resolved once more. No device, no network.

mod common;

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::fakes::{FakeOpener, FakeResolver, ThreadSinkFactory, video_id, video_track};
use common::fixtures::level_wav;
use nemo_audio::player::{Player, PlayerDeps};
use nemo_core::BoxFuture;
use nemo_core::audio::AudioFormat;
use nemo_core::player::{PlayState, PlayerCommand};
use nemo_core::services::{ByteSource, PlayerControl, StreamOpener};
use nemo_core::stream::{ResolvedStream, StreamError, StreamResolver};
use nemo_core::track::VideoId;

const LIMIT: Duration = Duration::from_secs(20);

/// A [`FakeResolver`] that records hints and counts resolves.
struct Logging {
    inner: FakeResolver,
    log: Mutex<Vec<String>>,
    resolves: AtomicUsize,
}

impl StreamResolver for Logging {
    fn resolve<'a>(
        &'a self,
        video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        self.resolves.fetch_add(1, Ordering::SeqCst);
        self.inner.resolve(video)
    }

    fn warm(&self, video: &VideoId) {
        self.log
            .lock()
            .expect("log")
            .push(format!("warm {}", video.as_str()));
    }

    fn stream_failed(&self, video: &VideoId) {
        self.log
            .lock()
            .expect("log")
            .push(format!("failed {}", video.as_str()));
    }
}

/// Refuses the first open of each listed id, like an expired URL (HTTP 403).
struct Flaky {
    inner: FakeOpener,
    refuse_once: Mutex<HashSet<String>>,
}

impl StreamOpener for Flaky {
    fn open(&self, stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        let id = stream.url.expose().trim_start_matches("mem://").to_owned();
        if self.refuse_once.lock().expect("set").remove(&id) {
            return Err(StreamError::Network("HTTP status 403".to_owned()));
        }
        self.inner.open(stream)
    }
}

fn start(refuse: &[u32]) -> (Arc<dyn PlayerControl>, Arc<Logging>) {
    let streams: Vec<(String, Vec<u8>)> = (1..=4)
        .map(|n| (video_id(n), level_wav(1000, 48_000, 48_000 * 20)))
        .collect();
    let resolver = Arc::new(Logging {
        inner: FakeResolver {
            known: streams.iter().map(|(id, _)| id.clone()).collect(),
        },
        log: Mutex::new(Vec::new()),
        resolves: AtomicUsize::new(0),
    });
    let opener = Flaky {
        inner: FakeOpener {
            files: streams.into_iter().collect(),
            chunk: None,
        },
        refuse_once: Mutex::new(refuse.iter().map(|n| video_id(*n)).collect()),
    };
    let player = Player::spawn(PlayerDeps {
        resolver: Arc::clone(&resolver) as Arc<dyn StreamResolver>,
        opener: Arc::new(opener),
        segments: None,
        sink_factory: Arc::new(ThreadSinkFactory {
            fixed: Some(AudioFormat {
                sample_rate: 48_000,
                channels: 2,
                bit_depth: None,
            }),
            capture: Arc::default(),
            opened: Arc::default(),
            interval: Duration::from_micros(2500),
        }),
    });
    (player, resolver)
}

fn wait(what: &str, mut done: impl FnMut() -> bool) {
    let begin = Instant::now();
    while !done() {
        assert!(begin.elapsed() < LIMIT, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn the_next_two_entries_are_warmed_while_one_plays() {
    let (player, resolver) = start(&[]);
    player.send(PlayerCommand::SetQueue {
        tracks: (1..=4).map(video_track).collect(),
        start: 0,
    });
    wait("playing", || player.snapshot().state == PlayState::Playing);
    wait("warm hints", || {
        resolver.log.lock().expect("log").len() >= 2
    });
    let log = resolver.log.lock().expect("log").clone();
    assert_eq!(
        log[..2],
        [
            format!("warm {}", video_id(2)),
            format!("warm {}", video_id(3))
        ]
    );
    assert!(
        !log.iter()
            .any(|l| l.contains(&video_id(1)) || l.contains(&video_id(4)))
    );
}

#[test]
fn a_refused_stream_is_reported_and_resolved_again() {
    let (player, resolver) = start(&[1]);
    player.send(PlayerCommand::Play(video_track(1)));
    wait("playing", || player.snapshot().state == PlayState::Playing);
    let log = resolver.log.lock().expect("log").clone();
    assert!(log.contains(&format!("failed {}", video_id(1))), "{log:?}");
    assert_eq!(resolver.resolves.load(Ordering::SeqCst), 2);
}
