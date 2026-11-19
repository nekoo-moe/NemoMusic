//! Hand-run measurement of click-to-first-audio for SoundCloud (soundcloud-full, tasks 6 and 13).
//!
//! It contacts `soundcloud.com`, `a-v2.sndcdn.com`, `api-v2.soundcloud.com` and the media hosts,
//! about 12 to 16 requests per run, with a few seconds between the steps. It prints times in
//! milliseconds and counts only: no address, id, title or token.
//!
//! Run: `cargo run -p nemo-soundcloud --example latency -- cold` (the default) or `-- warm`.
//!
//! Stages of one play, as the player does them on its prepare thread: `lookup` (the stream
//! address, one API call), `playlist` (the HLS playlist, one media call, and the start of the
//! reader, which fetches the init segment and the first segment at the same time), `probe` (the
//! decoder opens the container, which waits for those first bytes) and `first block` (the first
//! decoded audio).
//!
//! - `cold`: the first play after a search, with nothing fetched ahead except the client id, the
//!   media client and the connection to the media host (as `nemo-app` does at start).
//! - `warm`: the next queue entry after `warm` finished its look-up and prefetch, then a seek 30 s
//!   into it (a new stream from the segment that holds the time) and a seek back to the start.

use std::sync::Arc;
use std::time::{Duration, Instant};

use nemo_audio::decoder::supported_stream_codecs;
use nemo_audio::stream_decoder::StreamDecoder;
use nemo_core::catalog::{Catalog, SearchKind};
use nemo_core::services::StreamOpener;
use nemo_core::stream::Transport;
use nemo_core::track::Track;
use nemo_core::url_resolver::{FormatPolicy, UrlResolver};
use nemo_soundcloud::{ReqwestHttp, ScClient, SoundcloudCatalog, streams};

const PAUSE: Duration = Duration::from_secs(3);
/// Time given to `warm` before the next entry starts.
const WARM_WAIT: Duration = Duration::from_secs(5);

fn ms(since: Instant) -> u128 {
    since.elapsed().as_millis()
}

struct Stages {
    seeks: Vec<(u64, u128)>,
    lookup: u128,
    playlist: u128,
    probe: u128,
    block: u128,
    total: u128,
    codec: String,
    bitrate: Option<u32>,
    hls: bool,
}

/// One play of `track`, timed stage by stage, on a blocking thread as the player does.
async fn play(
    resolver: &nemo_soundcloud::ScStreamResolver,
    opener: &nemo_soundcloud::ScStreamOpener,
    track: &Track,
    seeks: &[u64],
) -> Result<Stages, String> {
    let page = track.source_url.clone().ok_or("track has no page")?;
    let policy = FormatPolicy {
        playable: supported_stream_codecs().to_vec(),
        allow_hls: true,
    };
    let click = Instant::now();
    let stream = resolver
        .resolve(&page, &policy)
        .await
        .map_err(|e| e.to_string())?;
    let lookup = ms(click);
    let opener = opener.clone();
    let seeks = seeks.to_vec();
    let hls = matches!(stream.transport, Transport::Hls { .. });
    let (info, rest) = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let opening = Instant::now();
        let source = opener.open(&stream).map_err(|e| e.to_string())?;
        let playlist = ms(opening);
        let probing = Instant::now();
        let mut decoder = StreamDecoder::open(source, None).map_err(|e| e.to_string())?;
        let probe = ms(probing);
        let decoding = Instant::now();
        let mut block = Vec::new();
        decoder.next_block(&mut block).map_err(|e| e.to_string())?;
        let first = ms(decoding);
        let total = ms(click);
        let mut seeked = Vec::new();
        for seconds in seeks {
            let began = Instant::now();
            decoder
                .seek(Duration::from_secs(seconds))
                .map_err(|e| e.to_string())?;
            decoder.next_block(&mut block).map_err(|e| e.to_string())?;
            seeked.push((seconds, ms(began)));
        }
        Ok((stream.info, (playlist, probe, first, total, seeked)))
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(Stages {
        seeks: rest.4,
        lookup,
        playlist: rest.0,
        probe: rest.1,
        block: rest.2,
        total: rest.3,
        codec: format!("{:?}", info.codec),
        bitrate: info.bitrate_kbps,
        hls,
    })
}

fn show(label: &str, s: &Stages) {
    println!(
        "{label}: lookup {} ms | playlist {} ms | probe {} ms | first block {} ms | click to audio {} ms ({}, {:?} kbps, {})",
        s.lookup,
        s.playlist,
        s.probe,
        s.block,
        s.total,
        s.codec,
        s.bitrate,
        if s.hls { "HLS" } else { "file" }
    );
    for (to, took) in &s.seeks {
        println!("{label}: seek to {to} s, first audio after {took} ms");
    }
}

#[tokio::main]
async fn main() {
    let warm_mode = std::env::args().nth(1).as_deref() == Some("warm");
    let handle = tokio::runtime::Handle::current();
    let http = ReqwestHttp::new(handle.clone()).expect("HTTP client");
    let client = Arc::new(ScClient::new(Arc::new(http)));
    let (resolver, opener) = streams(Arc::clone(&client), handle);
    // What `nemo-app` does as soon as SoundCloud starts: the client id first, then (only when it
    // is known) the media client and the connection to the media host.
    let began = Instant::now();
    if let Err(e) = client.warm_up().await {
        println!("client id: {e}");
        return;
    }
    println!(
        "client id (cold, home page and one script): {} ms",
        ms(began)
    );
    let early = opener.clone();
    let built = Instant::now();
    tokio::task::spawn_blocking(move || {
        let _ = early.warm_up();
        early.preconnect();
    })
    .await
    .expect("thread");
    println!(
        "media client and connection opened ahead: {} ms (before any play)",
        ms(built)
    );

    let catalog = SoundcloudCatalog::new(Arc::clone(&client));
    let songs = match catalog
        .search_kind("ambient piano", SearchKind::Songs)
        .await
    {
        Ok(found) => found.songs,
        Err(e) => {
            println!("search: {e}");
            return;
        }
    };
    println!("search: {} songs", songs.len());
    let Some(track) = songs.into_iter().next() else {
        println!("search gave no track");
        return;
    };

    if warm_mode {
        // The next entry: `warm` looks the stream up and fetches the playlist and first segment
        // in the background while the current track plays.
        let page = track.source_url.clone().expect("page");
        resolver.warm(&page);
        tokio::time::sleep(WARM_WAIT).await;
        match play(&resolver, &opener, &track, &[30, 0]).await {
            Ok(s) => show("warm ", &s),
            Err(e) => println!("warm failed: {e}"),
        }
    } else {
        // The listener reads the results for a moment, then clicks.
        tokio::time::sleep(PAUSE).await;
        match play(&resolver, &opener, &track, &[]).await {
            Ok(s) => show("cold ", &s),
            Err(e) => println!("cold failed: {e}"),
        }
    }
}
