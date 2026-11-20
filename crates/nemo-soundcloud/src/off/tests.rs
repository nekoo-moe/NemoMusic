//! The store-only wipe against a fake credential store and a temporary folder.

use nemo_core::secret_store::{MemoryStore, StoreError};

use super::*;

const TOKEN: &str = "2-293232-7000001-SyntheticToken";

fn files(name: &str) -> AccountFiles {
    let dir = std::env::temp_dir().join(format!("nemo-sc-off-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    AccountFiles {
        webview_dir: dir.join("webview"),
        cache_file: dir.join("account").join("soundcloud.json"),
    }
}

fn leave_things(files: &AccountFiles) {
    std::fs::create_dir_all(files.webview_dir.join("Default")).expect("dir");
    std::fs::write(files.webview_dir.join("Default").join("Cookies"), "x").expect("file");
    std::fs::create_dir_all(files.cache_file.parent().expect("parent")).expect("dir");
    std::fs::write(&files.cache_file, "{}").expect("file");
}

fn clean(files: &AccountFiles) {
    if let Some(dir) = files.cache_file.parent().and_then(std::path::Path::parent) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn nothing_left_means_the_keyring_is_not_asked() {
    let files = files("none");
    assert!(!has_leftovers(&files));
    leave_things(&files);
    assert!(has_leftovers(&files));
    clean(&files);
}

#[test]
fn the_wipe_removes_the_token_the_cache_and_the_profile() {
    let files = files("wipe");
    let store = MemoryStore::default();
    store.set(TOKEN_KEY, TOKEN.as_bytes()).expect("set");
    leave_things(&files);
    let report = wipe_stored(&store, &files).expect("wiped");
    assert!(store.is_empty());
    assert!(!files.cache_file.exists() && !files.webview_dir.exists());
    assert_eq!((report.credentials_removed, report.files_removed), (1, 2));
    assert!(!has_leftovers(&files));
    // Again, with nothing left: fine, and it counts nothing.
    let again = wipe_stored(&store, &files).expect("wiped");
    assert_eq!((again.credentials_removed, again.files_removed), (0, 0));
    clean(&files);
}

#[test]
fn a_store_that_cannot_delete_keeps_the_reminder_for_the_next_start() {
    struct Stuck;
    impl SecretStore for Stuck {
        fn set(&self, _: &str, _: &[u8]) -> Result<(), StoreError> {
            Ok(())
        }
        fn get(&self, _: &str) -> Result<Option<Vec<u8>>, StoreError> {
            Ok(None)
        }
        fn delete(&self, _: &str) -> Result<bool, StoreError> {
            Err(StoreError("the store refused the delete"))
        }
    }
    let files = files("stuck");
    leave_things(&files);
    let error = wipe_stored(&Stuck, &files).expect_err("store");
    assert_eq!(
        error,
        SourceAccountError::Storage("the store refused the delete".to_owned())
    );
    assert!(has_leftovers(&files), "the next start tries again");
    assert!(files.cache_file.exists(), "kept as the reminder");
    // Once the cache file is gone the marker alone is still a reminder.
    std::fs::remove_file(&files.cache_file).expect("remove");
    std::fs::remove_dir_all(&files.webview_dir).expect("remove");
    assert!(has_leftovers(&files));
    let store = MemoryStore::default();
    wipe_stored(&store, &files).expect("wiped");
    assert!(!has_leftovers(&files));
    clean(&files);
}
