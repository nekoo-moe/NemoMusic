//! Playlist and album pages opened from Home and Explore, one per id kind (development only,
//! real network). Run by hand:
//! `cargo test -p nemo-ytm --test diagnose_playlists -- --ignored --nocapture`
//! `NEMO_DIAG_MODE=anon` (default) uses the anonymous catalog; `signed` loads the saved session
//! through `GoogleAccount` (Windows Credential Manager) the way `diagnose_account.rs` does.
//!
//! Prints the id kind (prefix class), the result and, on failure, the raw page shape. Never a
//! title, id, name, URL or cookie.
#![allow(
    clippy::print_stderr,
    reason = "a diagnostic reports what it observed on the real network"
)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::account::{AccountError, AccountInfo, AccountService, AccountState};
use nemo_core::catalog::{BrowseId, Catalog, FeedItem, FeedShelf};
use nemo_core::secret::Secret;
use nemo_ytm::account::{AccountConfig, GoogleAccount, SessionSlot};
use nemo_ytm::catalog::YtmCatalog;
use nemo_ytm::credentials::WindowsStore;
use nemo_ytm::diagnostics::id_kind;
use nemo_ytm::session::{SessionBackend, SessionCatalog};

struct Capture {
    handle: tokio::runtime::Handle,
    caught: Arc<Mutex<Option<Arc<SessionCatalog>>>>,
}

impl SessionBackend for Capture {
    fn probe(&self, _: Secret<String>) -> BoxFuture<'_, Result<AccountInfo, AccountError>> {
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

/// One id per kind: (is album, id).
fn pick(shelves: &[FeedShelf], into: &mut BTreeMap<&'static str, (bool, BrowseId)>) {
    for item in shelves.iter().flat_map(|s| &s.items) {
        let (album, id) = match item {
            FeedItem::Playlist(p) => (false, p.id.clone()),
            FeedItem::Album(a) => (true, a.id.clone()),
            _ => continue,
        };
        into.entry(id_kind(&id.0)).or_insert((album, id));
    }
}

fn pause() {
    std::thread::sleep(Duration::from_secs(1));
}

#[test]
#[ignore = "real network; signed mode uses the saved account session"]
#[allow(clippy::too_many_lines, reason = "a linear development report")]
fn open_every_kind_of_playlist() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    let signed = std::env::var("NEMO_DIAG_MODE").is_ok_and(|m| m == "signed");
    let scratch = std::env::var_os("KIROCREW_SCRATCH")
        .map_or_else(std::env::temp_dir, std::path::PathBuf::from)
        .join("nemo-diagnose-playlists");
    std::fs::create_dir_all(&scratch).expect("scratch dir");

    let mut session = None;
    let anonymous = YtmCatalog::new(runtime.handle().clone());
    let mut _account = None;
    if signed {
        let cache = scratch.join("account.json");
        std::fs::write(&cache, r#"{"name":"diagnostic","handle":null,"avatar":[]}"#)
            .expect("cache");
        let caught = Arc::new(Mutex::new(None));
        let account = GoogleAccount::new(
            AccountConfig {
                helper: scratch.join("none.exe"),
                webview_dir: scratch.join("webview"),
                cache_file: cache,
            },
            Arc::new(WindowsStore::new().expect("credential store")),
            Arc::new(Capture {
                handle: runtime.handle().clone(),
                caught: Arc::clone(&caught),
            }),
            SessionSlot::default(),
            runtime.handle().clone(),
        );
        assert!(
            matches!(account.state(), AccountState::SignedIn(_)),
            "no saved session"
        );
        session = caught.lock().expect("lock").clone();
        _account = Some(account);
    }
    let catalog: &dyn Catalog = match &session {
        Some(s) => s.as_ref(),
        None => &anonymous,
    };
    eprintln!("mode: {}", if signed { "signed in" } else { "anonymous" });

    let mut ids = BTreeMap::new();
    match runtime.block_on(catalog.home_feed()) {
        Ok(home) => {
            eprintln!("home: {} shelves", home.shelves.len());
            pick(&home.shelves, &mut ids);
        }
        Err(e) => eprintln!("home: error ({e})"),
    }
    pause();
    match runtime.block_on(catalog.explore()) {
        Ok(explore) => {
            let releases = FeedShelf {
                title: String::new(),
                subtitle: None,
                kind: nemo_core::catalog::ShelfKind::Cards,
                items: explore
                    .new_releases
                    .iter()
                    .cloned()
                    .map(FeedItem::Album)
                    .collect(),
            };
            pick(&explore.shelves, &mut ids);
            pick(&explore.charts, &mut ids);
            pick(std::slice::from_ref(&releases), &mut ids);
        }
        Err(e) => eprintln!("explore: error ({e})"),
    }
    for (kind, (album, id)) in &ids {
        pause();
        let result = if *album {
            runtime
                .block_on(catalog.album(id))
                .map(|a| format!("{} tracks", a.tracks.len()))
        } else {
            runtime.block_on(catalog.playlist(id)).map(|p| {
                format!(
                    "{} tracks, title {}",
                    p.tracks.len(),
                    !p.summary.title.is_empty()
                )
            })
        };
        match result {
            Ok(text) => eprintln!(
                "{kind} ({}): ok, {text}",
                if *album { "album()" } else { "playlist()" }
            ),
            Err(e) => {
                eprintln!("{kind}: error ({e})");
                pause();
                let browse = if *album || id.0.starts_with("VL") {
                    id.0.clone()
                } else {
                    format!("VL{}", id.0)
                };
                let shape = match &session {
                    Some(s) => runtime.block_on(s.diagnose_browse(browse)),
                    None => runtime.block_on(anonymous.diagnose_browse(browse)),
                };
                eprintln!("  {shape}");
            }
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
}
