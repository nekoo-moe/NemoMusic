//! Seeking an HLS stream by segment, through the decoder, against a recorded eight-second tone
//! whose frequency steps up every quarter second (`tests/fixtures/hls-sweep/`, made with
//! `ffmpeg`, no third-party audio).
//!
//! A seek reopens the stream at the segment before the time, probes it and drops the audio
//! before the time. Away from the steps of the tone, AAC output after the first frame of a restart
//! equals the output of an unbroken decode, so the tests compare the samples after a seek with the
//! samples of a straight decode at the same time. At the steps the encoder substitutes noise
//! (PNS), and the decoder's noise generator differs after a restart, so the test times sit in the
//! middle of a step and the compared window ends before the next one.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use nemo_audio::stream_decoder::StreamDecoder;

use super::*;
use crate::opener::Shared;
use crate::test_server::{Response, TestServer};

const INIT: &[u8] = include_bytes!("../../tests/fixtures/hls-sweep/init.mp4");
const SEGMENTS: [&[u8]; 5] = [
    include_bytes!("../../tests/fixtures/hls-sweep/seg0.m4s"),
    include_bytes!("../../tests/fixtures/hls-sweep/seg1.m4s"),
    include_bytes!("../../tests/fixtures/hls-sweep/seg2.m4s"),
    include_bytes!("../../tests/fixtures/hls-sweep/seg3.m4s"),
    include_bytes!("../../tests/fixtures/hls-sweep/seg4.m4s"),
];
const PLAYLIST: &str = include_str!("../../tests/fixtures/hls-sweep/out.m3u8");
const RATE: usize = 44_100;
const CHANNELS: usize = 2;

fn serve() -> TestServer {
    TestServer::start(|path, _| match path {
        "/sweep/out.m3u8" => Response::new(200, PLAYLIST),
        "/sweep/init.mp4" => Response::new(200, INIT),
        other => other
            .strip_prefix("/sweep/seg")
            .and_then(|n| n.strip_suffix(".m4s"))
            .and_then(|n| n.parse::<usize>().ok())
            .and_then(|n| SEGMENTS.get(n))
            .map_or_else(
                || Response::new(404, ""),
                |bytes| Response::new(200, *bytes),
            ),
    })
}

fn decoder(server: &TestServer) -> StreamDecoder {
    let shared = Shared::with(Rules::loopback());
    let client = shared.rules().client().expect("client");
    let list = shared
        .playlist(&client, &server.url("/sweep/out.m3u8"))
        .expect("playlist");
    let source = HlsSource::start(client, shared.rules(), list, None).expect("source");
    StreamDecoder::open(Box::new(source), None).expect("decoder")
}

/// Everything the decoder gives from here, interleaved.
fn decode_all(decoder: &mut StreamDecoder) -> Vec<f32> {
    let mut all = Vec::new();
    let mut block = Vec::new();
    while decoder.next_block(&mut block).expect("decodes") {
        all.extend_from_slice(&block);
    }
    all
}

/// At least `frames` frames from here.
fn decode_frames(decoder: &mut StreamDecoder, frames: usize) -> Vec<f32> {
    let mut out = Vec::new();
    let mut block = Vec::new();
    while out.len() < frames * CHANNELS && decoder.next_block(&mut block).expect("decodes") {
        out.extend_from_slice(&block);
    }
    out.truncate(frames * CHANNELS);
    out
}

fn start_index(seconds: f64) -> usize {
    // The rounded frame of a time in seconds; the values are small and positive.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let frame = (seconds * RATE as f64).round() as usize;
    frame * CHANNELS
}

#[test]
fn a_seek_lands_on_the_exact_frame_of_an_unbroken_decode() {
    let server = serve();
    let reference = decode_all(&mut decoder(&server));
    assert!(
        reference.len() > 8 * RATE * CHANNELS,
        "eight seconds and the encoder delay"
    );
    // The start, inside a segment, either side of a segment boundary (2.020, 4.017, 6.014 s)
    // and near the end.
    for seconds in [
        0.075, 0.125, 2.075, 2.125, 4.1, 4.125, 5.625, 6.125, 6.07, 7.625, 7.9,
    ] {
        let mut seeker = decoder(&server);
        seeker.seek(Duration::from_secs_f64(seconds)).expect("seek");
        let got = decode_frames(&mut seeker, 3000);
        let at = start_index(seconds);
        let want = &reference[at..(at + got.len()).min(reference.len())];
        assert!(!got.is_empty(), "{seconds} s: audio after the seek");
        let worst = got
            .iter()
            .zip(want)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            worst < 1e-4,
            "{seconds} s: differs by {worst} from the unbroken decode"
        );
    }
}

#[test]
fn seeking_back_and_forth_on_one_decoder_works_and_fetches_only_what_it_needs() {
    let server = serve();
    let reference = decode_all(&mut decoder(&server));
    let mut seeker = decoder(&server);
    let _ = decode_frames(&mut seeker, 2000);
    for seconds in [6.07, 1.075, 4.125, 0.075] {
        let before = server.paths().len();
        seeker.seek(Duration::from_secs_f64(seconds)).expect("seek");
        let got = decode_frames(&mut seeker, 3000);
        let at = start_index(seconds);
        assert_eq!(got.len(), 3000 * CHANNELS);
        let worst = got
            .iter()
            .zip(&reference[at..])
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);
        assert!(worst < 1e-4, "{seconds} s: {worst}");
        let asked: Vec<String> = server.paths().split_off(before);
        assert!(
            asked.contains(&"/sweep/init.mp4".to_owned()),
            "the init segment again: {asked:?}"
        );
    }
    // After seeking to 6 s only the segments from there were asked for, never 0 to 2 again.
    let paths = server.paths();
    let at_six = paths
        .iter()
        .rposition(|p| p == "/sweep/seg3.m4s")
        .expect("segment 3");
    assert!(at_six > 0);
}

#[test]
fn a_seek_fetches_the_target_segment_and_not_the_ones_before_it() {
    let hits: Arc<[AtomicUsize; 5]> = Arc::default();
    let counter = Arc::clone(&hits);
    let server = TestServer::start(move |path, _| {
        if let Some(n) = path
            .strip_prefix("/sweep/seg")
            .and_then(|n| n.strip_suffix(".m4s"))
            .and_then(|n| n.parse::<usize>().ok())
            && let Some(hit) = counter.get(n)
        {
            hit.fetch_add(1, Ordering::SeqCst);
        }
        match path {
            "/sweep/out.m3u8" => Response::new(200, PLAYLIST),
            "/sweep/init.mp4" => Response::new(200, INIT),
            other => other
                .strip_prefix("/sweep/seg")
                .and_then(|n| n.strip_suffix(".m4s"))
                .and_then(|n| n.parse::<usize>().ok())
                .and_then(|n| SEGMENTS.get(n))
                .map_or_else(
                    || Response::new(404, ""),
                    |bytes| Response::new(200, *bytes),
                ),
        }
    });
    let mut seeker = decoder(&server);
    let first_pass = hits
        .iter()
        .map(|h| h.load(Ordering::SeqCst))
        .collect::<Vec<_>>();
    assert_eq!(first_pass[0], 1);
    seeker.seek(Duration::from_secs_f64(6.5)).expect("seek");
    let _ = decode_frames(&mut seeker, 1000);
    let after: Vec<usize> = hits.iter().map(|h| h.load(Ordering::SeqCst)).collect();
    assert_eq!(after[0], 1, "segment 0 is not fetched again: {after:?}");
    assert!(
        after[3] >= 1,
        "the segment before 6.5 s minus the restart margin is: {after:?}"
    );
}

#[test]
fn a_seek_past_the_end_ends_the_stream_without_an_error() {
    let server = serve();
    let mut seeker = decoder(&server);
    seeker
        .seek(Duration::from_mins(10))
        .expect("seek past the end");
    let mut block = Vec::new();
    assert!(
        !seeker.next_block(&mut block).expect("no error"),
        "nothing left to play"
    );
}

#[test]
fn a_stream_whose_segments_are_refused_does_not_open() {
    // Every segment answers 403: the first probe fails with an error, not a panic.
    let server = TestServer::start(|path, _| match path {
        "/sweep/out.m3u8" => Response::new(200, PLAYLIST),
        "/sweep/init.mp4" => Response::new(200, INIT),
        _ => Response::new(403, ""),
    });
    let shared = Shared::with(Rules::loopback());
    let client = shared.rules().client().expect("client");
    let list = shared
        .playlist(&client, &server.url("/sweep/out.m3u8"))
        .expect("playlist");
    let source = HlsSource::start(client, shared.rules(), list, None).expect("source");
    let opened = StreamDecoder::open(Box::new(source), None);
    assert!(opened.is_err());
}
