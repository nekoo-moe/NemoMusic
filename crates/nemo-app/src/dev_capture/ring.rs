//! The bounded in-memory ring of captured events.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tracing::Level;

/// How many events the ring keeps; the oldest are dropped first.
pub const RING_CAPACITY: usize = 2000;

/// One captured event. Every text in it is already redacted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedEvent {
    /// Wall-clock time, milliseconds since the Unix epoch.
    pub at_ms: u64,
    /// The event level.
    pub level: Level,
    /// The `tracing` target (a module path or `nemo_trace`).
    pub target: String,
    /// The log message.
    pub message: String,
    /// The allow-listed structured fields, in the order they were recorded.
    pub fields: Vec<(&'static str, String)>,
}

impl CapturedEvent {
    /// The value of the field `name`, if the event has it.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Debug)]
struct Shared {
    events: Mutex<VecDeque<CapturedEvent>>,
    capacity: usize,
    /// Counts every push and clear, so a reader can tell that the ring changed.
    generation: AtomicU64,
}

/// A cheap-to-clone handle to the ring. Memory only: nothing here is written anywhere.
#[derive(Debug, Clone)]
pub struct DevRing {
    shared: Arc<Shared>,
}

impl DevRing {
    /// An empty ring that keeps at most `capacity` events (at least 1).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            shared: Arc::new(Shared {
                events: Mutex::new(VecDeque::with_capacity(capacity.min(256))),
                capacity,
                generation: AtomicU64::new(0),
            }),
        }
    }

    // The ring holds plain data, so a panic in another thread cannot leave it half-written.
    fn lock(&self) -> MutexGuard<'_, VecDeque<CapturedEvent>> {
        self.shared
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Add an event; the oldest one goes when the ring is full.
    pub fn push(&self, event: CapturedEvent) {
        let mut events = self.lock();
        while events.len() >= self.shared.capacity {
            events.pop_front();
        }
        events.push_back(event);
        drop(events);
        self.shared.generation.fetch_add(1, Ordering::Release);
    }

    /// A copy of the events, oldest first.
    #[must_use]
    pub fn snapshot(&self) -> Vec<CapturedEvent> {
        self.lock().iter().cloned().collect()
    }

    /// Empty the ring.
    pub fn clear(&self) {
        self.lock().clear();
        self.shared.generation.fetch_add(1, Ordering::Release);
    }

    /// How many events the ring holds now.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether the ring holds no event.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// A number that changes whenever the ring changes.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.shared.generation.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(n: u64) -> CapturedEvent {
        CapturedEvent {
            at_ms: n,
            level: Level::INFO,
            target: "t".to_owned(),
            message: format!("m{n}"),
            fields: vec![("op", n.to_string())],
        }
    }

    #[test]
    fn the_ring_drops_the_oldest_events_at_its_bound() {
        let ring = DevRing::new(3);
        for n in 0..10 {
            ring.push(event(n));
        }
        let kept: Vec<u64> = ring.snapshot().iter().map(|e| e.at_ms).collect();
        assert_eq!(kept, vec![7, 8, 9]);
        assert_eq!(ring.len(), 3);
    }

    #[test]
    fn the_default_capacity_is_about_two_thousand() {
        let ring = DevRing::new(RING_CAPACITY);
        for n in 0..(RING_CAPACITY as u64 + 50) {
            ring.push(event(n));
        }
        let snap = ring.snapshot();
        assert_eq!(snap.len(), RING_CAPACITY);
        assert_eq!(snap.first().map(|e| e.at_ms), Some(50));
        assert!(snap.windows(2).all(|w| w[0].at_ms < w[1].at_ms));
    }

    #[test]
    fn clear_empties_the_ring_and_changes_the_generation() {
        let ring = DevRing::new(4);
        ring.push(event(1));
        let before = ring.generation();
        ring.clear();
        assert!(ring.is_empty());
        assert_ne!(ring.generation(), before);
    }

    #[test]
    fn clones_share_one_ring() {
        let ring = DevRing::new(4);
        let other = ring.clone();
        other.push(event(1));
        assert_eq!(ring.len(), 1);
    }

    #[test]
    fn a_zero_capacity_still_keeps_one_event() {
        let ring = DevRing::new(0);
        ring.push(event(1));
        ring.push(event(2));
        assert_eq!(ring.snapshot().len(), 1);
    }

    #[test]
    fn field_finds_a_value_by_name() {
        assert_eq!(event(5).field("op"), Some("5"));
        assert_eq!(event(5).field("host"), None);
    }
}
