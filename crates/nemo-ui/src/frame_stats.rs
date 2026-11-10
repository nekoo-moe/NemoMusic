//! Frame-interval statistics for the F3 readout (carried over from the glass spike). Pure.
//!
//! It measures the time between redraws, not GPU time.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Intervals older than this are dropped.
const WINDOW: Duration = Duration::from_secs(1);

/// Rolling frame-interval statistics over the last second.
#[derive(Debug, Clone, Default)]
pub struct FrameStats {
    frames: VecDeque<(Instant, f32)>,
    last: Option<Instant>,
}

/// A snapshot of the statistics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Readout {
    /// Average interval in ms.
    pub avg_ms: f32,
    /// Worst interval in ms.
    pub worst_ms: f32,
    /// Frames in the last second.
    pub frames: usize,
}

impl FrameStats {
    /// Record a frame at `now`. A gap longer than a second (the UI was idle) is not an interval.
    pub fn record(&mut self, now: Instant) {
        if let Some(prev) = self.last {
            let dt = now.saturating_duration_since(prev);
            if dt < WINDOW {
                self.frames.push_back((now, dt.as_secs_f32() * 1000.0));
            } else {
                self.frames.clear();
            }
        }
        self.last = Some(now);
        while self
            .frames
            .front()
            .is_some_and(|(t, _)| now.saturating_duration_since(*t) > WINDOW)
        {
            self.frames.pop_front();
        }
    }

    /// The current numbers, or `None` before two frames.
    #[must_use]
    pub fn readout(&self) -> Option<Readout> {
        if self.frames.is_empty() {
            return None;
        }
        let sum: f32 = self.frames.iter().map(|(_, ms)| ms).sum();
        let worst = self.frames.iter().map(|(_, ms)| *ms).fold(0.0, f32::max);
        // At most about a hundred entries, so the conversion is exact.
        #[allow(clippy::cast_precision_loss)]
        let avg = sum / self.frames.len() as f32;
        Some(Readout {
            avg_ms: avg,
            worst_ms: worst,
            frames: self.frames.len(),
        })
    }
}

impl Readout {
    /// `16.7 ms (60 fps), worst 18.2 ms`.
    #[must_use]
    pub fn text(&self) -> String {
        let fps = if self.avg_ms > 0.0 {
            1000.0 / self.avg_ms
        } else {
            0.0
        };
        format!(
            "{:.1}\u{2009}ms ({fps:.0}\u{2009}fps) \u{b7} worst {:.1}\u{2009}ms \u{b7} {} frames/s",
            self.avg_ms, self.worst_ms, self.frames
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_two_frames() {
        let mut s = FrameStats::default();
        assert!(s.readout().is_none());
        s.record(Instant::now());
        assert!(s.readout().is_none());
    }

    #[test]
    fn averages_and_tracks_the_worst() {
        let mut s = FrameStats::default();
        let t0 = Instant::now();
        for (i, ms) in [0_u64, 16, 33, 50, 100].into_iter().enumerate() {
            let _ = i;
            s.record(t0 + Duration::from_millis(ms));
        }
        let r = s.readout().expect("readout");
        assert_eq!(r.frames, 4);
        assert!((r.worst_ms - 50.0).abs() < 0.5);
        assert!((r.avg_ms - 25.0).abs() < 0.5);
        assert!(r.text().contains("worst 50.0"));
    }

    #[test]
    fn idle_gaps_are_not_counted_and_old_frames_expire() {
        let mut s = FrameStats::default();
        let t0 = Instant::now();
        s.record(t0);
        s.record(t0 + Duration::from_millis(16));
        s.record(t0 + Duration::from_secs(30));
        assert!(
            s.readout().is_none(),
            "a 30 s gap is idle, not a slow frame"
        );
        s.record(t0 + Duration::from_secs(30) + Duration::from_millis(16));
        let r = s.readout().expect("readout");
        assert!(r.worst_ms < 20.0);
        for i in 0..200 {
            s.record(t0 + Duration::from_secs(31) + Duration::from_millis(16 * i));
        }
        assert!(s.readout().expect("readout").frames <= 64);
    }
}
