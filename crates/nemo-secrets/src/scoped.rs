//! A view of a [`SecretStore`] that sees only the keys under one prefix.

use std::sync::Arc;

use nemo_core::secret_store::{SecretStore, StoreError, is_valid_key};

/// Prefixes every key with `<prefix>/`, so a crate cannot touch another crate's entries by
/// mistake.
///
/// A key that fails [`is_valid_key`] after prefixing (too long, or with a character the store
/// does not accept) is refused before the inner store is called. The `session-*` keys of the
/// `nemo-ytm` vault stay unscoped until account-actions task 16 migrates them.
pub struct ScopedStore {
    inner: Arc<dyn SecretStore>,
    prefix: String,
}

impl ScopedStore {
    /// Wrap `inner` so every key is written as `<prefix>/<key>`.
    #[must_use]
    pub fn new(inner: Arc<dyn SecretStore>, prefix: &str) -> Self {
        Self {
            inner,
            prefix: prefix.to_owned(),
        }
    }

    /// The full key in the inner store, or an error when it is not a valid key.
    fn full_key(&self, key: &str) -> Result<String, StoreError> {
        if key.is_empty() {
            return Err(StoreError("the key is empty"));
        }
        let full = format!("{}/{key}", self.prefix);
        if is_valid_key(&full) {
            Ok(full)
        } else {
            Err(StoreError("the key is not valid after prefixing"))
        }
    }
}

impl SecretStore for ScopedStore {
    fn set(&self, key: &str, value: &[u8]) -> Result<(), StoreError> {
        self.inner.set(&self.full_key(key)?, value)
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        self.inner.get(&self.full_key(key)?)
    }

    fn delete(&self, key: &str) -> Result<bool, StoreError> {
        self.inner.delete(&self.full_key(key)?)
    }
}

#[cfg(test)]
mod tests {
    use nemo_core::secret_store::MemoryStore;

    use super::*;

    fn two_scopes() -> (Arc<MemoryStore>, ScopedStore, ScopedStore) {
        let inner = Arc::new(MemoryStore::default());
        let a = ScopedStore::new(inner.clone(), "lastfm");
        let b = ScopedStore::new(inner.clone(), "spotify");
        (inner, a, b)
    }

    #[test]
    fn two_scopes_do_not_see_each_others_keys() {
        let (inner, a, b) = two_scopes();
        a.set("session", b"one").expect("set a");
        b.set("session", b"two").expect("set b");
        assert_eq!(a.get("session"), Ok(Some(b"one".to_vec())));
        assert_eq!(b.get("session"), Ok(Some(b"two".to_vec())));
        assert_eq!(inner.get("lastfm/session"), Ok(Some(b"one".to_vec())));
        assert_eq!(inner.get("session"), Ok(None));
        assert_eq!(a.delete("session"), Ok(true));
        assert_eq!(b.get("session"), Ok(Some(b"two".to_vec())));
    }

    #[test]
    fn delete_reports_whether_the_key_existed() {
        let (_, a, _) = two_scopes();
        assert_eq!(a.delete("token"), Ok(false));
        a.set("token", b"v").expect("set");
        assert_eq!(a.delete("token"), Ok(true));
        assert_eq!(a.delete("token"), Ok(false));
    }

    #[test]
    fn a_key_too_long_after_prefixing_is_refused() {
        let (inner, a, _) = two_scopes();
        // "lastfm/" is 7 bytes, so 89 bytes is the longest key that still fits in 96.
        let fits = "k".repeat(89);
        let too_long = "k".repeat(90);
        a.set(&fits, b"v").expect("fits");
        assert!(a.set(&too_long, b"v").is_err());
        assert!(a.get(&too_long).is_err());
        assert!(a.delete(&too_long).is_err());
        assert_eq!(inner.len(), 1);
    }

    #[test]
    fn empty_and_invalid_keys_are_refused() {
        let (inner, a, _) = two_scopes();
        for key in ["", "Bad", "a b"] {
            assert!(a.set(key, b"v").is_err(), "{key:?}");
        }
        assert!(inner.is_empty());
    }
}
