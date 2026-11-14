//! The Secret Service (GNOME Keyring, `KWallet`) on Linux.

use std::collections::HashMap;
use std::sync::Arc;

use keyring_core::Entry;
use keyring_core::api::CredentialStoreApi;
use nemo_core::secret_store::{SecretStore, StoreError};

use crate::{SERVICE, entry_delete, entry_get, entry_set};

/// The Secret Service (GNOME Keyring, `KWallet`) through `keyring-core`.
pub struct SecretServiceStore {
    store: Arc<zbus_secret_service_keyring_store::Store>,
}

impl SecretServiceStore {
    /// Connect to the Secret Service on the session bus.
    ///
    /// # Errors
    /// Returns a [`StoreError`] when no Secret Service is running. Nothing is written to a file
    /// instead.
    pub fn new() -> Result<Self, StoreError> {
        let store = zbus_secret_service_keyring_store::Store::new()
            .map_err(|_| StoreError("the system keyring (Secret Service) is not available"))?;
        Ok(Self { store })
    }

    fn entry(&self, key: &str) -> Result<Entry, StoreError> {
        let modifiers = HashMap::from([("label", "NemoMusic sign-in")]);
        self.store
            .build(SERVICE, key, Some(&modifiers))
            .map_err(|_| StoreError("credential entry is not valid"))
    }
}

impl SecretStore for SecretServiceStore {
    fn set(&self, key: &str, value: &[u8]) -> Result<(), StoreError> {
        entry_set(&self.entry(key)?, value)
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        entry_get(&self.entry(key)?)
    }

    fn delete(&self, key: &str) -> Result<bool, StoreError> {
        entry_delete(&self.entry(key)?)
    }
}

#[cfg(test)]
mod tests {
    use nemo_core::secret_store::MAX_SECRET_BYTES;

    use super::*;

    #[test]
    #[ignore = "writes a synthetic entry to the real Secret Service keyring"]
    fn secret_service_round_trip() {
        let store = SecretServiceStore::new().expect("store");
        let key = "nemo-secrets/round-trip-test";
        let value = vec![b'v'; MAX_SECRET_BYTES];
        store.set(key, &value).expect("set");
        assert_eq!(store.get(key).expect("get"), Some(value));
        assert!(store.delete(key).expect("delete"));
        assert_eq!(store.get(key).expect("get"), None);
        assert!(!store.delete(key).expect("delete again"));
    }
}
