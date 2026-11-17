//! Tests carrying open decoders out of a stopped pipeline (`Pipeline::stop_and_take`).

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use common::fixtures::wav_bytes;
use nemo_audio::pipeline::{FeedCmd, Job, Pipeline};
use nemo_audio::render::UNITY_GAIN_BITS;
use nemo_audio::sink::CaptureSink;
use nemo_audio::source::MemorySource;
use nemo_audio::stream_decoder::StreamDecoder;
use nemo_core::audio::AudioFormat;

#[test]
fn stopped_pipeline_returns_active_and_queued_jobs_with_decoder_position() {
    let rate = 44_100;
    let channels = 2;
    let format = AudioFormat {
        sample_rate: rate,
        channels,
        bit_depth: Some(16),
    };

    let frames = (rate * 5) as usize;
    let samples: Vec<i32> = (0..frames * usize::from(channels))
        .map(|i| {
            let offset = i32::try_from(i % 1000).unwrap_or(0);
            offset * 20
        })
        .collect();
    let wav_data1 = wav_bytes(&samples, 16, rate, channels);
    let wav_data2 = wav_bytes(&samples, 16, rate, channels);

    let decoder1 = StreamDecoder::open(Box::new(MemorySource::new(wav_data1)), Some("wav"))
        .expect("open wav 1");
    let decoder2 = StreamDecoder::open(Box::new(MemorySource::new(wav_data2)), Some("wav"))
        .expect("open wav 2");

    let played = Arc::new(AtomicU64::new(0));
    let gain = Arc::new(AtomicU32::new(UNITY_GAIN_BITS));
    let sink = CaptureSink::new(format);

    let pipeline = Pipeline::start(Box::new(sink), &played, &gain).expect("start pipeline");

    let first = Job {
        token: 101,
        decoder: decoder1,
        start: Duration::ZERO,
        span: None,
        segments: Vec::new(),
        replay_gain: None,
    };
    let second = Job {
        token: 102,
        decoder: decoder2,
        start: Duration::ZERO,
        span: None,
        segments: Vec::new(),
        replay_gain: None,
    };

    pipeline.send(FeedCmd::Load(first));
    pipeline.send(FeedCmd::Load(second));

    let start = std::time::Instant::now();
    while pipeline.shared.pushed.load(Ordering::SeqCst) == 0
        && start.elapsed() < Duration::from_secs(2)
    {
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        pipeline.shared.pushed.load(Ordering::SeqCst) > 0,
        "feeder should have decoded and pushed audio"
    );

    let mut taken = pipeline.stop_and_take();

    assert_eq!(
        taken.len(),
        2,
        "both active and queued jobs must be returned"
    );
    assert_eq!(taken[0].token, 101);
    assert_eq!(taken[1].token, 102);

    assert!(
        taken[0].start > Duration::ZERO,
        "active job position must be advanced beyond 0, got {:?}",
        taken[0].start
    );

    let mut block = Vec::new();
    let has_more = taken[0]
        .decoder
        .next_block(&mut block)
        .expect("next_block from carried decoder");
    assert!(has_more, "carried decoder should decode subsequent blocks");
    assert!(!block.is_empty(), "decoded block should not be empty");

    assert_eq!(taken[1].start, Duration::ZERO);

    let sink2 = CaptureSink::new(format);
    let mut pipeline2 = Pipeline::start(Box::new(sink2), &played, &gain).expect("start pipeline 2");

    let carried_first = taken.remove(0);
    let carried_second = taken.remove(0);

    pipeline2.send(FeedCmd::Load(carried_first));
    pipeline2.send(FeedCmd::Load(carried_second));

    let start = std::time::Instant::now();
    while pipeline2.shared.pushed.load(Ordering::SeqCst) == 0
        && start.elapsed() < Duration::from_secs(2)
    {
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        pipeline2.shared.pushed.load(Ordering::SeqCst) > 0,
        "new pipeline should decode and push audio using the carried jobs"
    );

    pipeline2.stop();
}
