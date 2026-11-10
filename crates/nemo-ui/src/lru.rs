//! A small least-recently-used map for decoded images. Reads take `&self` (the view only has a
//! shared reference), so the recency stamps are `Cell`s.

use std::cell::Cell;
use std::collections::HashMap;

/// Number of decoded images kept in memory.
pub const ART_CAPACITY: usize = 300;

struct Slot<V> {
    value: V,
    used: Cell<u64>,
}

/// LRU map. Entries for which `pinned` returns `true` (requests in flight) are never evicted.
pub struct Lru<V> {
    map: HashMap<String, Slot<V>>,
    tick: Cell<u64>,
    capacity: usize,
}

impl<V> Lru<V> {
    /// Empty map that keeps at most `capacity` unpinned entries.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::new(),
            tick: Cell::new(0),
            capacity: capacity.max(1),
        }
    }

    fn next(&self) -> u64 {
        let t = self.tick.get() + 1;
        self.tick.set(t);
        t
    }

    /// Look up `key` and mark it as just used.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&V> {
        let slot = self.map.get(key)?;
        slot.used.set(self.next());
        Some(&slot.value)
    }

    /// `true` when `key` is present (does not touch recency).
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// `true` when there are no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Insert or replace `key`; then evict the least recently used unpinned entries until at most
    /// `capacity` unpinned entries remain.
    pub fn insert(&mut self, key: String, value: V, pinned: impl Fn(&V) -> bool) {
        let used = Cell::new(self.next());
        self.map.insert(key, Slot { value, used });
        loop {
            let unpinned = self.map.values().filter(|s| !pinned(&s.value)).count();
            if unpinned <= self.capacity {
                break;
            }
            let oldest = self
                .map
                .iter()
                .filter(|(_, s)| !pinned(&s.value))
                .min_by_key(|(_, s)| s.used.get())
                .map(|(k, _)| k.clone());
            match oldest {
                Some(k) => {
                    self.map.remove(&k);
                }
                None => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_the_least_recently_used() {
        let mut lru = Lru::new(3);
        for (i, k) in ["a", "b", "c"].into_iter().enumerate() {
            lru.insert(k.to_owned(), i, |_| false);
        }
        assert!(lru.get("a").is_some(), "touch a so b is the oldest");
        lru.insert("d".to_owned(), 3, |_| false);
        assert_eq!(lru.len(), 3);
        assert!(!lru.contains("b") && lru.contains("a") && lru.contains("c") && lru.contains("d"));
    }

    #[test]
    fn pinned_entries_survive_and_do_not_count() {
        let mut lru = Lru::new(2);
        let pinned = |v: &i32| *v < 0;
        lru.insert("loading-1".to_owned(), -1, pinned);
        lru.insert("loading-2".to_owned(), -2, pinned);
        for i in 0..5 {
            lru.insert(format!("k{i}"), i, pinned);
        }
        assert!(lru.contains("loading-1") && lru.contains("loading-2"));
        assert_eq!(lru.len(), 4, "2 pinned + 2 kept");
    }

    #[test]
    fn capacity_holds_at_the_art_limit() {
        let mut lru = Lru::new(ART_CAPACITY);
        for i in 0..(ART_CAPACITY + 50) {
            lru.insert(format!("k{i}"), i, |_| false);
        }
        assert_eq!(lru.len(), ART_CAPACITY);
        assert!(lru.contains(&format!("k{}", ART_CAPACITY + 49)));
        assert!(!lru.contains("k0"));
    }
}
