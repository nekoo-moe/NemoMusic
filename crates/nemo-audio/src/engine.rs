//! The engine: decoder -> ring -> sink, with an honest signal-path report.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use nemo_core::audio::{OutputInfo, SignalPath, StreamInfo};

use crate::decoder::DecodedAudio;
use crate::render::render;
use crate::ring::{new_ring, push_slice};
use crate::sink::{AudioSink, SinkError, check_format};

/// How long the feeder thread sleeps when the ring is full.
const FEEDER_NAP: Duration = Duration::from_millis(2);

/// Why the engine could not start.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The sink rejected the source format or failed to start.
    #[error(transparent)]
    Sink(#[from] SinkError),
    /// The decoded audio holds no samples.
    #[error("the decoded audio is empty")]
    Empty,
}

/// State shared between the engine, the feeder thread and the audio callback.
struct Shared {
    underruns: AtomicU64,
    played: AtomicU64,
    stop: AtomicBool,
}

/// Plays one decoded file through a sink with no sample-altering stage.
pub struct Engine<S: AudioSink> {
    sink: S,
    shared: Arc<Shared>,
    info: StreamInfo,
    total_samples: u64,
    feeder: Option<JoinHandle<()>>,
}

impl<S: AudioSink> Engine<S> {
    /// Check the format, pre-fill the ring, start the feeder thread and the sink.
    ///
    /// `ring_samples` is the ring capacity in interleaved samples. The ring is filled before
    /// the sink starts so playback begins without an underrun.
    ///
    /// # Errors
    ///
    /// [`EngineError::Empty`] for no samples, [`SinkError::FormatMismatch`] (wrapped) when the
    /// sink format differs from the source, or any error from [`AudioSink::start`].
    pub fn start(
        audio: DecodedAudio,
        mut sink: S,
        ring_samples: usize,
    ) -> Result<Self, EngineError> {
        if audio.samples.is_empty() {
            return Err(EngineError::Empty);
        }
        check_format(audio.format, sink.output_info().format)?;
        let total_samples = audio.samples.len() as u64;
        let (mut producer, mut consumer) = new_ring(ring_samples.max(1));
        let first = push_slice(&mut producer, &audio.samples);
        let shared = Arc::new(Shared {
            underruns: AtomicU64::new(0),
            played: AtomicU64::new(0),
            stop: AtomicBool::new(false),
        });

        let cb_shared = Arc::clone(&shared);
        sink.start(Box::new(move |out: &mut [f32]| {
            // RT-SAFE: atomics and a copy only.
            let left = total_samples.saturating_sub(cb_shared.played.load(Ordering::Relaxed));
            let want = usize::try_from(left).map_or(out.len(), |l| l.min(out.len()));
            let got = render(&mut consumer, &mut out[..want], &cb_shared.underruns);
            out[want..].fill(0.0);
            cb_shared.played.fetch_add(got as u64, Ordering::Relaxed);
        }))?;

        let feeder_shared = Arc::clone(&shared);
        let samples = audio.samples;
        let feeder = thread::spawn(move || {
            let mut offset = first;
            while offset < samples.len() && !feeder_shared.stop.load(Ordering::Relaxed) {
                let n = push_slice(&mut producer, &samples[offset..]);
                offset += n;
                if n == 0 {
                    thread::sleep(FEEDER_NAP);
                }
            }
        });

        Ok(Self {
            sink,
            shared,
            info: audio.info,
            total_samples,
            feeder: Some(feeder),
        })
    }

    /// Source -> processing -> output. Processing is empty because this engine has no stage that
    /// alters samples; the mode is whatever the sink reports (shared for [`crate::sink::CpalSink`]).
    #[must_use]
    pub fn signal_path(&self) -> SignalPath {
        let output: OutputInfo = self.sink.output_info().clone();
        SignalPath {
            source: self.info.clone(),
            processing: Vec::new(),
            output,
        }
    }

    /// Number of callbacks that ran short of samples (each counts once).
    #[must_use]
    pub fn underruns(&self) -> u64 {
        self.shared.underruns.load(Ordering::Relaxed)
    }

    /// `true` once every sample of the file has been handed to the sink.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.shared.played.load(Ordering::Relaxed) >= self.total_samples
    }

    /// Mutable access to the sink, for example to pump a [`crate::sink::CaptureSink`].
    pub fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    /// Stop the sink and the feeder thread.
    pub fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.sink.stop();
        if let Some(handle) = self.feeder.take() {
            // A panicked feeder only means the ring stopped refilling; nothing to recover.
            let _ = handle.join();
        }
    }
}

impl<S: AudioSink> Drop for Engine<S> {
    fn drop(&mut self) {
        self.stop();
    }
}
