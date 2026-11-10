//! Real-network smoke test (development only). Run by hand, at most twice:
//! `cargo test -p nemo-ytm --test smoke -- --ignored --nocapture`
//!
//! Search a generic query anonymously, resolve the first song with `yt-dlp`, open it with the
//! HTTP stream opener, read the head and a seek to the tail, and check the container magic bytes.
//! It prints format facts for the report. It never prints a URL or a video id.
#![allow(
    clippy::print_stderr,
    reason = "a smoke test reports what it observed on the real network"
)]

use std::io::{Read, Seek, SeekFrom};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use nemo_core::catalog::Catalog;
use nemo_core::services::StreamOpener;
use nemo_core::track::VideoId;
use nemo_ytm::catalog::YtmCatalog;
use nemo_ytm::stream::HttpStreamOpener;
use nemo_ytm::ytdlp::{args, locate_yt_dlp, parse};
use serde_json::Value;

const HEAD_BYTES: usize = 64 * 1024;
const TAIL_BYTES: usize = 4096;

fn describe(entry: &Value) -> String {
    let text = |key: &str| {
        entry
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_owned()
    };
    let number = |key: &str| {
        entry
            .get(key)
            .and_then(Value::as_f64)
            .map_or_else(|| "?".to_owned(), |v| format!("{v:.0}"))
    };
    format!(
        "id={} ext={} acodec={} abr={} asr={} container={}",
        text("format_id"),
        text("ext"),
        text("acodec"),
        number("abr"),
        number("asr"),
        text("container")
    )
}

fn container_of(head: &[u8]) -> &'static str {
    if head.len() >= 8 && &head[4..8] == b"ftyp" {
        "mp4"
    } else if head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        "webm"
    } else {
        "unknown"
    }
}

#[test]
#[ignore = "needs network access and yt-dlp; run by hand at most twice"]
fn search_resolve_open_and_read() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");

    // 1. Anonymous search with ytmapi-rs.
    let catalog = YtmCatalog::new(rt.handle().clone());
    let results = rt
        .block_on(catalog.search("piano"))
        .expect("ytmapi-rs anonymous search works");
    eprintln!(
        "search: songs={} albums={} artists={} playlists={}",
        results.songs.len(),
        results.albums.len(),
        results.artists.len(),
        results.playlists.len()
    );
    let song = results.songs.first().expect("at least one song");
    let video: &VideoId = song.video_id.as_ref().expect("song has a video id");
    eprintln!("first song: duration={:?}", song.duration);

    // 2. One yt-dlp run (blocking, own process) for the report and for the stream.
    let program = locate_yt_dlp().expect("yt-dlp is installed");
    let started = Instant::now();
    let output = Command::new(program)
        .args(args(video))
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .expect("yt-dlp runs");
    eprintln!(
        "yt-dlp: exit ok={} took={:?}",
        output.status.success(),
        started.elapsed()
    );
    assert!(output.status.success());
    let json = String::from_utf8(output.stdout).expect("utf-8");
    let root: Value = serde_json::from_str(&json).expect("json");
    eprintln!("selected by -f bestaudio: {}", describe(&root));
    if let Some(formats) = root.get("formats").and_then(Value::as_array) {
        for f in formats
            .iter()
            .filter(|f| f.get("vcodec").and_then(Value::as_str) == Some("none"))
        {
            eprintln!("  audio-only format: {}", describe(f));
        }
    }
    let resolved = parse(&json).expect("parse picks a stream");
    eprintln!(
        "parse picked: codec={:?} bitrate={:?} format={:?}",
        resolved.info.codec, resolved.info.bitrate_kbps, resolved.info.format
    );
    match resolved
        .expires_at
        .map(|t| t.duration_since(SystemTime::now()))
    {
        Some(Ok(left)) => eprintln!("url expires in {:.1} h", left.as_secs_f64() / 3600.0),
        Some(Err(_)) => eprintln!("url already expired?"),
        None => eprintln!("no expire parameter"),
    }

    // 3. Open and read through HttpStreamOpener (ranged requests).
    let opener = HttpStreamOpener::new();
    let mut source = opener.open(&resolved).expect("opens");
    let len = source.byte_len().expect("length is known");
    eprintln!("byte_len={len}");
    let mut head = vec![0_u8; HEAD_BYTES];
    source.read_exact(&mut head).expect("head");
    let container = container_of(&head);
    eprintln!("container by magic bytes: {container}");
    assert_ne!(container, "unknown", "unexpected container magic");
    let tail_start = source
        .seek(SeekFrom::End(-i64::try_from(TAIL_BYTES).expect("small")))
        .expect("seek to tail");
    assert_eq!(tail_start, len - TAIL_BYTES as u64);
    let mut tail = vec![0_u8; TAIL_BYTES];
    source.read_exact(&mut tail).expect("tail");
    assert!(tail.iter().any(|b| *b != 0), "tail is all zero");
    assert_eq!(source.read(&mut [0_u8; 8]).expect("eof read"), 0);
    eprintln!("head, seek to tail, tail, EOF: ok");

    // 4. Is a Range header needed? One plain GET, only the first 64 KiB are read.
    let plain = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("client")
        .get(resolved.url.expose())
        .send();
    match plain {
        Ok(mut response) => {
            let status = response.status().as_u16();
            let announced = response.content_length();
            let mut bytes = vec![0_u8; HEAD_BYTES];
            let read_ok = response.read_exact(&mut bytes).is_ok();
            eprintln!(
                "plain GET without Range: status={status} content_length_matches={} first64k_ok={read_ok}",
                announced == Some(len)
            );
        }
        Err(_) => eprintln!("plain GET without Range: request failed"),
    }
}
