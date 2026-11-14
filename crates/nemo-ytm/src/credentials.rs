//! How the signed-in session is kept in the OS credential store (ADR 0001). The store itself
//! (Windows Credential Manager, or the Secret Service on Linux) lives in `nemo-secrets`;
//! `nemo-app` hands it in as `Arc<dyn SecretStore>`.
//!
//! One credential blob holds at most 2560 bytes and a cookie header can be longer, so the header
//! is cut into parts of at most [`PART_BYTES`] bytes, one entry per part, plus a manifest entry
//! that says how many parts there are. Nothing here logs a value.

use std::sync::Arc;

use nemo_core::secret_store::{SecretStore, StoreError};

/// Biggest part, in bytes. Windows allows 2560 per blob; this leaves a wide margin.
pub const PART_BYTES: usize = 2000;
/// Most parts a session may have (about 32 KB, far above any real cookie header).
pub const MAX_PARTS: usize = 16;

const MANIFEST_KEY: &str = "session-manifest";

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
            // Best effort: remove the half-written parts. The write error is the one to report.
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
}
