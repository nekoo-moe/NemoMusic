//! The render path performs zero heap allocations, on both the normal and the underrun path.
//! This binary installs a counting global allocator, which needs `unsafe`.
#![allow(unsafe_code)] // reason: GlobalAlloc is an unsafe trait; test-only counting wrapper

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

mod common;

use nemo_audio::render::render;
use nemo_audio::ring::{new_ring, push_slice};

struct Counting;

thread_local! {
    // Per-thread count, so parallel tests do not see each other's allocations. A const-initialised
    // `Cell` has no destructor and never allocates.
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    ALLOCS.with(|c| c.set(c.get() + 1));
}

fn count() -> usize {
    ALLOCS.with(Cell::get)
}

// SAFETY: forwards every call unchanged to the system allocator and only counts.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        bump();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        bump();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        bump();
        unsafe { System.alloc_zeroed(layout) }
    }
}

#[global_allocator]
static A: Counting = Counting;

const CALLBACK_SAMPLES: usize = 512 * 2;
const CALLBACKS_10S: usize = 48_000 * 10 / 512 + 1;

#[test]
fn counter_sees_allocations() {
    let before = count();
    let boxed = std::hint::black_box(Box::new([0_u8; 64]));
    assert!(count() > before);
    drop(boxed);
}

#[test]
fn ten_seconds_of_render_allocate_nothing() {
    let (mut producer, mut consumer) = new_ring(CALLBACK_SAMPLES * CALLBACKS_10S);
    let block: Vec<f32> = (0..u16::try_from(CALLBACK_SAMPLES).unwrap_or(u16::MAX))
        .map(|i| f32::from(i) * 1e-4)
        .collect();
    for _ in 0..CALLBACKS_10S {
        assert_eq!(push_slice(&mut producer, &block), CALLBACK_SAMPLES);
    }
    let underruns = AtomicU64::new(0);
    let mut out = vec![0.0_f32; CALLBACK_SAMPLES];

    let before = count();
    for _ in 0..CALLBACKS_10S {
        render(&mut consumer, &mut out, &underruns);
    }
    let after = count();

    assert_eq!(after - before, 0, "render allocated on the normal path");
    assert_eq!(underruns.load(Ordering::SeqCst), 0);
}

#[test]
fn underrun_path_allocates_nothing() {
    let (mut producer, mut consumer) = new_ring(CALLBACK_SAMPLES);
    let half = vec![0.5_f32; CALLBACK_SAMPLES / 2];
    push_slice(&mut producer, &half);
    let underruns = AtomicU64::new(0);
    let mut out = vec![1.0_f32; CALLBACK_SAMPLES];

    let before = count();
    for _ in 0..1_000 {
        render(&mut consumer, &mut out, &underruns); // partial, then empty
    }
    let after = count();

    assert_eq!(after - before, 0, "render allocated on the underrun path");
    assert_eq!(underruns.load(Ordering::SeqCst), 1_000);
    assert!(out.iter().all(|s| *s == 0.0));
}

#[test]
fn ten_seconds_of_render_with_gain_allocate_nothing() {
    use std::sync::atomic::AtomicU32;

    use nemo_audio::render::{apply_gain, gain_for_percent};

    let (mut producer, mut consumer) = new_ring(CALLBACK_SAMPLES * CALLBACKS_10S);
    let block = vec![0.25_f32; CALLBACK_SAMPLES];
    for _ in 0..CALLBACKS_10S {
        assert_eq!(push_slice(&mut producer, &block), CALLBACK_SAMPLES);
    }
    let underruns = AtomicU64::new(0);
    let gain = AtomicU32::new(gain_for_percent(50).to_bits());
    let mut out = vec![0.0_f32; CALLBACK_SAMPLES];

    let before = count();
    for _ in 0..CALLBACKS_10S {
        let got = render(&mut consumer, &mut out, &underruns);
        apply_gain(&mut out[..got], &gain);
    }
    let after = count();

    assert_eq!(after - before, 0, "render + gain allocated");
    assert_eq!(underruns.load(Ordering::SeqCst), 0);
    assert!(
        out.iter().all(|s| s.to_bits() == 0.125_f32.to_bits()),
        "gain 50 halves the samples"
    );
}

/// Twelve filters per channel (the ten bands plus a low and a high shelf) on a stereo stream
/// at 48 kHz, 512-frame callbacks for 10 s.
#[test]
fn ten_seconds_of_equalizer_filters_allocate_nothing() {
    use nemo_audio::effects::{Biquad, Coefficients, ten_band};
    use nemo_core::effects::{EqFilter, FilterKind};

    let mut filters = ten_band([60, -30, 45, 0, -15, 30, -60, 90, -120, 120]);
    let shelf = |kind, freq_hz, gain_tenth_db| {
        Some(EqFilter {
            kind,
            freq_hz,
            gain_tenth_db,
            q_hundredths: 70,
        })
    };
    let [.., eleventh, twelfth] = &mut filters;
    *eleventh = shelf(FilterKind::LowShelf, 105, 40);
    *twelfth = shelf(FilterKind::HighShelf, 10_000, -20);
    let bank = filters
        .map(|f| {
            f.map_or(Coefficients::IDENTITY, |f| {
                Coefficients::for_filter(&f, 48_000)
            })
        })
        .map(Biquad::new);
    let mut channels = [bank; 2];
    let block: Vec<f32> = (0..u16::try_from(CALLBACK_SAMPLES).unwrap_or(u16::MAX))
        .map(|i| (f32::from(i) * 0.37).sin() * 0.5)
        .collect();
    let mut out = vec![0.0_f32; CALLBACK_SAMPLES];

    let before = count();
    for _ in 0..CALLBACKS_10S {
        out.copy_from_slice(&block);
        for frame in out.chunks_exact_mut(2) {
            for (sample, filters) in frame.iter_mut().zip(channels.iter_mut()) {
                *sample = filters.iter_mut().fold(*sample, |s, b| b.process(s));
            }
        }
    }
    let after = count();

    assert_eq!(after - before, 0, "the equalizer filters allocated");
    assert!(out.iter().all(|s| s.is_finite()));
}

/// A stereo 250 ms delay at 85% feedback, 48 kHz, 512-frame callbacks for 10 s, with a
/// parameter change and a reset on the way.
#[test]
fn ten_seconds_of_delay_allocate_nothing() {
    use nemo_audio::effects::{Delay, DelayParams};
    use nemo_core::effects::DelaySettings;

    let settings = DelaySettings {
        enabled: true,
        time_ms: 250,
        feedback_pct: 85,
        mix_pct: 50,
    };
    let params = DelayParams::new(&settings, 48_000);
    let shorter = DelayParams::new(
        &DelaySettings {
            time_ms: 40,
            ..settings
        },
        48_000,
    );
    let mut delay = Delay::new(2, 48_000, params);
    let block = vec![0.25_f32; CALLBACK_SAMPLES];
    let mut out = vec![0.0_f32; CALLBACK_SAMPLES];

    let before = count();
    for n in 0..CALLBACKS_10S {
        out.copy_from_slice(&block);
        if n == CALLBACKS_10S / 2 {
            delay.set_params(shorter);
        }
        if n == CALLBACKS_10S * 3 / 4 {
            delay.reset();
        }
        delay.process(&mut out);
    }
    let after = count();

    assert_eq!(after - before, 0, "the delay allocated");
    assert!(
        out.iter()
            .all(|s| s.is_finite() && s.abs() <= 0.25 / 0.15 + 1e-3)
    );
}

mod player_callback {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::thread;
    use std::time::Duration;

    use super::count;
    use crate::common::fakes::{FakeOpener, FakeResolver, local_track};
    use crate::common::fixtures::{tone_i16, wav_bytes};
    use nemo_audio::player::{Player, PlayerDeps};
    use nemo_audio::sink::{AudioSink, RenderCallback, SinkError, SinkFactory};
    use nemo_core::audio::{AudioFormat, OutputInfo, OutputMode};
    use nemo_core::library::{LocalPlayback, LocalTrackLookup, TrackSpan};
    use nemo_core::player::PlayerCommand;
    use nemo_core::track::Track;

    /// A device that counts the allocations its own thread makes inside the render callback.
    struct CountingSink {
        info: OutputInfo,
        callbacks: Arc<AtomicU64>,
        allocations: Arc<AtomicU64>,
        stop: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl AudioSink for CountingSink {
        fn output_info(&self) -> &OutputInfo {
            &self.info
        }

        fn start(&mut self, mut callback: RenderCallback) -> Result<(), SinkError> {
            let (callbacks, allocations, stop) = (
                Arc::clone(&self.callbacks),
                Arc::clone(&self.allocations),
                Arc::clone(&self.stop),
            );
            self.thread = Some(thread::spawn(move || {
                let mut buf = vec![0.0_f32; 960];
                let mut warm = 0;
                while !stop.load(Ordering::SeqCst) {
                    let before = count();
                    callback(&mut buf);
                    let used = count() - before;
                    // The first callbacks may touch lazily initialised state; measure after.
                    if buf.iter().any(|s| *s != 0.0) {
                        warm += 1;
                    }
                    if warm > 20 {
                        allocations.fetch_add(used as u64, Ordering::SeqCst);
                        callbacks.fetch_add(1, Ordering::SeqCst);
                    }
                    thread::sleep(Duration::from_micros(300));
                }
            }));
            Ok(())
        }

        fn stop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    struct Factory {
        callbacks: Arc<AtomicU64>,
        allocations: Arc<AtomicU64>,
    }

    impl SinkFactory for Factory {
        fn open(&self, source: AudioFormat) -> Result<Box<dyn AudioSink>, SinkError> {
            Ok(Box::new(CountingSink {
                info: OutputInfo {
                    device_name: "Counting".into(),
                    mode: OutputMode::Shared,
                    format: source,
                },
                callbacks: Arc::clone(&self.callbacks),
                allocations: Arc::clone(&self.allocations),
                stop: Arc::new(AtomicBool::new(false)),
                thread: None,
            }))
        }
    }

    /// Every local track is one span of its file, as a CUE track is.
    struct OneSpan(TrackSpan);

    impl LocalTrackLookup for OneSpan {
        fn playback(&self, _track: &Track) -> Option<LocalPlayback> {
            Some(LocalPlayback {
                span: Some(self.0),
                ..LocalPlayback::default()
            })
        }
    }

    #[test]
    fn the_real_player_callback_allocates_nothing_at_volume_100_and_50() {
        check_callback_allocates_nothing("long.wav", None);
    }

    #[test]
    fn the_callback_allocates_nothing_while_a_cue_span_plays() {
        let span = TrackSpan {
            start: Duration::from_secs(5),
            end: Some(Duration::from_secs(55)),
        };
        check_callback_allocates_nothing("span.wav", Some(Arc::new(OneSpan(span))));
    }

    /// Play a 60 s local file, then lower the volume, counting allocations in the callback.
    fn check_callback_allocates_nothing(name: &str, local: Option<Arc<dyn LocalTrackLookup>>) {
        let ints = tone_i16(48_000, 2, 60 * 48_000, 440.0);
        let dir = std::env::temp_dir().join(format!("nemo-rt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join(name);
        std::fs::write(&path, wav_bytes(&ints, 16, 48_000, 2)).expect("fixture");
        let callbacks = Arc::new(AtomicU64::new(0));
        let allocations = Arc::new(AtomicU64::new(0));
        let player = Player::spawn(PlayerDeps {
            resolver: Arc::new(FakeResolver { known: vec![] }),
            request: nemo_core::stream::StreamRequest::default(),
            local,
            opener: Arc::new(FakeOpener {
                files: std::collections::HashMap::default(),
                chunk: None,
            }),
            segments: None,
            sink_factory: Arc::new(Factory {
                callbacks: Arc::clone(&callbacks),
                allocations: Arc::clone(&allocations),
            }),
        });
        player.send(PlayerCommand::Play(local_track(1, path.clone())));
        let wait = |n: u64| {
            let start = std::time::Instant::now();
            while callbacks.load(Ordering::SeqCst) < n {
                assert!(start.elapsed() < Duration::from_secs(30), "no callbacks");
                thread::sleep(Duration::from_millis(5));
            }
        };
        wait(1000);
        player.send(PlayerCommand::SetVolume(50));
        let mark = callbacks.load(Ordering::SeqCst);
        wait(mark + 1000);
        assert_eq!(
            allocations.load(Ordering::SeqCst),
            0,
            "the render callback allocated"
        );
        drop(player);
        std::fs::remove_file(path).expect("cleanup");
    }
}
