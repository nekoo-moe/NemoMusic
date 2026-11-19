//! Hand-run measurement of click-to-first-audio for SoundCloud (soundcloud-full, task 6).
//!
//! It contacts `soundcloud.com`, `a-v2.sndcdn.com`, `api-v2.soundcloud.com` and the media hosts,
//! about 15 requests in all, with a few seconds between the steps. It prints times in
//! milliseconds and counts only: no address, id, title or token.
//!
//! Run: `cargo run -p nemo-soundcloud --release --example latency`
//!
//! Stages of one play, as the player does them on its prepare thread: `lookup` (the stream
//! address, one API call), `playlist` (the HLS playlist, one media call, and the start of the
//! reader, which fetches the init segment and the first segment at the same time), `probe` (the
//! decoder opens the container, which waits for those first bytes) and `first block` (the first
//! decoded audio). The cold run has nothing fetched ahead; the warm run is the next queue entry
//! after `warm` finished its look-up and prefetch.

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
        Ok((stream.info, (playlist, probe, ms(decoding))))
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(Stages {
        lookup,
        playlist: rest.0,
        probe: rest.1,
        block: rest.2,
        total: ms(click),
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
}

#[tokio::main]
async fn main() {
    let http = ReqwestHttp::new(tokio::runtime::Handle::current()).expect("HTTP client");
    let client = Arc::new(ScClient::new(Arc::new(http)));
    let began = Instant::now();
    if let Err(e) = client.warm_up().await {
        println!("client id: {e}");
        return;
    }
    println!(
        "client id (cold, home page and one script): {} ms",
        ms(began)
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
    let (resolver, opener) = streams(Arc::clone(&client), tokio::runtime::Handle::current());
    // `nemo-app` does this in the background as soon as SoundCloud starts.
    let early = opener.clone();
    let built = Instant::now();
    tokio::task::spawn_blocking(move || early.warm_up())
        .await
        .expect("thread")
        .expect("client");
    println!(
        "HTTP client for the media hosts: {} ms (before any play, no request)",
        ms(built)
    );
    let mut songs = songs.into_iter();
    let (Some(first), Some(second)) = (songs.next(), songs.next()) else {
        println!("search gave fewer than two tracks");
        return;
    };

    tokio::time::sleep(PAUSE).await;
    match play(&resolver, &opener, &first).await {
        Ok(s) => show("cold ", &s),
        Err(e) => println!("cold failed: {e}"),
    }

    // The next entry: `warm` looks the stream up and fetches the playlist and first segment in
    // the background while the current track plays; the user then waits for the track to end.
    tokio::time::sleep(PAUSE).await;
    let page = second.source_url.clone().expect("page");
    resolver.warm(&page);
    tokio::time::sleep(WARM_WAIT).await;
    tokio::time::sleep(PAUSE).await;
    match play(&resolver, &opener, &second).await {
        Ok(s) => show("warm ", &s),
        Err(e) => println!("warm failed: {e}"),
    }
}
