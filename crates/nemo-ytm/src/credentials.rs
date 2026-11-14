//! Where the signed-in session is kept: Windows Credential Manager on Windows, the Secret
//! Service (GNOME Keyring, `KWallet`) on Linux (ADR 0001).
//!
//! One credential blob holds at most 2560 bytes and a cookie header can be longer, so the header
//! is cut into parts of at most [`PART_BYTES`] bytes, one entry per part, plus a manifest entry
//! that says how many parts there are. Nothing here logs a value.

use std::collections::HashMap;
use std::sync::Arc;

use keyring_core::api::CredentialStoreApi;
use keyring_core::{Entry, Error as KeyringError};
use nemo_core::secret_store::{SecretStore, StoreError};

/// Service name for every entry this app writes.
pub const SERVICE: &str = "NemoMusic";
/// Biggest part, in bytes. Windows allows 2560 per blob; this leaves a wide margin.
pub const PART_BYTES: usize = 2000;
/// Most parts a session may have (about 32 KB, far above any real cookie header).
pub const MAX_PARTS: usize = 16;

const MANIFEST_KEY: &str = "session-manifest";

/// The credential store of this platform.
#[cfg(windows)]
pub type PlatformStore = WindowsStore;
/// The credential store of this platform.
#[cfg(target_os = "linux")]
pub type PlatformStore = SecretServiceStore;

/// Windows Credential Manager through `keyring-core`.
#[cfg(windows)]
pub struct WindowsStore {
    store: Arc<windows_native_keyring_store::Store>,
}

#[cfg(windows)]
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

#[cfg(windows)]
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

/// The Secret Service (GNOME Keyring, `KWallet`) through `keyring-core`.
#[cfg(target_os = "linux")]
pub struct SecretServiceStore {
    store: Arc<zbus_secret_service_keyring_store::Store>,
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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

/// Fixed error texts of one store. Platform errors are dropped: they can name the credential.
struct Texts {
    write: &'static str,
    read: &'static str,
    delete: &'static str,
}

#[cfg(windows)]
const TEXTS: Texts = Texts {
    write: "cannot write to Windows Credential Manager",
    read: "cannot read Windows Credential Manager",
    delete: "cannot delete from Windows Credential Manager",
};

#[cfg(not(windows))]
const TEXTS: Texts = Texts {
    write: "cannot write to the system keyring",
    read: "cannot read the system keyring",
    delete: "cannot delete from the system keyring",
};

fn entry_set(entry: &Entry, value: &[u8]) -> Result<(), StoreError> {
    entry.set_secret(value).map_err(|_| StoreError(TEXTS.write))
}

fn entry_get(entry: &Entry) -> Result<Option<Vec<u8>>, StoreError> {
    match entry.get_secret() {
        Ok(bytes) => Ok(Some(bytes)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(_) => Err(StoreError(TEXTS.read)),
    }
}

fn entry_delete(entry: &Entry) -> Result<bool, StoreError> {
    match entry.delete_credential() {
        Ok(()) => Ok(true),
        Err(KeyringError::NoEntry) => Ok(false),
        Err(_) => Err(StoreError(TEXTS.delete)),
    }
}

fn part_key(index: usize) -> String {
    format!("session-{index}")
}

/// Cut `text` into parts of at most `max` bytes without splitting a character.
#[must_use]
pub fn split_parts(text: &str, max: usize) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let mut end = max.min(rest.len());
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        if end == 0 {
            // `max` is smaller than the next character: take that character whole.
            end = rest.chars().next().map_or(rest.len(), char::len_utf8);
        }
        let (head, tail) = rest.split_at(end);
        parts.push(head);
        rest = tail;
    }
    parts
}

/// The session stored as parts plus a manifest.
pub struct Vault {
    store: Arc<dyn SecretStore>,
}

impl Vault {
    /// Use `store` for all entries.
    #[must_use]
    pub fn new(store: Arc<dyn SecretStore>) -> Self {
        Self { store }
    }

    /// Save the cookie header, replacing any earlier one.
    ///
    /// # Errors
    /// Returns a [`StoreError`] when the header is too large or a write fails. A failed save
    /// removes what it wrote.
    pub fn save(&self, cookie: &str) -> Result<(), StoreError> {
        let parts = split_parts(cookie, PART_BYTES);
        if parts.is_empty() || parts.len() > MAX_PARTS {
            return Err(StoreError("the sign-in does not fit the credential store"));
        }
        let written = parts
            .iter()
            .enumerate()
            .try_for_each(|(i, part)| self.store.set(&part_key(i), part.as_bytes()))
            .and_then(|()| {
                self.store
                    .set(MANIFEST_KEY, parts.len().to_string().as_bytes())
            });
        if let Err(err) = written {
            let _ = self.wipe();
            return Err(err);
        }
        // Parts left over from a longer earlier session.
        for index in parts.len()..MAX_PARTS {
            self.store.delete(&part_key(index))?;
        }
        Ok(())
    }

    /// Read the saved cookie header; `None` when nothing (or something unusable) is saved.
    ///
    /// # Errors
    /// Returns a [`StoreError`] when the store cannot be read.
    pub fn load(&self) -> Result<Option<String>, StoreError> {
        let Some(manifest) = self.store.get(MANIFEST_KEY)? else {
            return Ok(None);
        };
        let count = std::str::from_utf8(&manifest)
            .ok()
            .and_then(|t| t.trim().parse::<usize>().ok())
            .filter(|n| (1..=MAX_PARTS).contains(n));
        let Some(count) = count else {
            return Ok(None);
        };
        let mut joined = Vec::new();
        for index in 0..count {
            match self.store.get(&part_key(index))? {
                Some(bytes) => joined.extend_from_slice(&bytes),
                None => return Ok(None),
            }
        }
        Ok(String::from_utf8(joined).ok())
    }

    /// Delete every entry (all possible parts and the manifest). Returns how many existed.
    ///
    /// # Errors
    /// Returns the first [`StoreError`]; the other entries are still tried.
    pub fn wipe(&self) -> Result<usize, StoreError> {
        let mut removed = 0;
        let mut first_error = None;
        let keys = (0..MAX_PARTS)
            .map(part_key)
            .chain(std::iter::once(MANIFEST_KEY.to_owned()));
        for key in keys {
            match self.store.delete(&key) {
                Ok(true) => removed += 1,
                Ok(false) => {}
                Err(err) => first_error = first_error.or(Some(err)),
            }
        }
        first_error.map_or(Ok(removed), Err)
    }
}

#[cfg(test)]
#[allow(
    clippy::assert_is_empty,
    clippy::needless_pass_by_value,
    reason = "readable test assertions and helpers"
)]
mod tests {
    use nemo_core::secret_store::MemoryStore;

    use super::*;

    fn vault() -> (Vault, Arc<MemoryStore>) {
        let store = Arc::new(MemoryStore::default());
        (Vault::new(store.clone()), store)
    }

    #[test]
    fn split_keeps_every_byte_and_character() {
        let text = "aé".repeat(1500);
        for max in [1, 2, 3, 7, 2000] {
            let parts = split_parts(&text, max);
            assert!(parts.iter().all(|p| p.len() <= max.max(2)), "{max}");
            assert_eq!(parts.concat(), text);
        }
        assert!(split_parts("", 10).is_empty());
    }

    #[test]
    fn a_long_cookie_round_trips_in_several_entries() {
        let (vault, store) = vault();
        let cookie = format!("SAPISID=x; big={}", "v".repeat(5000));
        vault.save(&cookie).expect("save");
        assert_eq!(store.len(), 1 + 3);
        assert_eq!(
            vault.load().expect("load").as_deref(),
            Some(cookie.as_str())
        );
    }

    #[test]
    fn saving_a_shorter_cookie_drops_stale_parts() {
        let (vault, store) = vault();
        vault.save(&"a".repeat(4500)).expect("save long");
        vault.save("SAPISID=x;").expect("save short");
        assert_eq!(store.len(), 2);
        assert_eq!(vault.load().expect("load").as_deref(), Some("SAPISID=x;"));
    }

    #[test]
    fn nothing_saved_loads_as_none() {
        let (vault, _) = vault();
        assert_eq!(vault.load().expect("load"), None);
    }

    #[test]
    fn a_missing_part_or_bad_manifest_loads_as_none() {
        let (vault, store) = vault();
        vault.save(&"a".repeat(4500)).expect("save");
        store.delete("session-1").expect("delete");
        assert_eq!(vault.load().expect("load"), None);
        store.set(MANIFEST_KEY, b"99").expect("set");
        assert_eq!(vault.load().expect("load"), None);
    }

    #[test]
    fn a_failed_save_leaves_nothing_behind() {
        let (vault, store) = vault();
        vault.save("SAPISID=x;").expect("first");
        store.fail_writes(true);
        assert!(vault.save("SAPISID=y;").is_err());
        store.fail_writes(false);
        assert!(store.is_empty());
    }

    #[test]
    fn wipe_removes_everything_and_counts() {
        let (vault, store) = vault();
        vault.save(&"a".repeat(4500)).expect("save");
        assert_eq!(vault.wipe().expect("wipe"), 4);
        assert!(store.is_empty());
        assert_eq!(vault.wipe().expect("again"), 0);
    }

    #[test]
    fn empty_or_huge_cookies_are_refused() {
        let (vault, store) = vault();
        assert!(vault.save("").is_err());
        assert!(vault.save(&"a".repeat(PART_BYTES * MAX_PARTS + 1)).is_err());
        assert!(store.is_empty());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "writes synthetic entries to the real Windows Credential Manager"]
    fn windows_credential_manager_round_trip() {
        let vault = Vault::new(Arc::new(WindowsStore::new().expect("store")));
        let cookie = format!("SAPISID=synthetic; filler={}", "v".repeat(4500));
        vault.save(&cookie).expect("save");
        assert_eq!(
            vault.load().expect("load").as_deref(),
            Some(cookie.as_str())
        );
        assert_eq!(vault.wipe().expect("wipe"), 4);
        assert_eq!(vault.load().expect("load"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "writes synthetic entries to the real Secret Service keyring"]
    fn secret_service_round_trip() {
        let vault = Vault::new(Arc::new(SecretServiceStore::new().expect("store")));
        let cookie = format!("SAPISID=synthetic; filler={}", "v".repeat(4500));
        vault.save(&cookie).expect("save");
        assert_eq!(
            vault.load().expect("load").as_deref(),
            Some(cookie.as_str())
        );
        assert_eq!(vault.wipe().expect("wipe"), 4);
        assert_eq!(vault.load().expect("load"), None);
    }
}
