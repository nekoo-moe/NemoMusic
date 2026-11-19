//! Stage times from "resolve" to "200 ms of audio decoded" (development only, real network, no
//! audio device needed).
//!
//! `NEMO_PROBE_IDS=<id1>,<id2>,<id3> cargo run -p nemo-ytm --example fast_start_stages -- <mode>`
//!
//! Modes (each song is one resolve, so a run is a handful of YouTube requests; the pause between
//! songs is [`PAUSE`]; run it a few times at most):
//! - `native`: the native innertube path only, one shared client (the first song pays the
//!   TLS set-up, like the first click after start-up).
//! - `ytdlp`: yt-dlp only (`NEMO_YTDLP=<program>` picks the executable; default: the located one).
//! - `chain`: the app's chain (cache, native, yt-dlp): songs 1 and 3 cold, song 1 again (a
//!   replay), then a simulated queue: song 2 is warmed while "song 1 plays" and then started.
//!
//! Each song prints: resolve, open (connect and the first window), probe (container header),
//! prefill (decode until 200 ms of audio). The audio device open and first callback are not
//! measured here (no device on a build machine). It never prints a URL or an id.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "an example reports what it measured"
)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use nemo_audio::pipeline::PREFILL_MS;
use nemo_audio::stream_decoder::StreamDecoder;
use nemo_core::services::StreamOpener;
use nemo_core::stream::StreamResolver;
use nemo_core::track::VideoId;
use nemo_ytm::innertube::InnertubeResolver;
use nemo_ytm::resolver::{NATIVE_PROFILES, default_chain};
use nemo_ytm::stream::HttpStreamOpener;
use nemo_ytm::ytdlp::{YtDlpResolver, locate_yt_dlp};

/// Pause between songs, so the requests stay human-paced.
const PAUSE: Duration = Duration::from_secs(4);

fn ms(since: Instant) -> u128 {
    since.elapsed().as_millis()
}

/// Open the resolved stream, probe it and decode `PREFILL_MS` of audio; print each stage.
fn after_resolve(
    label: &str,
    resolve_ms: u128,
    stream: &nemo_core::stream::ResolvedStream,
    opener: HttpStreamOpener,
) {
    let open = Instant::now();
    let source = match opener.open(stream) {
        Ok(s) => s,
        Err(e) => {
            println!("{label}: resolve {resolve_ms} ms, open FAILED ({e})");
            return;
        }
    };
    let open_ms = ms(open);
    let probe = Instant::now();
    let mut decoder = match StreamDecoder::open(source, None) {
        Ok(d) => d,
        Err(e) => {
            println!("{label}: resolve {resolve_ms} ms, open {open_ms} ms, probe FAILED ({e})");
            return;
        }
    };
    let probe_ms = ms(probe);
    let rate = u64::from(decoder.info().format.sample_rate);
    let channels = u64::from(decoder.info().format.channels);
    let want = rate * channels * PREFILL_MS / 1000;
    let fill = Instant::now();
    let (mut got, mut first_ms, mut block) = (0_u64, None, Vec::new());
    while got < want {
        if !decoder.next_block(&mut block).unwrap_or(false) {
            break;
        }
        first_ms.get_or_insert_with(|| ms(fill));
        got += block.len() as u64;
    }
    println!(
        "{label}: resolve {resolve_ms} | open {open_ms} | probe {probe_ms} | first packet {} | \
         {PREFILL_MS} ms of audio {} | total {} ms ({:?} {:?} kbps)",
        first_ms.unwrap_or(0),
        ms(fill),
        resolve_ms + open_ms + probe_ms + ms(fill),
        stream.info.codec,
        stream.info.bitrate_kbps,
    );
}

fn timed_resolve(
    runtime: &tokio::runtime::Runtime,
    resolver: &dyn StreamResolver,
    video: &VideoId,
) -> Option<(u128, nemo_core::stream::ResolvedStream)> {
    let start = Instant::now();
    match runtime.block_on(resolver.resolve(video)) {
        Ok(stream) => Some((ms(start), stream)),
        Err(e) => {
            println!("resolve FAILED after {} ms ({e})", ms(start));
            None
        }
    }
}

/// The songs to measure: `NEMO_PROBE_IDS`, or the first three songs of one anonymous search
/// (`NEMO_PROBE_QUERY`, one request), so no id is stored in the repository.
fn probe_ids(runtime: &tokio::runtime::Runtime) -> Vec<VideoId> {
    let listed: Vec<VideoId> = std::env::var("NEMO_PROBE_IDS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| VideoId::parse(s).ok())
        .collect();
    let Ok(query) = std::env::var("NEMO_PROBE_QUERY") else {
        return listed;
    };
    if !listed.is_empty() {
        return listed;
    }
    let catalog = nemo_ytm::catalog::YtmCatalog::new(runtime.handle().clone());
    let found = runtime
        .block_on(nemo_core::catalog::Catalog::search(&catalog, &query))
        .map(|r| {
            r.songs
                .into_iter()
                .filter_map(|t| t.video_id)
                .take(3)
                .collect()
        });
    std::thread::sleep(Duration::from_secs(2));
    found.unwrap_or_default()
}

fn main() {
    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "chain".to_owned());
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    else {
        return;
    };
    let ids = probe_ids(&runtime);
    if ids.is_empty() {
        eprintln!("set NEMO_PROBE_IDS (comma-separated video ids) or NEMO_PROBE_QUERY (a search)");
        return;
    }
    let codecs = nemo_audio::decoder::supported_stream_codecs();
    let program = std::env::var_os("NEMO_YTDLP")
        .map(std::path::PathBuf::from)
        .or_else(locate_yt_dlp);
    let ytdlp = program
        .map_or_else(YtDlpResolver::new, YtDlpResolver::with_program)
        .with_allowed_codecs(codecs)
        .with_handle(runtime.handle().clone());
    let resolver: Arc<dyn StreamResolver> = match mode.as_str() {
        "native" => match InnertubeResolver::new(runtime.handle().clone(), NATIVE_PROFILES) {
            Ok(r) => Arc::new(r.with_allowed_codecs(codecs)),
            Err(e) => {
                eprintln!("native client: {e}");
                return;
            }
        },
        "ytdlp" => Arc::new(ytdlp),
        _ => default_chain(ytdlp, codecs, runtime.handle()),
    };
    let opener = HttpStreamOpener::new();
    println!("mode={mode}");
    if mode == "describe" {
        // Why the native path refuses: playability status and format counts, no URL, no id.
        if let Ok(native) = InnertubeResolver::new(runtime.handle().clone(), NATIVE_PROFILES) {
            for (n, video) in ids.iter().take(2).enumerate() {
                for line in runtime.block_on(native.describe(video)) {
                    println!("song {}: {line}", n + 1);
                }
                std::thread::sleep(PAUSE);
            }
        }
        return;
    }
    for (n, video) in ids.iter().enumerate() {
        // In chain mode song 2 stays untouched until it is warmed, below.
        if mode == "chain" && n == 1 {
            continue;
        }
        if let Some((resolve_ms, stream)) = timed_resolve(&runtime, resolver.as_ref(), video) {
            after_resolve(&format!("song {}", n + 1), resolve_ms, &stream, opener);
        }
        std::thread::sleep(PAUSE);
    }
    if mode != "chain" {
        return;
    }
    if let Some((resolve_ms, stream)) = timed_resolve(&runtime, resolver.as_ref(), &ids[0]) {
        after_resolve("song 1 again (replay)", resolve_ms, &stream, opener);
    }
    if let [_, second, ..] = ids.as_slice() {
        // "Song 1 plays": warm song 2, wait as a listener would, then start it.
        resolver.warm(second);
        std::thread::sleep(PAUSE + Duration::from_secs(4));
        if let Some((resolve_ms, stream)) = timed_resolve(&runtime, resolver.as_ref(), second) {
            after_resolve("song 2 after warm", resolve_ms, &stream, opener);
        }
    }
}
