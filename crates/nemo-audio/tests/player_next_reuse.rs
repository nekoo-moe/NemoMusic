//! The song prefetched behind the current one starts at once on Next: no second resolve, no
//! second open (fast-start spec, criteria 4 and 5). Fakes only: no device, no network.

mod common;

use std::collections::HashMap;
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
/// How long a resolve of a prefetched song takes: longer than a start may take when the song
/// is already open.
const SLOW: Duration = Duration::from_millis(600);

/// Counts resolves per video; the listed video ids take [`SLOW`] to resolve.
struct Counting {
    inner: FakeResolver,
    slow: Vec<String>,
    resolves: Mutex<HashMap<String, usize>>,
}

impl StreamResolver for Counting {
    fn resolve<'a>(
        &'a self,
        video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        *self
            .resolves
            .lock()
            .expect("resolves")
            .entry(video.as_str().to_owned())
            .or_default() += 1;
        if self.slow.iter().any(|s| s == video.as_str()) {
            std::thread::sleep(SLOW);
        }
        self.inner.resolve(video)
    }
}

type Opens = Arc<Mutex<HashMap<String, usize>>>;

/// Counts opens per video (the fake URL is `mem://<id>`).
struct CountingOpener {
    inner: FakeOpener,
    opens: Opens,
}

impl StreamOpener for CountingOpener {
    fn open(&self, stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        let id = stream.url.expose().trim_start_matches("mem://").to_owned();
        *self.opens.lock().expect("opens").entry(id).or_default() += 1;
        self.inner.open(stream)
    }
}

struct Rig {
    player: Arc<dyn PlayerControl>,
    resolver: Arc<Counting>,
    opens: Opens,
}

impl Rig {
    fn resolves(&self, n: u32) -> usize {
        self.resolver
            .resolves
            .lock()
            .expect("resolves")
            .get(&video_id(n))
            .copied()
            .unwrap_or(0)
    }

    fn opens(&self, n: u32) -> usize {
        self.opens
            .lock()
            .expect("opens")
            .get(&video_id(n))
            .copied()
            .unwrap_or(0)
    }

    fn playing(&self, n: u32) -> bool {
        let snap = self.player.snapshot();
        snap.state == PlayState::Playing && snap.current.is_some_and(|t| t.id.0 == format!("t{n}"))
    }
}

fn start(slow: &[u32]) -> Rig {
    let streams: Vec<(String, Vec<u8>)> = (1..=5)
        .map(|n| (video_id(n), level_wav(1000, 48_000, 48_000 * 20)))
        .collect();
    let resolver = Arc::new(Counting {
        inner: FakeResolver {
            known: streams.iter().map(|(id, _)| id.clone()).collect(),
        },
        slow: slow.iter().map(|n| video_id(*n)).collect(),
        resolves: Mutex::default(),
    });
    let opens = Opens::default();
    let opener = CountingOpener {
        inner: FakeOpener {
            files: streams.into_iter().collect(),
            chunk: None,
        },
        opens: Arc::clone(&opens),
    };
    let player = Player::spawn(PlayerDeps {
        resolver: Arc::clone(&resolver) as Arc<dyn StreamResolver>,
        request: nemo_core::stream::StreamRequest::default(),
        local: None,
        offline: None,
        url_resolver: None,
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
        device_enumerator: None,
        device_watch_interval: None,
    });
    Rig {
        player,
        resolver,
        opens,
    }
}

fn wait(what: &str, mut done: impl FnMut() -> bool) {
    let begin = Instant::now();
    while !done() {
        assert!(begin.elapsed() < LIMIT, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn queue(rig: &Rig, count: u32) {
    rig.player.send(PlayerCommand::SetQueue {
        tracks: (1..=count).map(video_track).collect(),
        start: 0,
    });
}

/// Wait until song `n` is open on the prefetch thread, and a little longer for the probe and
/// the hand-over to the control thread.
fn wait_prefetched(rig: &Rig, n: u32) {
    wait("the prefetch open", || rig.opens(n) >= 1);
    std::thread::sleep(Duration::from_millis(200));
}

#[test]
fn next_starts_the_opened_song_at_once_with_no_second_resolve_or_open() {
    let rig = start(&[2]);
    queue(&rig, 3);
    wait("song 1", || rig.playing(1));
    wait_prefetched(&rig, 2);
    assert_eq!((rig.resolves(2), rig.opens(2)), (1, 1));

    let pressed = Instant::now();
    rig.player.send(PlayerCommand::Next);
    wait("song 2", || rig.playing(2));
    let took = pressed.elapsed();
    assert!(
        took < SLOW * 2 / 3,
        "Next took {took:?}; the open song must not wait for a resolve ({SLOW:?})"
    );
    assert_eq!(rig.resolves(2), 1, "no second resolve");
    wait("song 3 prefetched", || rig.resolves(3) >= 1);
    assert_eq!(rig.resolves(2), 1);
}

#[test]
fn next_while_the_next_song_is_still_opening_takes_over_that_open() {
    let rig = start(&[2]);
    queue(&rig, 3);
    wait("song 1", || rig.playing(1));
    wait("the prefetch resolve starts", || rig.resolves(2) == 1);
    rig.player.send(PlayerCommand::Next);
    wait("song 2", || rig.playing(2));
    assert_eq!(rig.resolves(2), 1, "the open in flight is reused");
    assert_eq!(rig.opens(2), 1, "song 2 is opened once");
}

#[test]
fn a_queue_edit_replaces_the_prefetch_and_only_the_new_next_song_is_opened() {
    let rig = start(&[]);
    queue(&rig, 3);
    wait("song 1", || rig.playing(1));
    wait_prefetched(&rig, 2);
    rig.player.send(PlayerCommand::PlayNext(video_track(4)));
    wait_prefetched(&rig, 4);
    assert_eq!(
        rig.resolves(3),
        0,
        "the song behind the new next is not resolved"
    );
    rig.player.send(PlayerCommand::Next);
    wait("song 4", || rig.playing(4));
    assert_eq!((rig.resolves(4), rig.opens(4)), (1, 1));
    assert_eq!(rig.resolves(5), 0);
}

#[test]
fn a_song_that_is_not_next_is_opened_on_demand() {
    let rig = start(&[]);
    queue(&rig, 4);
    wait("song 1", || rig.playing(1));
    wait_prefetched(&rig, 2);
    rig.player.send(PlayerCommand::SetQueue {
        tracks: (1..=4).map(video_track).collect(),
        start: 3,
    });
    wait("song 4", || rig.playing(4));
    assert_eq!(rig.resolves(4), 1, "opened once, when asked for");
}
