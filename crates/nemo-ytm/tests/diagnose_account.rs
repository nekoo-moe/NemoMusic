//! Account diagnostic (development only, needs the owner's saved session). Run by hand:
//! `cargo test -p nemo-ytm --test diagnose_account -- --ignored --nocapture`
//!
//! It loads the saved session through `GoogleAccount` (Windows Credential Manager), asks the
//! account pages and prints shapes, counts and error labels. It never prints a title, id, name,
//! URL or cookie, and never signs out or rewrites the stored session: the account header cache
//! is a throwaway file, so the "check the session" path (which wipes a rejected session) is
//! not taken.
#![allow(
    clippy::print_stderr,
    reason = "a diagnostic reports what it observed on the real network"
)]

use std::sync::{Arc, Mutex};

use nemo_core::BoxFuture;
use nemo_core::account::{AccountError, AccountInfo, AccountService, AccountState};
use nemo_core::catalog::{Catalog, FeedItem};
use nemo_core::secret::Secret;
use nemo_core::stream::StreamResolver;
use nemo_ytm::account::{AccountConfig, GoogleAccount, SessionSlot};
use nemo_ytm::credentials::PlatformStore;
use nemo_ytm::diagnostics::Diagnosis;
use nemo_ytm::session::{SessionBackend, SessionCatalog};
use nemo_ytm::ytdlp::YtDlpResolver;

/// Builds the real signed-in catalog and keeps a typed handle to it.
struct Capture {
    handle: tokio::runtime::Handle,
    caught: Arc<Mutex<Option<Arc<SessionCatalog>>>>,
}

impl SessionBackend for Capture {
    fn probe(&self, _: Secret<String>) -> BoxFuture<'_, Result<AccountInfo, AccountError>> {
        // Never reached: the throwaway cache below makes the account skip the check.
        Box::pin(async { Err(AccountError::Network("not used".to_owned())) })
    }

    fn catalog(&self, cookie: Secret<String>) -> Arc<dyn Catalog> {
        let catalog = Arc::new(SessionCatalog::new(self.handle.clone(), cookie));
        if let Ok(mut slot) = self.caught.lock() {
            *slot = Some(Arc::clone(&catalog));
        }
        catalog
    }
}

#[test]
#[ignore = "uses the saved account session and the real network"]
fn describe_the_account_pages() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    let scratch = std::env::var_os("KIROCREW_SCRATCH")
        .map_or_else(std::env::temp_dir, std::path::PathBuf::from)
        .join("nemo-diagnose");
    std::fs::create_dir_all(&scratch).expect("scratch dir");
    let cache = scratch.join("account.json");
    std::fs::write(&cache, r#"{"name":"diagnostic","handle":null,"avatar":[]}"#).expect("cache");

    let caught = Arc::new(Mutex::new(None));
    let slot: SessionSlot = Arc::default();
    let account = GoogleAccount::new(
        AccountConfig {
            helper: scratch.join("none.exe"),
            webview_dir: scratch.join("webview"),
            cache_file: cache,
        },
        Arc::new(PlatformStore::new().expect("credential store")),
        Arc::new(Capture {
            handle: runtime.handle().clone(),
            caught: Arc::clone(&caught),
        }),
        slot,
        runtime.handle().clone(),
    );
    assert!(
        matches!(account.state(), AccountState::SignedIn(_)),
        "no saved session"
    );
    let catalog = caught
        .lock()
        .expect("lock")
        .clone()
        .expect("the account built the signed-in catalog");

    let report = if std::env::var_os("NEMO_VIDEO_ONLY").is_some() {
        // Cheap re-run of the stream check: one explore page, no library calls.
        let explore = runtime.block_on(catalog.explore());
        let sample_video = explore.ok().and_then(|e| {
            e.shelves
                .iter()
                .flat_map(|s| &s.items)
                .find_map(|i| match i {
                    FeedItem::Video(t) => t.video_id.clone(),
                    _ => None,
                })
        });
        Diagnosis {
            lines: Vec::new(),
            sample_video,
        }
    } else {
        runtime.block_on(catalog.diagnose())
    };
    for line in &report.lines {
        eprintln!("{line}");
    }
    // A music video must play through the same stream path as a song.
    match report.sample_video {
        Some(video) => {
            let resolver = YtDlpResolver::new().with_handle(runtime.handle().clone());
            match runtime.block_on(resolver.resolve(&video)) {
                Ok(stream) => eprintln!(
                    "video via yt-dlp: ok, codec {:?}, bitrate {:?} kbps",
                    stream.info.codec, stream.info.bitrate_kbps
                ),
                Err(e) => eprintln!("video via yt-dlp: error ({e})"),
            }
        }
        None => eprintln!("video via yt-dlp: no music video found to try"),
    }
    let _ = std::fs::remove_dir_all(&scratch);
}
