//! Windows Credential Manager.

use std::collections::HashMap;
use std::sync::Arc;

use keyring_core::Entry;
use keyring_core::api::CredentialStoreApi;
use nemo_core::secret_store::{SecretStore, StoreError};

use crate::{SERVICE, entry_delete, entry_get, entry_set};

/// Windows Credential Manager through `keyring-core`.
pub struct WindowsStore {
    store: Arc<windows_native_keyring_store::Store>,
}

impl WindowsStore {
    /// Open the credential store.
    ///
    /// # Errors
    /// Returns a [`StoreError`] when the store cannot be created.
    pub fn new() -> Result<Self, StoreError> {
        let store = windows_native_keyring_store::Store::new()
            .map_err(|_| StoreError("credential store is not available"))?;
        Ok(Self { store })
    }

    fn entry(&self, key: &str) -> Result<Entry, StoreError> {
        // `Local`: stays on this computer instead of roaming with a domain profile.
        let modifiers = HashMap::from([("persistence", "Local")]);
        self.store
            .build(SERVICE, key, Some(&modifiers))
            .map_err(|_| StoreError("credential entry is not valid"))
    }
}

impl SecretStore for WindowsStore {
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
    #[ignore = "writes a synthetic entry to the real Windows Credential Manager"]
    fn windows_credential_manager_round_trip() {
        let store = WindowsStore::new().expect("store");
        let key = "nemo-secrets/round-trip-test";
        let value = vec![b'v'; MAX_SECRET_BYTES];
        store.set(key, &value).expect("set");
        assert_eq!(store.get(key).expect("get"), Some(value));
        assert!(store.delete(key).expect("delete"));
        assert_eq!(store.get(key).expect("get"), None);
        assert!(!store.delete(key).expect("delete again"));
    }
}
