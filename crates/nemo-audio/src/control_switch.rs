//! The old output of a device switch: stop it, then read what it leaves behind.

use std::sync::atomic::Ordering;
use std::time::Duration;

use nemo_core::player::PlayState;

use crate::control::Core;
use crate::pipeline::{FeedEvent, Job, Pipeline};

/// What a stopped output leaves behind for the new one.
pub(crate) struct Taken {
    /// The songs with their open decoders, the current one first.
    pub jobs: Vec<Job>,
    /// How far the current song was played when the device stopped.
    pub pos: Duration,
    /// The current song's decoder is gone, so the song must be opened again at `pos`.
    pub reopen: bool,
}

impl Core {
    /// Stop `old` and collect its songs and the position the device had reached.
    pub(crate) fn take_old_output(&mut self, mut old: Pipeline) -> Taken {
        // Stop the device first: while it runs it keeps playing frames that a position read
        // would not count, and the new device would play them again.
        old.halt_output();
        self.underruns_base += old.shared.underruns.load(Ordering::Relaxed);
        let (mut jobs, events) = old.stop_and_take_with_events();
        // A song that began a moment ago may have its `Started` report unread. Without it the
        // position is unknown (counted as zero) and the new device would start the song over.
        for event in events {
            if let FeedEvent::Started {
                token,
                at_frame,
                base,
            } = event
            {
                self.on_started(token, at_frame, base);
            }
        }
        let pos = self.shared.position();
        // When the current song is not among the jobs, the feeder has decoded all of it and
        // what is left lies in the old ring, which cannot move. The next song's decoder must
        // not be moved to this song's position.
        let reopen = self
            .cur
            .as_ref()
            .is_some_and(|c| c.loaded && !jobs.iter().any(|j| j.token == c.token));
        if reopen {
            jobs.clear();
        }
        Taken { jobs, pos, reopen }
    }

    /// Open the current song again at `pos`, keeping a pause.
    pub(crate) fn reopen_current(&mut self, pos: Duration) {
        let Some(index) = self.index.filter(|_| !self.parked) else {
            return;
        };
        let paused = self.state == PlayState::Paused;
        self.start_track(index, pos);
        if paused {
            self.pause();
        }
    }
}
