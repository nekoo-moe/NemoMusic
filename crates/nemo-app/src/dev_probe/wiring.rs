//! The sources of the app's probe: small closures over the services that already exist.

use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use nemo_core::account::AccountState;
use nemo_core::dev::{DevPathKind, DevProbe, DevSolverState};
use nemo_core::source_account::SourceAccountState;
use nemo_ytm::cipher::OffMarker;
use nemo_ytm::credentials::Vault;
use nemo_ytm::ytdlp::{managed, managed_dir};
use serde_json::Value;

use super::{AppProbe, SecretFacts, Sources};
use crate::backend::Services;
use crate::config::Settings;
use crate::dev_capture::DevRing;
use crate::paths::AppPaths;

/// How many characters of the managed `yt-dlp` build hash are shown.
const BUILD_CHARS: usize = 12;

/// The probe over the running services.
pub(crate) fn for_app(
    ring: DevRing,
    services: &Services,
    paths: &AppPaths,
    settings: &Settings,
) -> Arc<dyn DevProbe> {
    let backend = &services.backend;
    let (account, sc) = (
        Arc::clone(&backend.account),
        backend.soundcloud_account.clone(),
    );
    let sc_for_secret = sc.is_some();
    let (store, store_sc) = (services.store.clone(), services.store.clone());
    let player = Arc::clone(&backend.player);
    let root = paths.root.clone();
    let off_file = paths.in_process_off.clone();
    let (setting_on, has_cipher) = (settings.in_process_resolver, services.cipher.is_some());
    let (user_set, ytdlp) = (settings.yt_dlp_path.is_some(), services.ytdlp.clone());
    Arc::new(AppProbe::new(
        ring,
        Sources {
            ytm_signed_in: Box::new(move || {
                Some(matches!(account.state(), AccountState::SignedIn(_)))
            }),
            sc_signed_in: Box::new(move || {
                sc.as_ref()
                    .map(|a| matches!(a.state(), SourceAccountState::SignedIn(_)))
            }),
            ytm_secret: Box::new(move || {
                store.clone().map_or_else(SecretFacts::default, |store| {
                    Vault::new(store).load().map_or_else(
                        |_| SecretFacts::default(),
                        |v| SecretFacts::of(v.as_deref(), true),
                    )
                })
            }),
            sc_secret: Box::new(move || match store_sc.as_ref().filter(|_| sc_for_secret) {
                Some(store) => match store.get(nemo_soundcloud::TOKEN_KEY) {
                    Ok(Some(bytes)) => {
                        SecretFacts::of(Some(&String::from_utf8_lossy(&bytes)), false)
                    }
                    Ok(None) => SecretFacts::of(None, false),
                    Err(_) => SecretFacts::default(),
                },
                None => SecretFacts::default(),
            }),
            audio: Box::new(move || player.snapshot().signal_path.as_ref().map(audio_text)),
            config_now: Box::new(move || to_json(&Settings::load(&root))),
            config_default: to_json(&Settings::default()),
            solver: Box::new(move || {
                solver_state(
                    setting_on,
                    has_cipher,
                    &OffMarker::new(Some(off_file.clone())),
                )
            }),
            ytdlp_path: Box::new(move || {
                path_kind(
                    Path::new(ytdlp.program()),
                    user_set,
                    managed_dir().as_deref(),
                )
            }),
            ytdlp_build: Box::new(|| {
                managed::ManagedYtDlp::for_this_platform()
                    .and_then(|m| m.installed_hash())
                    .map(|hash| hash.chars().take(BUILD_CHARS).collect())
            }),
        },
    ))
}

fn to_json(settings: &Settings) -> Value {
    serde_json::to_value(settings).unwrap_or(Value::Null)
}

/// `device, 48000 Hz, 24 bit, 2 ch, exclusive` for the current signal path.
fn audio_text(path: &nemo_core::audio::SignalPath) -> String {
    let out = &path.output;
    let depth = out
        .format
        .bit_depth
        .map_or_else(String::new, |bits| format!(", {bits} bit"));
    format!(
        "{}, {} Hz{depth}, {} ch, {:?}",
        out.device_name, out.format.sample_rate, out.format.channels, out.mode
    )
}

/// The state of the in-process solver this run.
pub(super) fn solver_state(
    setting_on: bool,
    has_cipher: bool,
    marker: &OffMarker,
) -> DevSolverState {
    if !setting_on {
        DevSolverState::Disabled
    } else if marker.resting(SystemTime::now()) {
        DevSolverState::AutoOff
    } else if has_cipher {
        DevSolverState::Active
    } else {
        DevSolverState::Unknown
    }
}

/// Where the program of `yt-dlp` comes from.
pub(super) fn path_kind(program: &Path, user_set: bool, managed: Option<&Path>) -> DevPathKind {
    if user_set {
        DevPathKind::User
    } else if program == Path::new("yt-dlp") || program == Path::new("yt-dlp.exe") {
        DevPathKind::Missing
    } else if managed.is_some_and(|dir| program.starts_with(dir)) {
        DevPathKind::Managed
    } else {
        DevPathKind::Path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_path_kind_tells_user_managed_path_and_missing_apart() {
        let managed = Path::new("/data/NemoMusic/bin/yt-dlp");
        let in_managed = Path::new("/data/NemoMusic/bin/yt-dlp/yt-dlp");
        assert_eq!(
            path_kind(in_managed, true, Some(managed)),
            DevPathKind::User
        );
        assert_eq!(
            path_kind(in_managed, false, Some(managed)),
            DevPathKind::Managed
        );
        assert_eq!(
            path_kind(Path::new("/usr/bin/yt-dlp"), false, Some(managed)),
            DevPathKind::Path
        );
        assert_eq!(
            path_kind(Path::new("yt-dlp"), false, Some(managed)),
            DevPathKind::Missing
        );
        assert_eq!(
            path_kind(Path::new("yt-dlp.exe"), false, None),
            DevPathKind::Missing
        );
    }

    #[test]
    fn the_solver_state_follows_the_setting_the_marker_and_the_resolver() {
        let off = OffMarker::new(None);
        assert_eq!(solver_state(false, true, &off), DevSolverState::Disabled);
        assert_eq!(solver_state(true, true, &off), DevSolverState::Active);
        assert_eq!(solver_state(true, false, &off), DevSolverState::Unknown);
    }
}
