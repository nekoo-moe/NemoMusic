//! The secret store contract every crate uses for credentials (ADR 0004, Decision 3).
//!
//! The platform stores live elsewhere; this module has no I/O. A store is a flat map from a
//! short ASCII key to at most [`MAX_SECRET_BYTES`] bytes.

/// Most bytes one value may hold. Windows allows 2560 per blob; this leaves a margin.
pub const MAX_SECRET_BYTES: usize = 2000;

/// Longest key, in bytes.
const MAX_KEY_BYTES: usize = 96;

/// A secret store failed. Texts are fixed: platform errors can name the credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct StoreError(pub &'static str);

/// Whether `key` is a valid store key: ASCII `[a-z0-9/_-]`, 1 to 96 bytes, written as
/// `<crate>/<purpose>[/<account>]`.
#[must_use]
pub fn is_valid_key(key: &str) -> bool {
    (1..=MAX_KEY_BYTES).contains(&key.len())
        && key.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'/' | b'_' | b'-')
        })
}

/// A flat key to bytes store. The real one is the OS store; tests use [`MemoryStore`].
pub trait SecretStore: Send + Sync {
    /// Write or replace `key`.
    ///
    /// # Errors
    /// Returns a [`StoreError`] when the store refuses the write.
    fn set(&self, key: &str, value: &[u8]) -> Result<(), StoreError>;

    /// Read `key`; `None` when it does not exist.
    ///
    /// # Errors
    /// Returns a [`StoreError`] when the store cannot be read.
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError>;

    /// Delete `key`; returns whether it existed.
    ///
    /// # Errors
    /// Returns a [`StoreError`] when the store refuses the delete.
    fn delete(&self, key: &str) -> Result<bool, StoreError>;
}

#[cfg(feature = "test-util")]
pub use memory::MemoryStore;

#[cfg(feature = "test-util")]
mod memory {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, MutexGuard, PoisonError};

    use super::{MAX_SECRET_BYTES, SecretStore, StoreError, is_valid_key};

    /// Map-backed store for tests (feature `test-util`). Enforces the same limits as the real
    /// stores: a key must pass [`is_valid_key`] and a value holds at most [`MAX_SECRET_BYTES`].
    #[derive(Debug, Default)]
    pub struct MemoryStore {
        entries: Mutex<BTreeMap<String, Vec<u8>>>,
        fail_writes: AtomicBool,
    }

    impl MemoryStore {
        /// Make every later `set` fail (`true`) or succeed again (`false`).
        pub fn fail_writes(&self, fail: bool) {
            self.fail_writes.store(fail, Ordering::SeqCst);
        }

        /// How many entries the store holds.
        #[must_use]
        pub fn len(&self) -> usize {
            self.entries().len()
        }

        /// Whether the store holds no entry.
        #[must_use]
        pub fn is_empty(&self) -> bool {
            self.entries().is_empty()
        }

        fn entries(&self) -> MutexGuard<'_, BTreeMap<String, Vec<u8>>> {
            // A test that panicked while holding the lock leaves a usable map.
            self.entries.lock().unwrap_or_else(PoisonError::into_inner)
        }
    }

    impl SecretStore for MemoryStore {
        fn set(&self, key: &str, value: &[u8]) -> Result<(), StoreError> {
            if !is_valid_key(key) {
                return Err(StoreError("the key is not valid"));
            }
            if value.len() > MAX_SECRET_BYTES {
                return Err(StoreError("the value is too large"));
            }
            if self.fail_writes.load(Ordering::SeqCst) {
                return Err(StoreError("write refused"));
            }
            self.entries().insert(key.to_owned(), value.to_vec());
            Ok(())
        }

        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
            Ok(self.entries().get(key).cloned())
        }

        fn delete(&self, key: &str) -> Result<bool, StoreError> {
            Ok(self.entries().remove(key).is_some())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_keys_are_accepted() {
        for key in [
            "session-manifest",
            "session-0",
            "ytm/acct-1/part-0",
            "a",
            "x_y",
        ] {
            assert!(is_valid_key(key), "{key}");
        }
        assert!(is_valid_key(&"a".repeat(96)));
    }

    #[test]
    fn invalid_keys_are_refused() {
        let long = "a".repeat(97);
        for key in ["", long.as_str(), "Session", "a b", "a.b", "ä"] {
            assert!(!is_valid_key(key), "{key:?}");
        }
    }

    #[cfg(feature = "test-util")]
    #[test]
    fn memory_store_refuses_a_value_over_the_limit() {
        let store = MemoryStore::default();
        assert!(store.set("k", &[0; MAX_SECRET_BYTES + 1]).is_err());
        assert!(store.is_empty());
        store
            .set("k", &[0; MAX_SECRET_BYTES])
            .expect("at the limit");
        assert_eq!(store.len(), 1);
    }

    #[cfg(feature = "test-util")]
    #[test]
    fn memory_store_round_trips_and_refuses_bad_keys() {
        let store = MemoryStore::default();
        assert!(store.set("Bad.Key", b"v").is_err());
        store.set("ytm/acct-1/part-0", b"v").expect("set");
        assert_eq!(store.get("ytm/acct-1/part-0"), Ok(Some(b"v".to_vec())));
        store.fail_writes(true);
        assert!(store.set("other", b"v").is_err());
        assert_eq!(store.delete("ytm/acct-1/part-0"), Ok(true));
        assert_eq!(store.delete("ytm/acct-1/part-0"), Ok(false));
    }
}
