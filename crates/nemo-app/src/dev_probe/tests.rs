use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use nemo_core::dev::{DevLenClass, DevPathKind, DevProbe, DevSolverState};
use serde_json::json;
use tracing::Level;

use super::{AppProbe, SecretFacts, Sources, unknown_sources};
use crate::dev_capture::{CapturedEvent, DevRing, Redactor};

fn fixed_clock() -> u64 {
    1_700_000_000_000
}

fn trace(at_ms: u64, provider: &str, op: &str, status: &str, host: &str) -> CapturedEvent {
    CapturedEvent {
        at_ms,
        level: Level::DEBUG,
        target: "nemo_trace".to_owned(),
        message: "trace".to_owned(),
        fields: vec![
            ("provider", provider.to_owned()),
            ("op", op.to_owned()),
            ("host", host.to_owned()),
            ("status", status.to_owned()),
            ("ms", "40".to_owned()),
        ],
    }
}

fn probe(ring: &DevRing, sources: Sources) -> AppProbe {
    AppProbe::with_parts(
        ring.clone(),
        Redactor::with_home("/home/alice"),
        sources,
        fixed_clock,
    )
}

#[test]
fn a_probe_with_no_sources_shows_every_fact_as_unknown() {
    let ring = DevRing::new(10);
    let snap = probe(&ring, unknown_sources()).snapshot();
    assert_eq!(snap.taken_ms, fixed_clock());
    assert!(snap.trace.is_empty() && snap.log.is_empty() && snap.providers.is_empty());
    assert_eq!(snap.resolver.solver, DevSolverState::Unknown);
    assert_eq!(snap.resolver.ytdlp_path, DevPathKind::Unknown);
    assert_eq!(snap.accounts.len(), 2);
    for account in &snap.accounts {
        assert_eq!(account.signed_in, None);
        assert_eq!(account.token_present, None);
        assert_eq!(account.token_len, DevLenClass::Unknown);
    }
    assert_eq!(snap.system.audio, None);
    assert!(snap.system.config.is_empty());
    assert_eq!(snap.system.version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn the_snapshot_holds_rows_cards_hosts_and_the_last_sign_in() {
    let ring = DevRing::new(50);
    ring.push(trace(1, "ytm", "search", "200", "music.youtube.com"));
    ring.push(trace(2, "ytm", "search", "403", "music.youtube.com"));
    ring.push(trace(3, "soundcloud", "signin_check", "rejected", ""));
    ring.push(trace(4, "soundcloud", "me", "200", "api-v2.soundcloud.com"));
    let snap = probe(&ring, unknown_sources()).snapshot();
    assert_eq!(snap.trace.len(), 4);
    assert_eq!(snap.trace[0].at_ms, 4);
    assert_eq!(snap.providers.len(), 2);
    let ytm = snap
        .providers
        .iter()
        .find(|p| p.name == "ytm")
        .expect("ytm card");
    assert_eq!((ytm.calls, ytm.errors), (2, 1));
    assert_eq!(snap.system.egress[0].host, "music.youtube.com");
    assert_eq!(snap.system.egress[0].count, 2);
    let sc = snap
        .accounts
        .iter()
        .find(|a| a.site == "SoundCloud")
        .expect("account");
    assert_eq!(sc.last_stage.as_deref(), Some("signin_check"));
    assert_eq!(sc.last_outcome.as_deref(), Some("rejected"));
    assert_eq!(snap.generation, ring.generation());
}

#[test]
fn no_secret_value_reaches_the_snapshot() {
    let ring = DevRing::new(10);
    let mut sources = unknown_sources();
    sources.ytm_secret = Box::new(|| {
        SecretFacts::of(
            Some("SAPISID=TOPSECRETCOOKIE1; HSID=TOPSECRETCOOKIE2"),
            true,
        )
    });
    sources.sc_secret = Box::new(|| SecretFacts::of(Some("2-123456-TOPSECRETTOKEN"), false));
    sources.ytm_signed_in = Box::new(|| Some(true));
    let snap = probe(&ring, sources).snapshot();
    let text = format!("{snap:?}");
    assert!(!text.contains("TOPSECRET"), "{text}");
    let ytm = &snap.accounts[0];
    assert_eq!(ytm.signed_in, Some(true));
    assert_eq!(ytm.token_present, Some(true));
    assert_eq!(ytm.token_len, DevLenClass::Medium);
    assert_eq!(ytm.cookie_names, vec!["HSID", "SAPISID"]);
    assert_eq!(snap.accounts[1].token_len, DevLenClass::Short);
}

#[test]
fn config_audio_and_resolver_facts_come_from_the_sources_and_are_redacted() {
    let ring = DevRing::new(10);
    let mut sources = unknown_sources();
    sources.config_now = Box::new(|| json!({"volume_percent": 80, "yt_dlp_path": "/home/alice/y"}));
    sources.config_default = json!({"volume_percent": 100, "yt_dlp_path": null});
    sources.audio = Box::new(|| Some("Speakers of /home/alice, 48000 Hz".to_owned()));
    sources.solver = Box::new(|| DevSolverState::AutoOff);
    sources.ytdlp_path = Box::new(|| DevPathKind::Managed);
    sources.ytdlp_build = Box::new(|| Some("abcdef012345".to_owned()));
    let snap = probe(&ring, sources).snapshot();
    assert_eq!(
        snap.system.audio.as_deref(),
        Some("Speakers of ~, 48000 Hz")
    );
    let keys: Vec<(&str, Option<&str>)> = snap
        .system
        .config
        .iter()
        .map(|c| (c.key.as_str(), c.value.as_deref()))
        .collect();
    assert!(keys.contains(&("volume_percent", Some("80"))));
    assert!(keys.contains(&("yt_dlp_path", None)));
    assert_eq!(snap.resolver.solver, DevSolverState::AutoOff);
    assert_eq!(snap.resolver.ytdlp_path, DevPathKind::Managed);
    assert_eq!(snap.resolver.ytdlp_build.as_deref(), Some("abcdef012345"));
}

#[test]
fn the_credential_store_is_read_once_per_interval_not_once_per_snapshot() {
    let ring = DevRing::new(10);
    let reads = Arc::new(AtomicUsize::new(0));
    let mut sources = unknown_sources();
    let counter = Arc::clone(&reads);
    sources.ytm_secret = Box::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        SecretFacts::default()
    });
    let probe = probe(&ring, sources);
    for _ in 0..5 {
        let _ = probe.snapshot();
    }
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn clear_empties_the_ring_and_redact_uses_the_capture_rules() {
    let ring = DevRing::new(10);
    ring.push(trace(1, "ytm", "search", "200", "music.youtube.com"));
    let probe = probe(&ring, unknown_sources());
    assert_eq!(probe.snapshot().trace.len(), 1);
    probe.clear();
    assert!(probe.snapshot().trace.is_empty());
    assert_eq!(
        probe.redact("see https://a.example/x?k=SECRET and /home/alice/f"),
        "see a.example and ~/…"
    );
}
