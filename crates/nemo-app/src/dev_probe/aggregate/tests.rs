use nemo_core::dev::{DevAccount, DevLevel};
use tracing::Level;

use super::{Class, classify, last_signin, log_lines, percentile, providers, resolver, trace_rows};
use crate::dev_capture::CapturedEvent;

fn trace(at_ms: u64, provider: &str, op: &str, status: &str, ms: Option<u64>) -> CapturedEvent {
    let mut fields = vec![
        ("provider", provider.to_owned()),
        ("op", op.to_owned()),
        ("status", status.to_owned()),
    ];
    if let Some(ms) = ms {
        fields.push(("ms", ms.to_string()));
    }
    CapturedEvent {
        at_ms,
        level: Level::DEBUG,
        target: "nemo_trace".to_owned(),
        message: "trace".to_owned(),
        fields,
    }
}

fn with(mut event: CapturedEvent, name: &'static str, value: &str) -> CapturedEvent {
    event.fields.push((name, value.to_owned()));
    event
}

#[test]
fn status_words_and_codes_are_classified() {
    assert_eq!(classify("search", "200"), Class::Ok);
    assert_eq!(classify("search", "304"), Class::Ok);
    assert_eq!(classify("search", "403"), Class::Failed);
    assert_eq!(classify("search", "503"), Class::Failed);
    assert_eq!(classify("search", "ok"), Class::Ok);
    assert_eq!(classify("search", "timeout"), Class::Failed);
    assert_eq!(classify("search", "refused"), Class::Failed);
    assert_eq!(classify("cache", "miss"), Class::Note);
    assert_eq!(classify("rest", "resting"), Class::Note);
    assert_eq!(classify("stored_check", "stale"), Class::Ok);
}

#[test]
fn percentiles_use_the_nearest_rank() {
    let data: Vec<u64> = (1..=10).collect();
    assert_eq!(percentile(&data, 50), Some(5));
    assert_eq!(percentile(&data, 95), Some(10));
    assert_eq!(percentile(&[7], 50), Some(7));
    assert_eq!(percentile(&[], 50), None);
}

#[test]
fn the_trace_lists_newest_first_and_skips_log_lines() {
    let log = CapturedEvent {
        at_ms: 2,
        level: Level::WARN,
        target: "nemo_ytm::x".to_owned(),
        message: "hello".to_owned(),
        fields: vec![("error", "boom".to_owned())],
    };
    let events = vec![
        trace(1, "ytm", "search", "200", Some(40)),
        log,
        with(
            trace(3, "ytm", "search", "429", Some(90)),
            "host",
            "music.youtube.com",
        ),
    ];
    let rows = trace_rows(&events, 10);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].at_ms, 3);
    assert_eq!(rows[0].host, "music.youtube.com");
    assert!(rows[0].failed);
    assert!(!rows[1].failed);
    assert_eq!(trace_rows(&events, 1).len(), 1);
    let lines = log_lines(&events, 10);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].level, DevLevel::Warn);
    assert_eq!(lines[0].text, "hello error=boom");
}

#[test]
fn a_card_counts_calls_errors_and_the_last_error_kind() {
    let events = vec![
        trace(10, "soundcloud", "search", "200", Some(100)),
        trace(20, "soundcloud", "search", "429", Some(50)),
        trace(30, "soundcloud", "me", "timeout", Some(300)),
        trace(40, "soundcloud", "me", "200", Some(200)),
        with(trace(50, "soundcloud", "search", "429", None), "rest", "30"),
    ];
    let cards = providers(&events);
    assert_eq!(cards.len(), 1);
    let card = &cards[0];
    assert_eq!(card.name, "soundcloud");
    assert_eq!((card.calls, card.errors), (5, 3));
    assert_eq!(card.last_ok_ms, Some(40));
    assert_eq!(card.last_error_kind.as_deref(), Some("429"));
    assert_eq!(card.resting_until_ms, Some(50 + 30_000));
    assert_eq!(card.p50_ms, Some(100));
    assert_eq!(card.p95_ms, Some(300));
}

#[test]
fn percentiles_look_only_at_the_last_fifty_calls() {
    let mut events = vec![trace(0, "ytm", "search", "200", Some(10_000))];
    for n in 1..=50 {
        events.push(trace(n, "ytm", "search", "200", Some(100)));
    }
    let cards = providers(&events);
    assert_eq!(cards[0].p95_ms, Some(100));
    assert_eq!(cards[0].calls, 51);
}

#[test]
fn cards_are_sorted_by_name_and_notes_are_not_calls() {
    let events = vec![
        trace(1, "ytm", "search", "200", Some(5)),
        trace(2, "lyrics:lrclib", "lyrics", "404", Some(5)),
        trace(3, "resolver", "cache", "hit", None),
    ];
    let cards = providers(&events);
    let names: Vec<&str> = cards.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["lyrics:lrclib", "resolver", "ytm"]);
    assert_eq!(cards[1].calls, 0);
}

#[test]
fn the_resolver_counts_tiers_and_the_cache() {
    let events = vec![
        with(
            trace(1, "resolver", "stream_url", "ok", Some(70)),
            "tier",
            "native",
        ),
        with(
            trace(2, "resolver", "stream_url", "refused", Some(60)),
            "tier",
            "native",
        ),
        with(
            trace(3, "resolver", "stream_url", "ok", Some(3500)),
            "tier",
            "yt_dlp",
        ),
        trace(4, "resolver", "cache", "hit", None),
        trace(5, "resolver", "cache", "hit", None),
        trace(6, "resolver", "cache", "disk", None),
        trace(7, "resolver", "cache", "miss", None),
        trace(8, "ytm", "search", "200", Some(1)),
    ];
    let out = resolver(&events);
    let tier = |name: &str| {
        out.tiers
            .iter()
            .find(|t| t.name == name)
            .map(|t| (t.ok, t.failed))
    };
    assert_eq!(tier("native"), Some((1, 1)));
    assert_eq!(tier("in_process"), Some((0, 0)));
    assert_eq!(tier("yt_dlp"), Some((1, 0)));
    assert_eq!(
        (out.cache_hits, out.cache_disk_hits, out.cache_misses),
        (2, 1, 1)
    );
}

#[test]
fn the_last_sign_in_stage_comes_from_the_newest_signin_event() {
    let events = vec![
        trace(1, "ytm", "signin_window", "ok", None),
        trace(2, "ytm", "signin_check", "rejected", None),
        trace(3, "soundcloud", "signin_window", "cancelled", None),
        trace(4, "ytm", "search", "200", Some(1)),
    ];
    let mut account = DevAccount::default();
    last_signin(&events, "ytm", &mut account);
    assert_eq!(account.last_stage.as_deref(), Some("signin_check"));
    assert_eq!(account.last_outcome.as_deref(), Some("rejected"));
    assert_eq!(account.last_at_ms, Some(2));
    let mut none = DevAccount::default();
    last_signin(&events, "spotify", &mut none);
    assert_eq!(none.last_stage, None);
}
