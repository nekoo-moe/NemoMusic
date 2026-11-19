//! The in-process tier on the real network (development only): prepare the player, ask the `tv`
//! client, solve in process and open the address.
//!
//! `NEMO_PROBE_IDS=<id>[,<id>] NEMO_PLAYER_FILE=<file> cargo run -p nemo-ytm --example cipher_probe`
//!
//! About four requests for the first song (`iframe_api`, the player script of about 2.7 MB, the
//! `player` request, the first window of the stream), one `player` request and one window for
//! each song after it, five seconds apart. It prints times and outcomes, never an address or an
//! id. Run it twice: the second run finds the player in its file and downloads nothing.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "an example reports what it measured"
)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use nemo_core::services::StreamOpener;
use nemo_core::stream::StreamResolver;
use nemo_core::track::VideoId;
use nemo_ytm::cipher::{CipherResolver, PlayerRequest, TvRequest};
use nemo_ytm::innertube::describe_player;
use nemo_ytm::stream::HttpStreamOpener;

fn main() {
    let ids: Vec<VideoId> = std::env::var("NEMO_PROBE_IDS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| VideoId::parse(s).ok())
        .collect();
    if ids.is_empty() {
        eprintln!("set NEMO_PROBE_IDS (comma-separated video ids)");
        return;
    }
    let file = std::env::var_os("NEMO_PLAYER_FILE").map(PathBuf::from);
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    else {
        return;
    };
    let codecs = nemo_audio::decoder::supported_stream_codecs();
    let Ok(cipher) = CipherResolver::new(runtime.handle(), file, None, codecs) else {
        eprintln!("the resolver could not start");
        return;
    };
    let began = Instant::now();
    match runtime.block_on(cipher.store().get()) {
        Ok(player) => println!(
            "player ready in {} ms (timestamp {}, {} kB preprocessed)",
            began.elapsed().as_millis(),
            player.sts,
            player.preprocessed.len() / 1000
        ),
        Err(e) => {
            println!(
                "player FAILED after {} ms: {e}",
                began.elapsed().as_millis()
            );
            return;
        }
    }
    let opener = HttpStreamOpener::new();
    for (n, video) in ids.iter().enumerate() {
        if n > 0 {
            std::thread::sleep(Duration::from_secs(5));
        }
        let began = Instant::now();
        let stream = match runtime.block_on(cipher.resolve(video)) {
            Ok(stream) => stream,
            Err(e) => {
                println!(
                    "song {}: resolve FAILED after {} ms: {e}",
                    n + 1,
                    began.elapsed().as_millis()
                );
                // Why: what the `tv` client answered (status, reason, how many formats).
                if let (Ok(player), Ok(tv)) =
                    (runtime.block_on(cipher.store().get()), TvRequest::new())
                {
                    match runtime.block_on(tv.ask(video, player.sts)) {
                        Ok(text) => println!("  answer: {}", describe_player(&text)),
                        Err(e) => println!("  request failed: {e}"),
                    }
                }
                continue;
            }
        };
        let resolved = began.elapsed().as_millis();
        let opening = Instant::now();
        let window = opener.open(&stream);
        println!(
            "song {}: resolve {resolved} ms ({:?}, {:?} kbps), open {} ms: {}",
            n + 1,
            stream.info.codec,
            stream.info.bitrate_kbps,
            opening.elapsed().as_millis(),
            window.map_or_else(|e| format!("FAILED ({e})"), |_| "ok".to_owned()),
        );
    }
}
