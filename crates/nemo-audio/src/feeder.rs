//! The feeder thread: decode -> convert -> ring, one track after another.
//!
//! It owns the decoders and the ring producer. It never touches the sink. Seeks, resets and
//! SponsorBlock skips all happen here, on the decode side.

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use nemo_core::audio::AudioFormat;
use nemo_core::library::TrackSpan;
use nemo_core::segment::Segment;

use crate::chain::Chain;
use crate::pipeline::{CmdQueue, FeedCmd, FeedEvent, Job, PREFILL_MS, PipeShared};
use crate::ring::{Producer, push_slice};
use crate::stream_decoder::StreamDecoder;

/// Sleep when there is nothing to do or the ring is full.
const NAP: Duration = Duration::from_millis(2);
/// How long a ring flush waits for the audio callback to acknowledge.
const FLUSH_TIMEOUT: Duration = Duration::from_millis(500);

/// A [`TrackSpan`] in source frames, so that one CUE track ends on exactly the frame where the
/// next one starts.
#[derive(Clone, Copy)]
struct FrameSpan {
    /// File frame of the span start.
    start: u64,
    /// Frames in the span. `None` plays to the end of the file.
    len: Option<u64>,
}

impl FrameSpan {
    fn new(span: TrackSpan, rate: u32) -> Self {
        let start = to_frames(span.start, rate);
        Self {
            start,
            len: span
                .end
                .map(|end| to_frames(end, rate).saturating_sub(start)),
        }
    }
}

/// The track being decoded.
struct Active {
    token: u64,
    decoder: StreamDecoder,
    chain: Chain,
    /// Source frame index (from the track start) of the next decoded block.
    pos_frames: u64,
    /// The part of the file that is the track, if not all of it.
    span: Option<FrameSpan>,
    segments: Vec<Segment>,
    ignored: HashSet<String>,
}

/// The feeder thread state.
pub struct Feeder {
    producer: Producer<f32>,
    out: AudioFormat,
    shared: std::sync::Arc<PipeShared>,
    cmds: CmdQueue,
    events: Sender<FeedEvent>,
    queue: VecDeque<Job>,
    active: Option<Active>,
    skipping: bool,
    /// Set when a command invalidated the block being written.
    interrupted: bool,
    block: Vec<f32>,
    converted: Vec<f32>,
}

fn to_frames(d: Duration, rate: u32) -> u64 {
    // Rounding to the nearest frame is the intent; the value is non-negative.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let frames = (d.as_secs_f64() * f64::from(rate)).round() as u64;
    frames
}

/// The earliest time that lands on source frame `frames`. symphonia turns a seek time into a
/// frame by rounding down, so the time is rounded up to the next nanosecond.
fn frame_time(frames: u64, rate: u32) -> Duration {
    let nanos = (u128::from(frames) * 1_000_000_000).div_ceil(u128::from(rate.max(1)));
    Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX))
}

/// The file time of track position `to`: the same time without a span, else counted from the
/// span start, on an exact frame.
fn file_time(span: Option<FrameSpan>, to: Duration, rate: u32) -> Duration {
    span.map_or(to, |s| frame_time(s.start + to_frames(to, rate), rate))
}

impl Feeder {
    /// A feeder writing to `producer` in the `out` format.
    #[must_use]
    pub fn new(
        producer: Producer<f32>,
        out: AudioFormat,
        shared: std::sync::Arc<PipeShared>,
        cmds: CmdQueue,
        events: Sender<FeedEvent>,
    ) -> Self {
        Self {
            producer,
            out,
            shared,
            cmds,
            events,
            queue: VecDeque::new(),
            active: None,
            skipping: false,
            interrupted: false,
            block: Vec::new(),
            converted: Vec::new(),
        }
    }

    /// The thread body. Returns when the pipeline asks it to stop.
    pub fn run(mut self) {
        while !self.shared.stop.load(Ordering::SeqCst) {
            self.apply_commands();
            self.interrupted = false;
            if self.active.is_none() && !self.activate_next() {
                self.shared.idle.store(true, Ordering::SeqCst);
                thread::sleep(NAP);
                continue;
            }
            self.step();
        }
    }

    fn send(&self, event: FeedEvent) {
        // The receiver is gone only when the player is shutting down.
        let _ = self.events.send(event);
    }

    fn apply_commands(&mut self) {
        let cmds: Vec<FeedCmd> = match self.cmds.lock() {
            Ok(mut q) => q.drain(..).collect(),
            Err(_) => return,
        };
        for cmd in cmds {
            match cmd {
                FeedCmd::Load(job) => self.queue.push_back(job),
                FeedCmd::Seek { token, to } => self.seek(token, to),
                FeedCmd::SetSegments {
                    token,
                    mut segments,
                } => {
                    segments.sort_by_key(|s| s.start);
                    if let Some(a) = self.active.as_mut().filter(|a| a.token == token) {
                        a.segments = segments;
                    } else if let Some(j) = self.queue.iter_mut().find(|j| j.token == token) {
                        j.segments = segments;
                    }
                }
                FeedCmd::SetSkipping(on) => self.skipping = on,
                FeedCmd::Ignore { token, id } => {
                    if let Some(a) = self.active.as_mut().filter(|a| a.token == token) {
                        a.ignored.insert(id);
                    }
                }
                FeedCmd::Reset => self.reset(),
            }
        }
    }

    fn reset(&mut self) {
        self.queue.clear();
        self.active = None;
        self.flush_ring();
        self.shared.idle.store(true, Ordering::SeqCst);
        self.interrupted = true;
    }

    /// Empty the ring and restart the frame counters. Waits for the callback to acknowledge.
    fn flush_ring(&mut self) {
        self.shared.hold.store(true, Ordering::SeqCst);
        let request = self.shared.flush_req.fetch_add(1, Ordering::SeqCst) + 1;
        let start = std::time::Instant::now();
        while self.shared.flush_ack.load(Ordering::SeqCst) != request
            && start.elapsed() < FLUSH_TIMEOUT
            && !self.shared.stop.load(Ordering::SeqCst)
        {
            thread::sleep(Duration::from_millis(1));
        }
        self.shared.pushed.store(0, Ordering::SeqCst);
    }

    fn seek(&mut self, token: u64, to: Duration) {
        if let Some(job) = self.queue.iter_mut().find(|j| j.token == token) {
            job.start = to;
            return;
        }
        let Some(mut a) = self.active.take().filter(|a| a.token == token) else {
            self.send(FeedEvent::SeekMiss { token, to });
            return;
        };
        let rate = a.decoder.info().format.sample_rate;
        if let Err(e) = a.decoder.seek(file_time(a.span, to, rate)) {
            tracing::warn!(error = %e, "seek failed");
            self.send(FeedEvent::Failed {
                token,
                message: "could not seek in this track".into(),
            });
            return;
        }
        a.chain.reset();
        a.pos_frames = to_frames(to, rate);
        self.flush_ring();
        self.send(FeedEvent::Started {
            token,
            at_frame: 0,
            base: to,
        });
        self.active = Some(a);
        self.interrupted = true;
    }

    /// Start the next queued job. Returns `false` when there is none.
    fn activate_next(&mut self) -> bool {
        let Some(mut job) = self.queue.pop_front() else {
            return false;
        };
        let source = job.decoder.info().format;
        let chain = match Chain::new(source, self.out) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "could not build the conversion chain");
                self.fail(
                    job.token,
                    "could not convert this track to the output format",
                );
                return true;
            }
        };
        // A CUE track starts at its span start: an accurate seek lands on that exact frame.
        let span = job.span.map(|s| FrameSpan::new(s, source.sample_rate));
        let from = file_time(span, job.start, source.sample_rate);
        if !from.is_zero() && job.decoder.seek(from).is_err() {
            self.fail(job.token, "could not seek in this track");
            return true;
        }
        self.shared.idle.store(false, Ordering::SeqCst);
        let buffered = self.buffered_frames();
        if buffered < self.prefill_frames() {
            self.shared.hold.store(true, Ordering::SeqCst);
        }
        self.send(FeedEvent::Started {
            token: job.token,
            at_frame: self.shared.pushed.load(Ordering::SeqCst),
            base: job.start,
        });
        self.active = Some(Active {
            token: job.token,
            pos_frames: to_frames(job.start, source.sample_rate),
            span,
            decoder: job.decoder,
            chain,
            segments: std::mem::take(&mut job.segments),
            ignored: HashSet::new(),
        });
        true
    }

    fn fail(&mut self, token: u64, message: &str) {
        self.send(FeedEvent::Failed {
            token,
            message: message.into(),
        });
        self.active = None;
    }

    fn prefill_frames(&self) -> u64 {
        u64::from(self.out.sample_rate) * PREFILL_MS / 1000
    }

    fn buffered_frames(&self) -> u64 {
        self.shared
            .pushed
            .load(Ordering::SeqCst)
            .saturating_sub(self.shared.played.load(Ordering::SeqCst))
    }

    fn release_if_primed(&self) {
        if self.shared.hold.load(Ordering::SeqCst)
            && self.buffered_frames() >= self.prefill_frames()
        {
            self.shared.hold.store(false, Ordering::SeqCst);
        }
    }

    /// Decode one block of the active track and write it.
    fn step(&mut self) {
        let Some(a) = self.active.as_mut() else {
            return;
        };
        let token = a.token;
        match a.decoder.next_block(&mut self.block) {
            Ok(true) => {}
            Ok(false) => {
                self.finish_active();
                return;
            }
            Err(e) => {
                tracing::warn!(error = %e, "decode failed");
                self.fail(token, "this track could not be decoded");
                return;
            }
        }
        let at_span_end = self.trim_to_span_end();
        let Ok(cut) = self.convert_block() else {
            self.fail(token, "this track could not be converted");
            return;
        };
        if !self.push_converted() {
            return;
        }
        if at_span_end {
            self.finish_active();
        } else if let Some(segment) = cut {
            self.skip_segment(&segment);
        }
    }

    /// Cut the block at the span end, counting frames from the span start. Returns `true` when
    /// the block reaches the end, so the track ends after it.
    fn trim_to_span_end(&mut self) -> bool {
        let Some(a) = self.active.as_ref() else {
            return false;
        };
        let Some(len) = a.span.and_then(|s| s.len) else {
            return false;
        };
        let channels = usize::from(a.decoder.info().format.channels);
        let left = usize::try_from(len.saturating_sub(a.pos_frames)).unwrap_or(usize::MAX);
        if self.block.len() / channels < left {
            return false;
        }
        self.block.truncate(left * channels);
        true
    }

    /// Trim the block at a segment start if one lies inside it, then convert it into
    /// `self.converted`. Returns the segment to skip after the block is written.
    fn convert_block(&mut self) -> Result<Option<Segment>, ()> {
        let cut = self.find_cut();
        let Some(a) = self.active.as_mut() else {
            return Err(());
        };
        let channels = usize::from(a.decoder.info().format.channels);
        let frames = self.block.len() / channels;
        a.pos_frames += frames as u64;
        let mut segment = None;
        if let Some((keep, seg)) = cut {
            self.block.truncate(keep * channels);
            segment = Some(seg);
        }
        self.converted.clear();
        a.chain
            .process(&self.block, &mut self.converted)
            .map_err(|e| tracing::warn!(error = %e, "conversion failed"))?;
        Ok(segment)
    }

    /// Frames of the current block to keep, and the segment that starts inside or covers it.
    fn find_cut(&self) -> Option<(usize, Segment)> {
        let a = self.active.as_ref().filter(|_| self.skipping)?;
        let format = a.decoder.info().format;
        let rate = f64::from(format.sample_rate);
        let frames = self.block.len() / usize::from(format.channels);
        let b0 = crate::units::f64_of(a.pos_frames) / rate;
        let b1 = crate::units::f64_of(a.pos_frames + frames as u64) / rate;
        let seg = a.segments.iter().find(|s| {
            !a.ignored.contains(&s.id) && s.start.as_secs_f64() < b1 && s.end.as_secs_f64() > b0
        })?;
        // Rounding to the nearest frame is the intent; the value is non-negative.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let keep = ((seg.start.as_secs_f64() - b0).max(0.0) * rate).round() as usize;
        Some((keep.min(frames), seg.clone()))
    }

    /// Flush the conversion tail, jump the decoder to the segment end and report the skip.
    fn skip_segment(&mut self, segment: &Segment) {
        let Some(a) = self.active.as_mut() else {
            return;
        };
        self.converted.clear();
        if a.chain.finish(&mut self.converted).is_err() || !self.push_converted() {
            return;
        }
        let Some(a) = self.active.as_mut() else {
            return;
        };
        a.chain.reset();
        let rate = a.decoder.info().format.sample_rate;
        let past_end = a.decoder.info().duration.is_some_and(|d| segment.end >= d);
        if past_end
            || a.decoder
                .seek(file_time(a.span, segment.end, rate))
                .is_err()
        {
            self.complete_active();
            return;
        }
        a.pos_frames = to_frames(segment.end, rate);
        let token = a.token;
        self.send(FeedEvent::Skipped {
            token,
            segment: segment.clone(),
            at_frame: self.shared.pushed.load(Ordering::SeqCst),
        });
    }

    /// The decoder reached the end: write the conversion tail, then report the track done.
    fn finish_active(&mut self) {
        self.converted.clear();
        if let Some(a) = self.active.as_mut()
            && let Err(e) = a.chain.finish(&mut self.converted)
        {
            tracing::warn!(error = %e, "could not flush the conversion tail");
            self.converted.clear();
        }
        if self.push_converted() {
            self.complete_active();
        }
    }

    fn complete_active(&mut self) {
        let Some(a) = self.active.take() else {
            return;
        };
        self.send(FeedEvent::Finished {
            token: a.token,
            end_frame: self.shared.pushed.load(Ordering::SeqCst),
        });
        if self.queue.is_empty() {
            self.shared.idle.store(true, Ordering::SeqCst);
            self.shared.hold.store(false, Ordering::SeqCst);
        }
    }
    /// Write `self.converted` into the ring. Returns `false` when a command or a stop
    /// request cut the write short.
    fn push_converted(&mut self) -> bool {
        let channels = usize::from(self.out.channels);
        let mut offset = 0;
        let total = self.converted.len();
        while offset < total {
            if self.shared.stop.load(Ordering::SeqCst) {
                return false;
            }
            let n = push_slice(&mut self.producer, &self.converted[offset..]);
            if n > 0 {
                offset += n;
                self.shared
                    .pushed
                    .fetch_add((n / channels) as u64, Ordering::SeqCst);
                self.release_if_primed();
            } else {
                self.apply_commands();
                if self.interrupted {
                    return false;
                }
                thread::sleep(NAP);
            }
        }
        true
    }
}
