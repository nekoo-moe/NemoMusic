//! The OS credential store behind the `nemo-core` [`SecretStore`] trait (ADR 0004,
//! Decision 1): Windows Credential Manager on Windows, the Secret Service (GNOME Keyring,
//! `KWallet`) on Linux.
//!
//! Every crate that needs a secret gets the store as `Arc<dyn SecretStore>` from `nemo-app`,
//! which opens it with [`open_platform_store`]. This crate depends on `nemo-core` only. Nothing
//! here logs a value, and every error text is fixed: platform errors can name the credential.

use std::sync::Arc;

use nemo_core::secret_store::{SecretStore, StoreError};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::SecretServiceStore;
#[cfg(windows)]
pub use windows::WindowsStore;

/// Service name for every entry this app writes.
pub const SERVICE: &str = "NemoMusic";

/// Open the credential store of this platform.
///
/// # Errors
/// Returns a [`StoreError`] when the store cannot be opened. Nothing is written to a file
/// instead.
#[cfg(windows)]
pub fn open_platform_store() -> Result<Arc<dyn SecretStore>, StoreError> {
    Ok(Arc::new(WindowsStore::new()?))
}

/// Open the credential store of this platform.
///
/// # Errors
/// Returns a [`StoreError`] when no Secret Service is running. Nothing is written to a file
/// instead.
#[cfg(target_os = "linux")]
pub fn open_platform_store() -> Result<Arc<dyn SecretStore>, StoreError> {
    Ok(Arc::new(SecretServiceStore::new()?))
}

/// Open the credential store of this platform.
///
/// # Errors
/// Always: this platform has no supported credential store.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn open_platform_store() -> Result<Arc<dyn SecretStore>, StoreError> {
    Err(StoreError("no credential store on this system"))
}

#[cfg(any(windows, target_os = "linux"))]
use keyring_core::{Entry, Error as KeyringError};

/// Fixed error texts of one store. Platform errors are dropped: they can name the credential.
#[cfg(any(windows, target_os = "linux"))]
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

#[cfg(target_os = "linux")]
const TEXTS: Texts = Texts {
    write: "cannot write to the system keyring",
    read: "cannot read the system keyring",
    delete: "cannot delete from the system keyring",
};

#[cfg(any(windows, target_os = "linux"))]
fn entry_set(entry: &Entry, value: &[u8]) -> Result<(), StoreError> {
    entry.set_secret(value).map_err(|_| StoreError(TEXTS.write))
}

#[cfg(any(windows, target_os = "linux"))]
fn entry_get(entry: &Entry) -> Result<Option<Vec<u8>>, StoreError> {
    match entry.get_secret() {
        Ok(bytes) => Ok(Some(bytes)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(_) => Err(StoreError(TEXTS.read)),
    }
}

#[cfg(any(windows, target_os = "linux"))]
fn entry_delete(entry: &Entry) -> Result<bool, StoreError> {
    match entry.delete_credential() {
        Ok(()) => Ok(true),
        Err(KeyringError::NoEntry) => Ok(false),
        Err(_) => Err(StoreError(TEXTS.delete)),
    }
}
