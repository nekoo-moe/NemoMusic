//! One open output: sink, ring, feeder thread and the atomics that tie them together.
//!
//! The feeder thread decodes, converts and writes into the ring; the sink callback reads the
//! ring. They share [`PipeShared`] (atomics only) and nothing else.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use nemo_core::audio::{AudioFormat, OutputInfo, ProcessingStage};
use nemo_core::segment::Segment;

use crate::feeder::Feeder;
use crate::render::{UNITY_GAIN_BITS, apply_gain, render};
use crate::ring::{Consumer, new_ring};
use crate::sink::{AudioSink, RenderCallback, SinkError};
use crate::stream_decoder::StreamDecoder;

/// Length of the ring in seconds of output audio.
const RING_SECONDS: usize = 1;
/// Audio the feeder buffers before it releases the callback, in milliseconds.
pub const PREFILL_MS: u64 = 200;

/// Atomics shared by the feeder thread, the audio callback and the control thread.
pub struct PipeShared {
    /// Callbacks that ran short of samples.
    pub underruns: AtomicU64,
    /// Output frames the callback has taken from the ring since the last flush.
    pub played: Arc<AtomicU64>,
    /// Output frames the feeder has written since the last flush.
    pub pushed: AtomicU64,
    /// Gain factor as `f32` bits.
    pub gain: Arc<AtomicU32>,
    /// Output silence and keep the ring.
    pub paused: AtomicBool,
    /// Output silence until enough audio is buffered. Not an underrun.
    pub hold: AtomicBool,
    /// Nothing more will be written: an empty ring is not an underrun.
    pub idle: AtomicBool,
    /// Ask the feeder thread to end.
    pub stop: AtomicBool,
    /// Number of ring flushes requested by the feeder.
    pub flush_req: AtomicU64,
    /// Number of ring flushes the callback has carried out.
    pub flush_ack: AtomicU64,
    /// Output channel count.
    pub channels: usize,
}

/// Work for the feeder thread.
pub enum FeedCmd {
    /// Queue a track behind the one being decoded.
    Load(Job),
    /// Move the track with this token to a position.
    Seek {
        /// Token of the track to seek in.
        token: u64,
        /// Target position.
        to: Duration,
    },
    /// Segments that may be skipped in the track with this token.
    SetSegments {
        /// Token of the track.
        token: u64,
        /// The segments.
        segments: Vec<Segment>,
    },
    /// Switch segment skipping on or off.
    SetSkipping(bool),
    /// Do not skip this segment again in the track with this token.
    Ignore {
        /// Token of the track.
        token: u64,
        /// Segment id.
        id: String,
    },
    /// Drop every queued and running track and empty the ring.
    Reset,
}

/// A track handed to the feeder.
pub struct Job {
    /// Identifies the track in events.
    pub token: u64,
    /// The opened stream.
    pub decoder: StreamDecoder,
    /// Where in the track to start.
    pub start: Duration,
    /// Segments known already (usually empty; `SetSegments` adds them later).
    pub segments: Vec<Segment>,
}

/// What the feeder reports.
#[derive(Debug)]
pub enum FeedEvent {
    /// The first frame of a track (or of a new position) was written at output frame `at_frame`.
    Started {
        /// Track token.
        token: u64,
        /// Value of `pushed` when the track's first frame was written.
        at_frame: u64,
        /// Track position of that first frame.
        base: Duration,
    },
    /// The whole track is in the ring; its end is at output frame `end_frame`.
    Finished {
        /// Track token.
        token: u64,
        /// Value of `pushed` after the last frame.
        end_frame: u64,
    },
    /// A segment was cut out of the stream at output frame `at_frame`.
    Skipped {
        /// Track token.
        token: u64,
        /// The segment.
        segment: Segment,
        /// Value of `pushed` where the cut happened.
        at_frame: u64,
    },
    /// The track failed. The message holds no secrets.
    Failed {
        /// Track token.
        token: u64,
        /// User-safe text.
        message: String,
    },
    /// A seek targeted a track the feeder no longer holds.
    SeekMiss {
        /// Track token.
        token: u64,
        /// The requested position.
        to: Duration,
    },
}

/// Commands from the control thread to the feeder.
pub type CmdQueue = Arc<Mutex<VecDeque<FeedCmd>>>;

/// An open output with its feeder thread.
pub struct Pipeline {
    sink: Box<dyn AudioSink>,
    /// Shared atomics.
    pub shared: Arc<PipeShared>,
    cmds: CmdQueue,
    /// Events from the feeder.
    pub events: Receiver<FeedEvent>,
    thread: Option<JoinHandle<()>>,
}

impl Pipeline {
    /// Start the feeder and the sink. `played` and `gain` outlive the pipeline.
    ///
    /// # Errors
    /// Returns the sink's error when it cannot start.
    pub fn start(
        mut sink: Box<dyn AudioSink>,
        played: &Arc<AtomicU64>,
        gain: &Arc<AtomicU32>,
    ) -> Result<Self, SinkError> {
        let format = sink.output_info().format;
        let channels = usize::from(format.channels);
        let frames = format.sample_rate as usize * RING_SECONDS;
        let (producer, consumer) = new_ring((frames * channels).max(channels));
        played.store(0, Ordering::Relaxed);
        let shared = Arc::new(PipeShared {
            underruns: AtomicU64::new(0),
            played: Arc::clone(played),
            pushed: AtomicU64::new(0),
            gain: Arc::clone(gain),
            paused: AtomicBool::new(false),
            hold: AtomicBool::new(true),
            idle: AtomicBool::new(true),
            stop: AtomicBool::new(false),
            flush_req: AtomicU64::new(0),
            flush_ack: AtomicU64::new(0),
            channels,
        });
        sink.start(make_callback(consumer, Arc::clone(&shared)))?;
        let cmds: CmdQueue = Arc::new(Mutex::new(VecDeque::new()));
        let (tx, events) = channel();
        let feeder = Feeder::new(producer, format, Arc::clone(&shared), Arc::clone(&cmds), tx);
        let thread = thread::Builder::new()
            .name("nemo-feeder".into())
            .spawn(move || feeder.run())
            .map_err(|e| SinkError::Backend(e.to_string()))?;
        Ok(Self {
            sink,
            shared,
            cmds,
            events,
            thread: Some(thread),
        })
    }

    /// Queue a command for the feeder.
    pub fn send(&self, cmd: FeedCmd) {
        if let Ok(mut q) = self.cmds.lock() {
            q.push_back(cmd);
        }
    }

    /// The device end of the signal path.
    #[must_use]
    pub fn output_info(&self) -> &OutputInfo {
        self.sink.output_info()
    }

    /// Latency the sink last reported, in output frames.
    #[must_use]
    pub fn latency_frames(&self) -> u64 {
        self.sink.latency_frames()
    }

    /// Stop the feeder, then the sink.
    pub fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            // A panicked feeder only means the ring stopped refilling; nothing to recover.
            let _ = handle.join();
        }
        self.sink.stop();
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The audio callback: ring -> (gain) -> device buffer.
///
/// RT-SAFE: atomics, a copy out of the ring and an optional in-place multiply. No allocation,
/// no lock, no logging.
fn make_callback(mut consumer: Consumer<f32>, shared: Arc<PipeShared>) -> RenderCallback {
    let mut seen_flush = 0_u64;
    Box::new(move |out: &mut [f32]| {
        let requested = shared.flush_req.load(Ordering::SeqCst);
        if requested != seen_flush {
            discard_all(&mut consumer);
            shared.played.store(0, Ordering::SeqCst);
            seen_flush = requested;
            shared.flush_ack.store(requested, Ordering::SeqCst);
        }
        let silent = shared.paused.load(Ordering::Relaxed)
            || shared.hold.load(Ordering::SeqCst)
            || (shared.idle.load(Ordering::SeqCst) && consumer.is_empty());
        if silent {
            out.fill(0.0);
            return;
        }
        let got = render(&mut consumer, out, &shared.underruns);
        apply_gain(&mut out[..got], &shared.gain);
        shared
            .played
            .fetch_add((got / shared.channels) as u64, Ordering::SeqCst);
    })
}

/// Drop everything in the ring.
///
/// RT-SAFE: one chunk read and commit, no allocation.
fn discard_all(consumer: &mut Consumer<f32>) {
    let n = consumer.slots();
    if n > 0
        && let Ok(chunk) = consumer.read_chunk(n)
    {
        chunk.commit_all();
    }
}

/// The stages that change samples between a source and an output format, in signal order.
/// Empty when the formats match.
#[must_use]
pub fn plan_stages(source: AudioFormat, output: AudioFormat) -> Vec<ProcessingStage> {
    let resample =
        (source.sample_rate != output.sample_rate).then_some(ProcessingStage::Resample {
            from_hz: source.sample_rate,
            to_hz: output.sample_rate,
        });
    let mix = (source.channels != output.channels).then_some(ProcessingStage::ChannelMix {
        from: source.channels,
        to: output.channels,
    });
    // Mixing first is cheaper when channels shrink; resampling first when they grow.
    if source.channels > output.channels {
        mix.into_iter().chain(resample).collect()
    } else {
        resample.into_iter().chain(mix).collect()
    }
}

/// The gain stage for the current gain bits, if the gain is not unity.
#[must_use]
pub fn gain_stage(bits: u32) -> Option<ProcessingStage> {
    (bits != UNITY_GAIN_BITS).then(|| ProcessingStage::Gain {
        db: 20.0 * f32::from_bits(bits).log10(),
    })
}
