//! Removing a SoundCloud sign-in while SoundCloud is off (owner decision, 2026-11-20: off also
//! signs out and wipes the token).
//!
//! While the switch is off no account exists (the services are not built), so a token kept from
//! an earlier session has no [`crate::ScAccount`] to sign out. This module is the store-only
//! wipe: it needs only the credential store and the folders of [`AccountFiles`]. `nemo-app`
//! runs it at start when SoundCloud is off and something may be left, and the account leaves a
//! small marker file when a sign-out could not delete the token, so the next start tries again.
//! Nothing here reads or logs the token.

use std::path::PathBuf;

use nemo_core::account::SignOutReport;
use nemo_core::secret_store::SecretStore;
use nemo_core::source_account::SourceAccountError;

use crate::account::{AccountFiles, TOKEN_KEY};

/// The marker file that says a token may still be in the credential store.
fn marker(files: &AccountFiles) -> PathBuf {
    files.cache_file.with_extension("wipe")
}

/// Record that the token could not be deleted, so the next start with SoundCloud off retries.
/// A failure to write the marker is ignored: there is nothing else to try.
pub(crate) fn mark_pending(files: &AccountFiles) {
    if let Some(dir) = marker(files).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(marker(files), b"");
}

/// Forget the marker after the token was deleted.
pub(crate) fn clear_pending(files: &AccountFiles) {
    let _ = std::fs::remove_file(marker(files));
}

/// Whether a sign-in may have left something behind: the cached name, the window's profile
/// folder or the retry marker. The credential store is not asked, so an app that never signed in
/// never touches the OS keyring (which may prompt to unlock) at start.
#[must_use]
pub fn has_leftovers(files: &AccountFiles) -> bool {
    files.cache_file.exists() || files.webview_dir.exists() || marker(files).exists()
}

/// Delete the stored token, the cached name and avatar and the window's profile folder.
///
/// The token goes first. When the store cannot delete it, the folders stay (the cached file is
/// the reminder to try again) and the marker is written.
///
/// # Errors
/// [`SourceAccountError::Storage`] when the credential store cannot delete the token.
pub fn wipe_stored(
    store: &dyn SecretStore,
    files: &AccountFiles,
) -> Result<SignOutReport, SourceAccountError> {
    let mut report = SignOutReport::default();
    match store.delete(TOKEN_KEY) {
        Ok(existed) => report.credentials_removed = usize::from(existed),
        Err(e) => {
            mark_pending(files);
            return Err(SourceAccountError::Storage(e.0.to_owned()));
        }
    }
    if std::fs::remove_dir_all(&files.webview_dir).is_ok() {
        report.files_removed += 1;
    }
    if std::fs::remove_file(&files.cache_file).is_ok() {
        report.files_removed += 1;
    }
    clear_pending(files);
    Ok(report)
}

#[cfg(test)]
mod tests;
