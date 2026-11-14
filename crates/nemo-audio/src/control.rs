//! The control thread: the queue, the state machine and the command handlers.
//!
//! It owns the output [`Pipeline`] and talks to the feeder thread through it. Events from the
//! feeder are handled in `control_tick.rs`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, PoisonError};
use std::time::Duration;

use nemo_core::audio::{AudioFormat, ProcessingStage, SignalPath, StreamInfo};
use nemo_core::effects::{EffectsStatus, TransitionSettings};
use nemo_core::player::{PlayState, PlayerEvent, PlayerSnapshot, RepeatMode};
use nemo_core::segment::Segment;
use nemo_core::track::Track;

use crate::pipeline::{FeedCmd, Job, Pipeline, gain_stage, plan_stages};
use crate::player::{Msg, PlayerDeps, Shared};
use crate::prepare::{Prepared, spawn_segments};
use crate::render::UNITY_GAIN_BITS;

/// How often the control thread wakes when no message arrives.
const TICK: Duration = Duration::from_millis(5);

/// One queue entry. The id lets a prefetched track be matched after the queue changed.
pub(crate) struct Entry {
    pub id: u64,
    pub track: Track,
}

/// A track that is current or about to be.
pub(crate) struct Run {
    pub token: u64,
    pub entry: u64,
    pub track: Track,
    pub start: Duration,
    pub loaded: bool,
    pub started: bool,
    pub source: Option<StreamInfo>,
    pub stages: Vec<ProcessingStage>,
    pub duration: Option<Duration>,
    pub finished: Option<u64>,
    pub at_frame: Option<u64>,
    pub segments_requested: bool,
}

impl Run {
    pub fn new(token: u64, entry: u64, track: Track, start: Duration) -> Self {
        Self {
            token,
            entry,
            track,
            start,
            loaded: false,
            started: false,
            source: None,
            stages: Vec::new(),
            duration: None,
            finished: None,
            at_frame: None,
            segments_requested: false,
        }
    }
}

/// Progress of the prefetched next track.
pub(crate) enum NextState {
    Preparing,
    Ready(Box<Prepared>),
    Failed,
}

/// The next track, opened ahead of time.
pub(crate) struct NextSlot {
    pub token: u64,
    pub entry: u64,
    pub track: Track,
    pub state: NextState,
}

/// The control thread state.
pub(crate) struct Core {
    pub deps: PlayerDeps,
    pub shared: Arc<Shared>,
    pub tx: Sender<Msg>,
    pub queue: Vec<Entry>,
    pub index: Option<usize>,
    pub ids: u64,
    pub state: PlayState,
    pub volume: u8,
    pub skip: bool,
    pub repeat: RepeatMode,
    pub gain: Arc<AtomicU32>,
    pub pipe: Option<Pipeline>,
    pub underruns_base: u64,
    pub last_underruns: u64,
    pub cur: Option<Run>,
    pub following: Option<Run>,
    pub next: Option<NextSlot>,
    pub failed: HashSet<u64>,
    pub skips: VecDeque<(u64, u64, Segment)>,
    pub last_skip: Option<(u64, Segment)>,
    pub ignored: HashMap<u64, HashSet<String>>,
    /// A remote device plays: decode nothing and hold no output device.
    pub parked: bool,
    /// The effects last set (audio-effects); nothing reaches the samples yet.
    pub effects: EffectsStatus,
    /// The song-transition settings last set (audio-effects); not applied yet.
    pub transition: TransitionSettings,
}

impl Core {
    pub fn new(deps: PlayerDeps, shared: Arc<Shared>, tx: Sender<Msg>) -> Self {
        Self {
            deps,
            shared,
            tx,
            queue: Vec::new(),
            index: None,
            ids: 0,
            state: PlayState::Stopped,
            volume: 100,
            skip: false,
            repeat: RepeatMode::Off,
            gain: Arc::new(AtomicU32::new(UNITY_GAIN_BITS)),
            pipe: None,
            underruns_base: 0,
            last_underruns: 0,
            cur: None,
            following: None,
            next: None,
            failed: HashSet::new(),
            skips: VecDeque::new(),
            last_skip: None,
            ignored: HashMap::new(),
            parked: false,
            effects: EffectsStatus::default(),
            transition: TransitionSettings::default(),
        }
    }

    /// The thread body.
    pub fn run(mut self, rx: &Receiver<Msg>) {
        loop {
            match rx.recv_timeout(TICK) {
                Ok(Msg::Shutdown) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Ok(msg) => self.handle(msg),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }
            self.tick();
        }
        self.stop_all();
    }

    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Cmd(cmd) => self.command(cmd),
            Msg::Prepared { token, result } => self.on_prepared(token, result),
            Msg::Segments { token, segments } => {
                if let Some(p) = &self.pipe {
                    p.send(FeedCmd::SetSegments { token, segments });
                }
            }
            Msg::Shutdown => {}
        }
    }

    pub fn new_id(&mut self) -> u64 {
        self.ids += 1;
        self.ids
    }

    pub fn entry(&mut self, track: Track) -> Entry {
        Entry {
            id: self.new_id(),
            track,
        }
    }

    // ---- notifications -------------------------------------------------------------------

    /// The signal path of the current track, from real engine state only.
    pub fn signal_path(&self) -> Option<SignalPath> {
        let cur = self.cur.as_ref().filter(|c| c.loaded)?;
        let pipe = self.pipe.as_ref()?;
        let mut processing = cur.stages.clone();
        processing.extend(gain_stage(self.gain.load(Ordering::Relaxed)));
        Some(SignalPath {
            source: cur.source.clone()?,
            processing,
            output: pipe.output_info().clone(),
        })
    }

    fn total_underruns(&self) -> u64 {
        let live = self
            .pipe
            .as_ref()
            .map_or(0, |p| p.shared.underruns.load(Ordering::Relaxed));
        self.underruns_base + live
    }

    /// Copy the current state into the snapshot the UI reads.
    pub fn publish(&self) {
        let cur = self.cur.as_ref();
        let snap = PlayerSnapshot {
            state: self.state,
            current: cur.map(|c| c.track.clone()),
            queue: self.queue.iter().map(|e| e.track.clone()).collect(),
            queue_index: self.index,
            duration: cur.and_then(|c| c.duration),
            volume_percent: self.volume,
            signal_path: self.signal_path(),
            underruns: self.total_underruns(),
            skip_segments: self.skip,
            repeat: self.repeat,
            // Live streams are not played yet (youtube-extras).
            live: false,
            effects: self.effects,
            transition: self.transition,
        };
        *self
            .shared
            .snapshot
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = snap;
    }

    /// Publish the snapshot, then tell the subscribers.
    pub fn notify(&self, event: &PlayerEvent) {
        self.publish();
        self.shared.emit(event);
    }

    pub fn set_state(&mut self, state: PlayState) {
        if self.state != state {
            self.state = state;
            self.notify(&PlayerEvent::StateChanged(state));
        }
    }

    pub fn fail(&self, message: &str) {
        self.notify(&PlayerEvent::Failed {
            message: message.to_owned(),
        });
    }

    // ---- starting and stopping -----------------------------------------------------------

    /// Make queue entry `index` the current track and start opening it at `start`. While parked it
    /// only moves the queue position and reports the new track.
    pub fn start_track(&mut self, index: usize, start: Duration) {
        let Some(entry) = self.queue.get(index) else {
            self.stop_all();
            return;
        };
        let (entry_id, track) = (entry.id, entry.track.clone());
        if let Some(p) = &self.pipe {
            p.send(FeedCmd::Reset);
            p.shared.paused.store(false, Ordering::SeqCst);
        }
        self.next = None;
        self.following = None;
        self.skips.clear();
        self.last_skip = None;
        self.ignored.retain(|e, _| *e == entry_id);
        let token = self.new_id();
        self.cur = Some(Run::new(token, entry_id, track.clone(), start));
        self.index = Some(index);
        if self.parked {
            // A remote device plays: move through the queue, open nothing.
            self.notify(&PlayerEvent::TrackChanged(track.id.clone()));
            self.notify(&PlayerEvent::QueueChanged);
            return;
        }
        let pos = &self.shared.pos;
        pos.start.store(u64::MAX, Ordering::SeqCst);
        pos.base_us.store(micros(start), Ordering::SeqCst);
        pos.duration_us.store(0, Ordering::SeqCst);
        pos.active.store(true, Ordering::SeqCst);
        self.set_state(PlayState::Buffering);
        self.notify(&PlayerEvent::TrackChanged(track.id.clone()));
        self.notify(&PlayerEvent::QueueChanged);
        self.spawn_prepare(token, track);
    }

    /// Stop playback, release the device and reset the queue position. The queue stays.
    pub fn stop_all(&mut self) {
        if let Some(mut p) = self.pipe.take() {
            self.underruns_base += p.shared.underruns.load(Ordering::Relaxed);
            p.stop();
        }
        let had_state = self.state != PlayState::Stopped;
        self.cur = None;
        self.following = None;
        self.next = None;
        self.index = None;
        self.skips.clear();
        self.last_skip = None;
        self.ignored.clear();
        self.shared.pos.active.store(false, Ordering::SeqCst);
        self.state = PlayState::Stopped;
        self.publish();
        if had_state {
            self.shared
                .emit(&PlayerEvent::StateChanged(PlayState::Stopped));
        }
        self.shared.emit(&PlayerEvent::QueueChanged);
    }

    // ---- opening -------------------------------------------------------------------------

    pub fn on_prepared(&mut self, token: u64, result: Result<Box<Prepared>, String>) {
        if self
            .cur
            .as_ref()
            .is_some_and(|c| c.token == token && !c.loaded)
        {
            match result {
                Ok(prepared) => self.load_current(*prepared),
                Err(message) => self.fail_current(&message),
            }
        } else if let Some(slot) = self.next.as_mut().filter(|n| n.token == token) {
            match result {
                Ok(prepared) => slot.state = NextState::Ready(prepared),
                Err(message) => {
                    slot.state = NextState::Failed;
                    self.notify(&PlayerEvent::Failed { message });
                }
            }
            self.try_load_following();
        }
    }

    /// Report a failed current track and move on to the next entry.
    pub fn fail_current(&mut self, message: &str) {
        self.fail(message);
        if let Some(c) = &self.cur {
            self.failed.insert(c.entry);
        }
        match self.next_entry_index() {
            Some(i) => self.start_track(i, Duration::ZERO),
            None => self.stop_all(),
        }
    }

    /// Open a sink for `format`, or keep the open one when it can carry the track.
    pub fn ensure_pipeline(&mut self, format: AudioFormat) -> Result<(), String> {
        if let Some(p) = &self.pipe {
            let out = p.output_info().format;
            let same = out.sample_rate == format.sample_rate && out.channels == format.channels;
            if same || !self.deps.sink_factory.can_follow_source() {
                return Ok(());
            }
        }
        if let Some(mut old) = self.pipe.take() {
            self.underruns_base += old.shared.underruns.load(Ordering::Relaxed);
            old.stop();
        }
        let sink = self.deps.sink_factory.open(format).map_err(|e| {
            tracing::error!(error = %e, "could not open the audio output");
            "could not open the audio output".to_owned()
        })?;
        let pipe = Pipeline::start(sink, &self.shared.pos.played, &self.gain).map_err(|e| {
            tracing::error!(error = %e, "could not start the audio output");
            "could not start the audio output".to_owned()
        })?;
        self.shared
            .pos
            .rate
            .store(pipe.output_info().format.sample_rate, Ordering::SeqCst);
        pipe.send(FeedCmd::SetSkipping(self.skip));
        self.pipe = Some(pipe);
        Ok(())
    }

    fn load_current(&mut self, prepared: Prepared) {
        let format = prepared.decoder.info().format;
        if let Err(message) = self.ensure_pipeline(format) {
            self.fail_current(&message);
            return;
        }
        let (Some(pipe), Some(cur)) = (self.pipe.as_ref(), self.cur.as_mut()) else {
            return;
        };
        cur.stages = plan_stages(format, pipe.output_info().format);
        cur.duration = prepared.duration().or(cur.track.duration);
        cur.source = Some(prepared.info);
        cur.loaded = true;
        pipe.send(FeedCmd::Load(Job {
            token: cur.token,
            decoder: prepared.decoder,
            start: cur.start,
            span: prepared.playback.span,
            segments: Vec::new(),
        }));
        for id in self.ignored.get(&cur.entry).into_iter().flatten() {
            pipe.send(FeedCmd::Ignore {
                token: cur.token,
                id: id.clone(),
            });
        }
        let duration = cur.duration;
        self.shared
            .pos
            .duration_us
            .store(duration.map_or(0, micros), Ordering::SeqCst);
        if let Some(d) = duration {
            self.notify(&PlayerEvent::DurationKnown(d));
        }
        if let Some(path) = self.signal_path() {
            self.notify(&PlayerEvent::SignalPathChanged(path));
        }
        self.fetch_segments();
        self.prefetch_next();
    }

    /// Ask for SponsorBlock segments of the current and following tracks.
    pub fn fetch_segments(&mut self) {
        let Some(source) = self.deps.segments.clone().filter(|_| self.skip) else {
            return;
        };
        for run in self.cur.iter_mut().chain(self.following.iter_mut()) {
            let (Some(video), Some(duration)) = (run.track.video_id.clone(), run.duration) else {
                continue;
            };
            if run.loaded && !run.segments_requested {
                run.segments_requested = true;
                spawn_segments(
                    Arc::clone(&source),
                    self.tx.clone(),
                    run.token,
                    video,
                    duration,
                );
            }
        }
    }
}

/// Microseconds in a duration, saturating.
pub(crate) fn micros(d: Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX)
}
