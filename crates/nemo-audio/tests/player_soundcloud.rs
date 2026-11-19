//! SoundCloud tracks through the player (soundcloud-full): a track with a page address and no
//! video id resolves through the URL resolver, its signal path carries the resolver's origin and
//! bitrate, a stream that cannot be opened is reported and resolved once more, and the next
//! SoundCloud entry is warmed. No device, no network.

mod common;

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::fakes::{FakeOpener, FakeResolver, ThreadSinkFactory, video_id, video_track};
use common::fixtures::level_wav;
use nemo_audio::player::{Player, PlayerDeps};
use nemo_core::BoxFuture;
use nemo_core::audio::{AudioFormat, Codec, Origin, StreamInfo};
use nemo_core::player::{PlayState, PlayerCommand};
use nemo_core::secret::Secret;
use nemo_core::services::{ByteSource, PlayerControl, StreamOpener};
use nemo_core::stream::{ResolvedStream, StreamError, StreamRequest, Transport};
use nemo_core::track::{Artwork, MediaUrl, Track, TrackId};
use nemo_core::url_resolver::{FormatPolicy, MediaEntry, MediaQuery, ResolverState, UrlResolver};

const LIMIT: Duration = Duration::from_secs(20);
const HOSTS: &[&str] = &["soundcloud.com"];

fn page(n: u32) -> MediaUrl {
    MediaUrl::parse(
        &format!("https://soundcloud.com/fixture-artist/track-{n}"),
        HOSTS,
    )
    .expect("valid page")
}

fn sc_track(n: u32) -> Track {
    Track {
        id: TrackId(format!("sc:{n}")),
        title: format!("Fixture Track {n}"),
        artists: vec!["Fixture Artist".into()],
        album: None,
        duration: None,
        origin: Origin::SoundCloud,
        video_id: None,
        local_path: None,
        artwork: Artwork::None,
        source_url: Some(page(n)),
    }
}

/// Answers every page with `mem://sc<n>` stated as AAC 160 kbps, and logs what it is asked.
#[derive(Default)]
struct FakeUrls {
    log: Mutex<Vec<String>>,
    policies: Mutex<Vec<FormatPolicy>>,
    resolves: AtomicUsize,
}

fn name(url: &MediaUrl) -> String {
    url.as_str()
        .rsplit('-')
        .next()
        .unwrap_or_default()
        .to_owned()
}

impl UrlResolver for FakeUrls {
    fn resolve<'a>(
        &'a self,
        url: &'a MediaUrl,
        policy: &'a FormatPolicy,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        self.resolves.fetch_add(1, Ordering::SeqCst);
        self.policies.lock().expect("policies").push(policy.clone());
        let id = name(url);
        Box::pin(async move {
            Ok(ResolvedStream {
                url: Secret::new(format!("mem://sc{id}")),
                info: StreamInfo {
                    origin: Origin::SoundCloud,
                    codec: Codec::Aac,
                    bitrate_kbps: Some(160),
                    format: None,
                },
                expires_at: None,
                transport: Transport::Hls { live: false },
                preview: false,
                audio_language: None,
                audio_tracks: Vec::new(),
            })
        })
    }

    fn list<'a>(
        &'a self,
        _query: &'a MediaQuery,
    ) -> BoxFuture<'a, Result<Vec<MediaEntry>, StreamError>> {
        Box::pin(async { Err(StreamError::NotAvailable) })
    }

    fn state(&self) -> ResolverState {
        ResolverState::Missing
    }

    fn warm(&self, url: &MediaUrl) {
        self.log
            .lock()
            .expect("log")
            .push(format!("warm {}", name(url)));
    }

    fn stream_failed(&self, url: &MediaUrl) {
        self.log
            .lock()
            .expect("log")
            .push(format!("failed {}", name(url)));
    }
}

/// Refuses the first open of each listed stream, like an address that stopped working.
struct Flaky {
    inner: FakeOpener,
    refuse_once: Mutex<Vec<String>>,
}

impl StreamOpener for Flaky {
    fn open(&self, stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        let id = stream.url.expose().trim_start_matches("mem://").to_owned();
        let mut refuse = self.refuse_once.lock().expect("refuse");
        if let Some(at) = refuse.iter().position(|r| *r == id) {
            refuse.remove(at);
            return Err(StreamError::Network("HTTP status 403".to_owned()));
        }
        drop(refuse);
        self.inner.open(stream)
    }
}

fn start(refuse: &[&str], with_resolver: bool) -> (Arc<dyn PlayerControl>, Arc<FakeUrls>) {
    let files: HashMap<String, Vec<u8>> = (1..=3)
        .map(|n| (format!("sc{n}"), level_wav(1000, 48_000, 48_000 * 20)))
        .chain((1..=2).map(|n| (video_id(n), level_wav(1000, 48_000, 48_000 * 20))))
        .collect();
    let urls = Arc::new(FakeUrls::default());
    let player = Player::spawn(PlayerDeps {
        resolver: Arc::new(FakeResolver {
            known: (1..=2).map(video_id).collect(),
        }),
        request: StreamRequest::default(),
        local: None,
        offline: None,
        url_resolver: with_resolver.then(|| Arc::clone(&urls) as Arc<dyn UrlResolver>),
        opener: Arc::new(Flaky {
            inner: FakeOpener { files, chunk: None },
            refuse_once: Mutex::new(refuse.iter().map(|s| (*s).to_owned()).collect()),
        }),
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
    (player, urls)
}

fn wait(what: &str, mut done: impl FnMut() -> bool) {
    let begin = Instant::now();
    while !done() {
        assert!(begin.elapsed() < LIMIT, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_soundcloud_track_plays_and_the_signal_path_names_soundcloud() {
    let (player, urls) = start(&[], true);
    player.send(PlayerCommand::Play(sc_track(1)));
    wait("playing", || player.snapshot().state == PlayState::Playing);
    let source = player.snapshot().signal_path.expect("signal path").source;
    assert_eq!(source.origin, Origin::SoundCloud);
    assert_eq!(source.bitrate_kbps, Some(160));
    let policies = urls.policies.lock().expect("policies").clone();
    assert_eq!(policies.len(), 1);
    assert!(policies[0].allow_hls);
    for codec in [Codec::Aac, Codec::Mp3] {
        assert!(policies[0].playable.contains(&codec), "{codec:?}");
    }
}

#[test]
fn a_soundcloud_track_without_a_resolver_fails_plainly() {
    let (player, _) = start(&[], false);
    player.send(PlayerCommand::Play(sc_track(1)));
    std::thread::sleep(Duration::from_millis(300));
    assert_ne!(player.snapshot().state, PlayState::Playing);
}

#[test]
fn a_refused_soundcloud_stream_is_reported_and_looked_up_again() {
    let (player, urls) = start(&["sc1"], true);
    player.send(PlayerCommand::Play(sc_track(1)));
    wait("playing", || player.snapshot().state == PlayState::Playing);
    assert_eq!(urls.log.lock().expect("log").clone(), ["failed 1"]);
    assert_eq!(urls.resolves.load(Ordering::SeqCst), 2);
}

#[test]
fn the_next_soundcloud_entry_is_warmed_and_a_youtube_entry_is_not_sent_to_it() {
    let (player, urls) = start(&[], true);
    player.send(PlayerCommand::SetQueue {
        tracks: vec![video_track(1), sc_track(2), video_track(2), sc_track(3)],
        start: 0,
    });
    wait("playing", || player.snapshot().state == PlayState::Playing);
    wait("warm hint", || !urls.log.lock().expect("log").is_empty());
    std::thread::sleep(Duration::from_millis(300));
    let log = urls.log.lock().expect("log").clone();
    assert!(log.contains(&"warm 2".to_owned()), "{log:?}");
    assert!(
        !log.iter().any(|l| l.ends_with(" 3")),
        "only the entries just ahead: {log:?}"
    );
}
