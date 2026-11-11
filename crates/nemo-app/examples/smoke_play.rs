//! End-to-end audio check without the UI: search YouTube Music, resolve with `yt-dlp`, stream
//! over HTTPS, decode and play on the default output device for a few seconds.
//!
//! ```text
//! cargo run -p nemo-app --example smoke_play --release -- "search words" [seconds]
//! ```
//!
//! Prints the badge, the signal path, the state and the underrun count. It makes about 6 requests
//! to YouTube hosts (one search, one `yt-dlp` run, the stream). Development use only.

use std::sync::Arc;
use std::time::{Duration, Instant};

use nemo_audio::decoder::supported_stream_codecs;
use nemo_audio::player::{Player, PlayerDeps};
use nemo_audio::sink::CpalSinkFactory;
use nemo_core::catalog::Catalog;
use nemo_core::player::{PlayState, PlayerCommand};
use nemo_ytm::catalog::YtmCatalog;
use nemo_ytm::stream::HttpStreamOpener;
use nemo_ytm::ytdlp::{YtDlpResolver, locate_yt_dlp};

fn main() {
    let mut args = std::env::args().skip(1);
    let query = args.next().unwrap_or_else(|| "lofi piano".to_owned());
    let seconds: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(15);

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => return eprintln!("tokio: {e}"),
    };
    let handle = runtime.handle().clone();

    let catalog = YtmCatalog::new(handle.clone());
    let results = match runtime.block_on(catalog.search(&query)) {
        Ok(r) => r,
        Err(e) => return eprintln!("search failed: {e}"),
    };
    let Some(track) = results.songs.into_iter().find(|t| t.video_id.is_some()) else {
        return eprintln!("no playable song for {query:?}");
    };
    println!("track: {} - {}", track.artists.join(", "), track.title);

    let Some(program) = locate_yt_dlp() else {
        return eprintln!("yt-dlp not found; install it with winget install yt-dlp.yt-dlp");
    };
    let resolver = YtDlpResolver::with_program(program)
        .with_allowed_codecs(supported_stream_codecs())
        .with_handle(handle);
    let player = Player::spawn(PlayerDeps {
        resolver: Arc::new(resolver),
        opener: Arc::new(HttpStreamOpener::default()),
        segments: None,
        sink_factory: Arc::new(CpalSinkFactory::new(None)),
    });

    player.send(PlayerCommand::Play(track));
    let start = Instant::now();
    let mut started_at = None;
    while start.elapsed() < Duration::from_secs(seconds + 20) {
        std::thread::sleep(Duration::from_millis(500));
        let s = player.snapshot();
        if s.state == PlayState::Playing && started_at.is_none() {
            started_at = Some(start.elapsed());
            if let Some(path) = &s.signal_path {
                println!("badge: {}", path.source.badge());
                println!("signal path: {path:?}");
                println!("bit-perfect: {}", path.is_bit_perfect());
            }
        }
        println!(
            "t={:>5.1}s state={:?} pos={:>5.1}s underruns={}",
            start.elapsed().as_secs_f32(),
            s.state,
            player.position().as_secs_f32(),
            s.underruns
        );
        if let Some(t0) = started_at
            && start.elapsed() >= t0 + Duration::from_secs(seconds)
        {
            break;
        }
    }
    let s = player.snapshot();
    println!(
        "result: started_after={:?} final_state={:?} position={:.1}s underruns={}",
        started_at,
        s.state,
        player.position().as_secs_f32(),
        s.underruns
    );
    player.send(PlayerCommand::Stop);
    std::thread::sleep(Duration::from_millis(300));
}
