//! Click-to-first-audio timing (development only, real network, real output device).
//!
//! `NEMO_PROBE_IDS=<id1>,<id2>,<id3> cargo run --release -p nemo-ytm --example click_to_audio -- <mode>`
//!
//! Modes: `old` (the located `yt-dlp` with the old arguments only), `ytdlp` (yt-dlp, fast
//! arguments then the old ones), `auto` (the app's resolver chain: cache, native client,
//! `yt-dlp`), `probe` (first id only: each native client and yt-dlp, resolve + open, no
//! playback). Play modes play each id once, then the first id again to show the cached path. It prints stage timings and the resolver path only: never a URL, and no ids.
//! The ids come from the environment so none is stored in the repository. Needs network: keep
//! it to a handful of runs (each song costs about three YouTube requests).
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "an example reports what it measured"
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use nemo_audio::player::{Player, PlayerDeps};
use nemo_audio::sink::CpalSinkFactory;
use nemo_core::BoxFuture;
use nemo_core::audio::{Origin, StreamInfo};
use nemo_core::player::{PlayState, PlayerCommand};
use nemo_core::services::{ByteSource, StreamOpener};
use nemo_core::stream::{ResolvedStream, StreamError, StreamResolver};
use nemo_core::track::{Artwork, Track, TrackId, VideoId};
use nemo_ytm::innertube::{ANDROID_VR, InnertubeResolver, VISIONOS};
use nemo_ytm::stream::HttpStreamOpener;
use nemo_ytm::ytdlp::{YtDlpResolver, locate_yt_dlp};

/// Adds up the time spent in `resolve` and `open`.
struct Clock {
    resolve_us: AtomicU64,
    open_us: AtomicU64,
}

struct TimedResolver(Arc<dyn StreamResolver>, Arc<Clock>);

impl StreamResolver for TimedResolver {
    fn resolve<'a>(
        &'a self,
        video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        Box::pin(async move {
            let start = Instant::now();
            let result = self.0.resolve(video).await;
            let us = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX);
            self.1.resolve_us.store(us, Ordering::SeqCst);
            match &result {
                Ok(s) => eprintln!("  resolve ok in {} ms ({:?})", us / 1000, s.info.codec),
                Err(e) => eprintln!("  resolve error in {} ms: {e}", us / 1000),
            }
            result
        })
    }

    fn warm(&self, video: &VideoId) {
        self.0.warm(video);
    }

    fn stream_failed(&self, video: &VideoId) {
        self.0.stream_failed(video);
    }
}

struct TimedOpener(HttpStreamOpener, Arc<Clock>);

impl StreamOpener for TimedOpener {
    fn open(&self, stream: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        let start = Instant::now();
        let result = self.0.open(stream);
        let us = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX);
        self.1.open_us.store(us, Ordering::SeqCst);
        if let Err(e) = &result {
            eprintln!("  open error in {} ms: {e}", us / 1000);
        }
        result
    }
}

fn track(id: &str) -> Option<Track> {
    let video = VideoId::parse(id).ok()?;
    Some(Track {
        id: TrackId(id.to_owned()),
        title: "probe".to_owned(),
        artists: Vec::new(),
        album: None,
        duration: None,
        origin: Origin::YoutubeMusic,
        video_id: Some(video),
        local_path: None,
        artwork: Artwork::None,
    })
}

fn ms(us: u64) -> u64 {
    us / 1000
}

/// One video: each native client, then `yt-dlp` with the fast arguments; resolve and open.
fn probe(runtime: &tokio::runtime::Runtime, id: &str, ytdlp: &YtDlpResolver) {
    let Ok(video) = VideoId::parse(id) else {
        eprintln!("bad id");
        return;
    };
    let codecs = nemo_audio::decoder::supported_stream_codecs();
    let opener = HttpStreamOpener::new();
    let try_one = |label: &str, resolver: &dyn StreamResolver| {
        let start = Instant::now();
        let resolved = runtime.block_on(resolver.resolve(&video));
        let resolve_ms = start.elapsed().as_millis();
        match resolved {
            Ok(stream) => {
                let open = Instant::now();
                let result = opener.open(&stream).map(|_| ());
                println!(
                    "{label}: resolve {resolve_ms} ms ({:?} {:?} kbps), open {} ms: {}",
                    stream.info.codec,
                    stream.info.bitrate_kbps,
                    open.elapsed().as_millis(),
                    result.map_or_else(|e| format!("error ({e})"), |()| "ok".to_owned())
                );
            }
            Err(e) => println!("{label}: resolve {resolve_ms} ms: error ({e})"),
        }
        std::thread::sleep(Duration::from_secs(1));
    };
    for profile in [ANDROID_VR, VISIONOS] {
        match InnertubeResolver::new(runtime.handle().clone(), &[profile]) {
            Ok(r) => try_one(profile.label, &r.with_allowed_codecs(codecs)),
            Err(e) => println!("{}: {e}", profile.label),
        }
    }
    try_one("yt-dlp fast args", ytdlp);
}

#[allow(clippy::too_many_lines, reason = "a linear measurement script")]
fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "auto".to_owned());
    let ids: Vec<String> = std::env::var("NEMO_PROBE_IDS")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    if ids.is_empty() {
        eprintln!("set NEMO_PROBE_IDS to comma-separated video ids");
        return;
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    let program = locate_yt_dlp();
    println!("mode={mode} yt-dlp-located={}", program.is_some());
    let ytdlp = program
        .clone()
        .map_or_else(YtDlpResolver::new, YtDlpResolver::with_program)
        .with_fast_args(mode != "old")
        .with_handle(runtime.handle().clone());
    let codecs = nemo_audio::decoder::supported_stream_codecs();
    if mode == "describe" {
        let native = InnertubeResolver::new(runtime.handle().clone(), &[VISIONOS, ANDROID_VR])
            .expect("client");
        for (n, id) in ids.iter().enumerate() {
            let Ok(video) = VideoId::parse(id) else {
                continue;
            };
            for line in runtime.block_on(native.describe(&video)) {
                println!("song {}: {line}", n + 1);
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        return;
    }
    if mode == "probe" {
        probe(&runtime, &ids[0], &ytdlp.with_allowed_codecs(codecs));
        return;
    }
    let ytdlp = ytdlp.with_allowed_codecs(codecs);
    let inner: Arc<dyn StreamResolver> = if mode == "auto" {
        nemo_ytm::resolver::default_chain(ytdlp, codecs, runtime.handle())
    } else {
        Arc::new(ytdlp)
    };
    let clock = Arc::new(Clock {
        resolve_us: AtomicU64::new(0),
        open_us: AtomicU64::new(0),
    });
    let player = Player::spawn(PlayerDeps {
        resolver: Arc::new(TimedResolver(inner, Arc::clone(&clock))),
        opener: Arc::new(TimedOpener(HttpStreamOpener::new(), Arc::clone(&clock))),
        segments: None,
        sink_factory: Arc::new(CpalSinkFactory::new(None)),
    });
    let mut events = player.subscribe();
    let mut plan: Vec<String> = ids.clone();
    plan.push(ids[0].clone());
    for (n, id) in plan.iter().enumerate() {
        let Some(t) = track(id) else {
            eprintln!("bad id");
            continue;
        };
        clock.resolve_us.store(0, Ordering::SeqCst);
        clock.open_us.store(0, Ordering::SeqCst);
        let start = Instant::now();
        player.send(PlayerCommand::Play(t));
        let outcome = loop {
            let snap = player.snapshot();
            if snap.state == PlayState::Playing {
                break Ok(start.elapsed());
            }
            if snap.state == PlayState::Stopped && start.elapsed() > Duration::from_millis(50) {
                break Err("stopped (failed)");
            }
            if start.elapsed() > Duration::from_secs(40) {
                break Err("timeout");
            }
            std::thread::sleep(Duration::from_millis(2));
        };
        let codec = player
            .snapshot()
            .signal_path
            .map(|p: nemo_core::audio::SignalPath| {
                let StreamInfo {
                    codec,
                    bitrate_kbps,
                    ..
                } = p.source;
                format!("{codec:?} {bitrate_kbps:?} kbps")
            });
        match outcome {
            Ok(total) => println!(
                "song {}{}: click->playing {} ms (resolve {} ms, open+first window {} ms, rest {} ms) {}",
                n + 1,
                if n == plan.len() - 1 {
                    " (repeat, cached)"
                } else {
                    ""
                },
                total.as_millis(),
                ms(clock.resolve_us.load(Ordering::SeqCst)),
                ms(clock.open_us.load(Ordering::SeqCst)),
                total
                    .as_millis()
                    .saturating_sub(u128::from(ms(clock.resolve_us.load(Ordering::SeqCst)
                        + clock.open_us.load(Ordering::SeqCst)))),
                codec.unwrap_or_default()
            ),
            Err(why) => {
                let mut reasons = Vec::new();
                while let Some(e) = events.try_next() {
                    if let nemo_core::player::PlayerEvent::Failed { message } = e {
                        reasons.push(message);
                    }
                }
                println!(
                    "song {}: {why} after {} ms (resolve {} ms) {reasons:?}",
                    n + 1,
                    start.elapsed().as_millis(),
                    ms(clock.resolve_us.load(Ordering::SeqCst))
                );
            }
        }
        while events.try_next().is_some() {}
        player.send(PlayerCommand::Stop);
        std::thread::sleep(Duration::from_millis(1200));
    }
}
