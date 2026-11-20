//! The bounded in-memory ring of captured events.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use nemo_core::dev::TRACE_TARGET;
use tracing::Level;

/// How many events the ring keeps; the oldest are dropped first.
pub const RING_CAPACITY: usize = 2000;

/// How many different hosts are counted; later ones are counted together as [`OTHER_HOSTS`].
const HOSTS_MAX: usize = 64;

/// The name under which hosts beyond [`HOSTS_MAX`] are counted.
pub const OTHER_HOSTS: &str = "(other)";

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
    /// Requests per host this run, from the trace events. Not emptied by `clear`.
    hosts: Mutex<BTreeMap<String, u64>>,
    /// When the ring was made: the start of the run.
    started: Instant,
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
                hosts: Mutex::new(BTreeMap::new()),
                started: Instant::now(),
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
        if event.target == TRACE_TARGET
            && let Some(host) = event.field("host").filter(|h| !h.is_empty())
        {
            self.note_host(host);
        }
        let mut events = self.lock();
        while events.len() >= self.shared.capacity {
            events.pop_front();
        }
        events.push_back(event);
        drop(events);
        self.shared.generation.fetch_add(1, Ordering::Release);
    }

    fn note_host(&self, host: &str) {
        let mut hosts = self
            .shared
            .hosts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let key = if hosts.contains_key(host) || hosts.len() < HOSTS_MAX {
            host
        } else {
            OTHER_HOSTS
        };
        *hosts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// The requests per host seen this run, by host name.
    #[must_use]
    pub fn hosts(&self) -> Vec<(String, u64)> {
        self.shared
            .hosts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|(host, count)| (host.clone(), *count))
            .collect()
    }

    /// How long ago the ring was made, which is the age of this run.
    #[must_use]
    pub fn uptime(&self) -> Duration {
        self.shared.started.elapsed()
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
    fn hosts_are_counted_from_trace_events_and_survive_clear() {
        let ring = DevRing::new(4);
        for host in ["a.example", "b.example", "a.example"] {
            let mut e = event(1);
            e.target = TRACE_TARGET.to_owned();
            e.fields = vec![("host", host.to_owned())];
            ring.push(e);
        }
        let mut other = event(2);
        other.fields = vec![("host", "ignored.example".to_owned())];
        ring.push(other);
        ring.clear();
        assert_eq!(
            ring.hosts(),
            vec![("a.example".to_owned(), 2), ("b.example".to_owned(), 1)]
        );
    }

    #[test]
    fn hosts_beyond_the_limit_are_counted_together() {
        let ring = DevRing::new(4);
        for n in 0..(HOSTS_MAX + 3) {
            let mut e = event(1);
            e.target = TRACE_TARGET.to_owned();
            e.fields = vec![("host", format!("h{n}.example"))];
            ring.push(e);
        }
        let hosts = ring.hosts();
        assert_eq!(hosts.len(), HOSTS_MAX + 1);
        assert_eq!(
            hosts
                .iter()
                .find(|(h, _)| h == OTHER_HOSTS)
                .map(|(_, c)| *c),
            Some(3)
        );
    }

    #[test]
    fn field_finds_a_value_by_name() {
        assert_eq!(event(5).field("op"), Some("5"));
        assert_eq!(event(5).field("host"), None);
    }
}
