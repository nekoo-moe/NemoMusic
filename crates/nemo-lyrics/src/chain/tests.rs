//! Unit tests for the provider chain.

use std::sync::atomic::{AtomicUsize, Ordering};

use nemo_core::BoxFuture;
use nemo_core::lyrics::{Granularity, LyricLine, MatchedMeta};

use super::*;

type Answer = Result<Option<LyricsDoc>, LyricsError>;

/// A provider that waits, then answers. Counts calls and concurrent calls.
struct Fake {
    id: ProviderId,
    hosts: &'static [&'static str],
    delay: Duration,
    answer: Answer,
    calls: Arc<AtomicUsize>,
    in_flight: Arc<AtomicUsize>,
    max_in_flight: Arc<AtomicUsize>,
}

impl Fake {
    fn new(
        name: &'static str,
        host: &'static [&'static str],
        delay_ms: u64,
        answer: Answer,
    ) -> Self {
        Self {
            id: ProviderId::new(name),
            hosts: host,
            delay: Duration::from_millis(delay_ms),
            answer,
            calls: Arc::new(AtomicUsize::new(0)),
            in_flight: Arc::new(AtomicUsize::new(0)),
            max_in_flight: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl LyricsProvider for Fake {
    fn id(&self) -> ProviderId {
        self.id
    }
    fn hosts(&self) -> &'static [&'static str] {
        self.hosts
    }
    fn fetch<'a>(&'a self, _q: &'a LyricsQuery) -> BoxFuture<'a, Answer> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_in_flight.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            self.answer.clone()
        })
    }
}

fn query() -> LyricsQuery {
    LyricsQuery {
        title: "Test Song".into(),
        artists: vec!["Test Artist".into()],
        album: None,
        duration_ms: Some(200_000),
        ytm_video_id: None,
        local_path: None,
    }
}

fn doc(name: &'static str, g: Granularity, last_ms: u64) -> LyricsDoc {
    let timed = g != Granularity::Unsynced;
    LyricsDoc {
        lines: vec![LyricLine {
            start_ms: timed.then_some(last_ms),
            end_ms: None,
            text: "line".into(),
            words: Vec::new(),
        }],
        granularity: g,
        provider: ProviderId::new(name),
        matched: None,
        instrumental: false,
    }
}

fn entry(f: Fake) -> (ChainEntry, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let calls = Arc::clone(&f.calls);
    let max = Arc::clone(&f.max_in_flight);
    (ChainEntry::new(Arc::new(f)), calls, max)
}

fn winner(r: &ChainResult) -> Option<&'static str> {
    r.best.as_ref().map(|c| c.doc.provider.as_str())
}

#[tokio::test(start_paused = true)]
async fn local_hit_ends_the_run_without_remote_calls() {
    let (local, _, _) = entry(Fake::new(
        "Local file",
        &[],
        5,
        Ok(Some(doc("Local file", Granularity::Unsynced, 0))),
    ));
    let (remote, remote_calls, _) = entry(Fake::new(
        "remote",
        &["a.example"],
        5,
        Ok(Some(doc("remote", Granularity::Word, 1000))),
    ));
    let r = run_chain(&[local, remote], &query(), ChainOptions::default(), &[]).await;
    assert_eq!(winner(&r), Some("Local file"));
    assert_eq!(remote_calls.load(Ordering::SeqCst), 0);
    assert_eq!(r.reports.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn local_miss_falls_through_to_remote() {
    let (local, _, _) = entry(Fake::new("Local file", &[], 1, Ok(None)));
    let (remote, _, _) = entry(Fake::new(
        "remote",
        &["a.example"],
        5,
        Ok(Some(doc("remote", Granularity::Line, 1000))),
    ));
    let r = run_chain(&[local, remote], &query(), ChainOptions::default(), &[]).await;
    assert_eq!(winner(&r), Some("remote"));
    assert_eq!(
        r.tried.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        ["Local file", "remote"]
    );
    assert_eq!(r.reports[0].outcome, Outcome::Miss);
}

#[tokio::test(start_paused = true)]
async fn remote_providers_run_in_parallel() {
    let (a, _, _) = entry(Fake::new(
        "a",
        &["a.example"],
        3000,
        Ok(Some(doc("a", Granularity::Line, 1))),
    ));
    let (b, _, _) = entry(Fake::new(
        "b",
        &["b.example"],
        3000,
        Ok(Some(doc("b", Granularity::Line, 1))),
    ));
    let start = Instant::now();
    let r = run_chain(&[a, b], &query(), ChainOptions::default(), &[]).await;
    assert!(
        start.elapsed() < Duration::from_millis(3500),
        "{:?}",
        start.elapsed()
    );
    assert_eq!(r.reports.len(), 2);
    // Tie on granularity and score: the earlier provider in the chain wins.
    assert_eq!(winner(&r), Some("a"));
}

#[tokio::test(start_paused = true)]
async fn same_host_requests_do_not_overlap() {
    let (a, _, max_a) = entry(Fake::new("a", &["shared.example"], 1000, Ok(None)));
    let (b, _, max_b) = entry(Fake::new("b", &["shared.example"], 1000, Ok(None)));
    let start = Instant::now();
    let _ = run_chain(&[a, b], &query(), ChainOptions::default(), &[]).await;
    assert!(start.elapsed() >= Duration::from_millis(2000));
    // Each fake saw at most itself in flight, and together they never overlapped.
    assert_eq!(max_a.load(Ordering::SeqCst), 1);
    assert_eq!(max_b.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn shared_host_in_flight_never_exceeds_one() {
    // One counter shared through the host: use a single Fake pair writing to the same atomics.
    let in_flight = Arc::new(AtomicUsize::new(0));
    let max = Arc::new(AtomicUsize::new(0));
    let mk = |name: &'static str| {
        let mut f = Fake::new(name, &["shared.example"], 500, Ok(None));
        f.in_flight = Arc::clone(&in_flight);
        f.max_in_flight = Arc::clone(&max);
        ChainEntry::new(Arc::new(f))
    };
    let entries = [mk("a"), mk("b"), mk("c")];
    let opts = ChainOptions {
        provider_timeout: Duration::from_secs(10),
        global_budget: Duration::from_secs(10),
    };
    let _ = run_chain(&entries, &query(), opts, &[]).await;
    assert_eq!(max.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn slow_provider_times_out_at_four_seconds() {
    let (slow, _, _) = entry(Fake::new(
        "slow",
        &["s.example"],
        10_000,
        Ok(Some(doc("slow", Granularity::Word, 1))),
    ));
    let (fast, _, _) = entry(Fake::new(
        "fast",
        &["f.example"],
        100,
        Ok(Some(doc("fast", Granularity::Line, 1))),
    ));
    let start = Instant::now();
    let r = run_chain(&[slow, fast], &query(), ChainOptions::default(), &[]).await;
    assert_eq!(start.elapsed(), Duration::from_secs(4));
    assert_eq!(winner(&r), Some("fast"));
    assert_eq!(r.reports[0].outcome, Outcome::TimedOut);
}

#[tokio::test(start_paused = true)]
async fn global_budget_caps_the_run() {
    let (slow, _, _) = entry(Fake::new("slow", &["s.example"], 20_000, Ok(None)));
    let opts = ChainOptions {
        provider_timeout: Duration::from_secs(60),
        global_budget: Duration::from_secs(6),
    };
    let start = Instant::now();
    let r = run_chain(&[slow], &query(), opts, &[]).await;
    assert_eq!(start.elapsed(), Duration::from_secs(6));
    assert_eq!(r.reports[0].outcome, Outcome::TimedOut);
    assert!(r.best.is_none());
    assert_eq!(r.tried.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn wrong_match_is_dropped_before_picking() {
    // Word-synced doc ends 60 s past the 200 s track: wrong match, loses to the line doc.
    let (bad, _, _) = entry(Fake::new(
        "bad",
        &["a.example"],
        1,
        Ok(Some(doc("bad", Granularity::Word, 260_000))),
    ));
    let (ok, _, _) = entry(Fake::new(
        "ok",
        &["b.example"],
        1,
        Ok(Some(doc("ok", Granularity::Line, 190_000))),
    ));
    let r = run_chain(&[bad, ok], &query(), ChainOptions::default(), &[]).await;
    assert_eq!(winner(&r), Some("ok"));
    assert_eq!(r.reports[0].outcome, Outcome::Rejected);
}

#[tokio::test(start_paused = true)]
async fn weak_metadata_match_is_dropped() {
    let mut other = doc("other", Granularity::Line, 1000);
    other.matched = Some(MatchedMeta {
        title: "Completely Different".into(),
        artists: vec!["Nobody".into()],
        album: None,
        duration_ms: Some(200_000),
    });
    let (p, _, _) = entry(Fake::new("other", &["a.example"], 1, Ok(Some(other))));
    let r = run_chain(&[p], &query(), ChainOptions::default(), &[]).await;
    assert!(r.best.is_none());
    assert_eq!(r.reports[0].outcome, Outcome::Rejected);
}

#[tokio::test(start_paused = true)]
async fn best_granularity_wins_over_chain_order() {
    let (a, _, _) = entry(Fake::new(
        "a",
        &["a.example"],
        1,
        Ok(Some(doc("a", Granularity::Unsynced, 0))),
    ));
    let (b, _, _) = entry(Fake::new(
        "b",
        &["b.example"],
        9,
        Ok(Some(doc("b", Granularity::Word, 5000))),
    ));
    let r = run_chain(&[a, b], &query(), ChainOptions::default(), &[]).await;
    assert_eq!(winner(&r), Some("b"));
}

#[tokio::test(start_paused = true)]
async fn errors_do_not_fail_the_lookup() {
    let limited = LyricsError::RateLimited {
        retry_after: Some(Duration::from_secs(9)),
    };
    let (a, _, _) = entry(Fake::new("a", &["a.example"], 1, Err(limited.clone())));
    let (b, _, _) = entry(Fake::new(
        "b",
        &["b.example"],
        1,
        Ok(Some(doc("b", Granularity::Line, 1))),
    ));
    let r = run_chain(&[a, b], &query(), ChainOptions::default(), &[]).await;
    assert_eq!(winner(&r), Some("b"));
    assert_eq!(r.reports[0].outcome, Outcome::Error(limited));
    let lookup = r.into_lookup();
    assert!(lookup.doc.is_some());
    assert_eq!(lookup.tried.len(), 2);
}

#[tokio::test(start_paused = true)]
async fn disabled_providers_are_skipped() {
    let (mut a, calls, _) = entry(Fake::new(
        "a",
        &["a.example"],
        1,
        Ok(Some(doc("a", Granularity::Word, 1))),
    ));
    a.enabled = false;
    let (b, _, _) = entry(Fake::new("b", &["b.example"], 1, Ok(None)));
    let r = run_chain(&[a, b], &query(), ChainOptions::default(), &[]).await;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        r.tried.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        ["b"]
    );
    assert!(r.best.is_none());
}

#[tokio::test(start_paused = true)]
async fn order_is_the_tie_breaker_and_prior_candidates_compete() {
    let (a, _, _) = entry(Fake::new(
        "a",
        &["a.example"],
        1,
        Ok(Some(doc("a", Granularity::Line, 1))),
    ));
    let prior = Candidate {
        doc: doc("cached", Granularity::Word, 1),
        score: 90,
        chain_order: 5,
    };
    let r = run_chain(
        std::slice::from_ref(&a),
        &query(),
        ChainOptions::default(),
        &[prior],
    )
    .await;
    assert_eq!(winner(&r), Some("cached"));
    let empty = run_chain(&[], &query(), ChainOptions::default(), &[]).await;
    assert!(empty.best.is_none() && empty.tried.is_empty());
}
