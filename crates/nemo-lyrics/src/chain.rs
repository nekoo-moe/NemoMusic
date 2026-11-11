//! The provider chain (ADR 0002).
//!
//! 1. The local provider (id [`LOCAL_ID`]) runs first and alone. A usable local result ends the
//!    lookup: no query leaves the machine for lyrics the user already has.
//! 2. Remote providers then run in parallel. Each gets a per-provider timeout (default 4 s,
//!    including the wait for its host), the whole chain has a global budget (default 6 s,
//!    counted from the start), and at most one request per host is in flight.
//! 3. Results are judged: a synced doc whose last timestamp is past the track end is a wrong match
//!    ([`reject_wrong_match`]), a doc that names its matched track must score as acceptable
//!    ([`score_match`]). The rest go to [`pick_best`].
//!
//! The order of `entries` is the chain order. The caller keeps it user-editable and may switch
//! single providers off with [`ChainEntry::enabled`].

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Duration;

use nemo_core::lyrics::{
    CreditedDoc, LyricsCredit, LyricsDoc, LyricsError, LyricsProvider, LyricsQuery, ProviderId,
};
use nemo_core::services::LyricsLookup;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::{Instant, timeout, timeout_at};

use crate::local::LOCAL_ID;
use crate::pick::{Candidate, pick_best};
use crate::score::{reject_wrong_match, score_match};

/// Score given to a doc that does not say which track it matched (found by id or by file).
const UNVERIFIED_SCORE: u8 = 100;

/// One provider in the chain, with its on/off switch.
#[derive(Clone)]
pub struct ChainEntry {
    /// The provider.
    pub provider: Arc<dyn LyricsProvider>,
    /// Disabled providers are skipped and do not appear in `tried`.
    pub enabled: bool,
}

impl ChainEntry {
    /// An enabled entry.
    #[must_use]
    pub fn new(provider: Arc<dyn LyricsProvider>) -> Self {
        Self {
            provider,
            enabled: true,
        }
    }
}

impl std::fmt::Debug for ChainEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChainEntry")
            .field("provider", &self.provider.id())
            .field("enabled", &self.enabled)
            .finish()
    }
}

/// Time limits of a chain run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainOptions {
    /// Longest a single remote provider may take, including waiting for its host.
    pub provider_timeout: Duration,
    /// Longest the whole run may take.
    pub global_budget: Duration,
}

impl Default for ChainOptions {
    fn default() -> Self {
        Self {
            provider_timeout: Duration::from_secs(4),
            global_budget: Duration::from_secs(6),
        }
    }
}

/// What one provider contributed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)] // A few short-lived values per lookup; boxing buys nothing.
pub enum Outcome {
    /// An accepted result.
    Hit {
        /// The lyrics.
        doc: LyricsDoc,
        /// Match score, 0 to 100.
        score: u8,
        /// Credit the source asks for.
        credit: Option<LyricsCredit>,
    },
    /// The provider has no lyrics for the track.
    Miss,
    /// The provider answered, but the answer was dropped as a wrong match.
    Rejected,
    /// The provider failed.
    Error(LyricsError),
    /// The provider did not finish within its timeout or the global budget.
    TimedOut,
}

/// The outcome of one provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderReport {
    /// Which provider.
    pub provider: ProviderId,
    /// What it contributed.
    pub outcome: Outcome,
}

/// The result of a chain run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainResult {
    /// The best accepted candidate, if any.
    pub best: Option<Candidate>,
    /// Providers that were asked, in chain order.
    pub tried: Vec<ProviderId>,
    /// One report per asked provider, in chain order. Providers skipped because the local
    /// provider already answered have no report.
    pub reports: Vec<ProviderReport>,
}

impl ChainResult {
    /// The shape the UI service returns.
    #[must_use]
    pub fn into_lookup(self) -> LyricsLookup {
        LyricsLookup {
            doc: self.best.map(|c| c.doc),
            tried: self.tried,
        }
    }
}

/// Judge one provider answer.
fn judge(query: &LyricsQuery, chain_order: usize, answer: Answer) -> (Outcome, Option<Candidate>) {
    let (doc, credit) = match answer {
        Ok(Some(found)) => found,
        Ok(None) => return (Outcome::Miss, None),
        Err(e) => return (Outcome::Error(e), None),
    };
    if reject_wrong_match(&doc, query.duration_ms) {
        return (Outcome::Rejected, None);
    }
    let score = match &doc.matched {
        None => UNVERIFIED_SCORE,
        Some(meta) => match score_match(query, meta) {
            Some(s) if s.is_acceptable() => s.total,
            _ => return (Outcome::Rejected, None),
        },
    };
    let candidate = Candidate {
        doc: doc.clone(),
        score,
        chain_order,
    };
    (Outcome::Hit { doc, score, credit }, Some(candidate))
}

type Answer = Result<Option<CreditedDoc>, LyricsError>;

/// Fetch with a timeout; the timeout also covers waiting for the host permits.
async fn fetch_limited(
    provider: Arc<dyn LyricsProvider>,
    query: Arc<LyricsQuery>,
    host_locks: Vec<Arc<Semaphore>>,
    limit: Duration,
) -> Option<Answer> {
    let work = async {
        // Permits are taken in a fixed (sorted host) order, so two providers sharing several
        // hosts cannot wait on each other.
        let mut permits = Vec::with_capacity(host_locks.len());
        for lock in &host_locks {
            match lock.acquire().await {
                Ok(p) => permits.push(p),
                Err(_) => {
                    return Err(LyricsError::Unavailable("host limiter closed".to_owned()));
                }
            }
        }
        provider.fetch_credited(&query).await
    };
    timeout(limit, work).await.ok()
}

/// Report for one provider.
fn report(
    entries: &[ChainEntry],
    order: usize,
    outcome: Outcome,
) -> Option<(usize, ProviderReport)> {
    entries.get(order).map(|e| {
        (
            order,
            ProviderReport {
                provider: e.provider.id(),
                outcome,
            },
        )
    })
}

/// Run the remote providers in parallel under the timeouts and the per-host limit.
async fn run_remote(
    entries: &[ChainEntry],
    remote: &[(usize, &ChainEntry)],
    query: &Arc<LyricsQuery>,
    options: ChainOptions,
    deadline: Instant,
) -> (Vec<Candidate>, Vec<(usize, ProviderReport)>) {
    let mut host_semaphores: HashMap<&'static str, Arc<Semaphore>> = HashMap::new();
    for (_, entry) in remote {
        for host in entry.provider.hosts() {
            host_semaphores
                .entry(host)
                .or_insert_with(|| Arc::new(Semaphore::new(1)));
        }
    }

    let mut set: JoinSet<(usize, Option<Answer>)> = JoinSet::new();
    let mut pending: BTreeSet<usize> = BTreeSet::new();
    for &(order, entry) in remote {
        let hosts: BTreeSet<&'static str> = entry.provider.hosts().iter().copied().collect();
        let locks: Vec<Arc<Semaphore>> = hosts
            .iter()
            .filter_map(|h| host_semaphores.get(h).cloned())
            .collect();
        let limit = options
            .provider_timeout
            .min(deadline.saturating_duration_since(Instant::now()));
        let provider = Arc::clone(&entry.provider);
        let q = Arc::clone(query);
        pending.insert(order);
        set.spawn(async move { (order, fetch_limited(provider, q, locks, limit).await) });
    }

    let mut candidates = Vec::new();
    let mut reports = Vec::new();
    let mut budget_spent = false;
    while !pending.is_empty() {
        match timeout_at(deadline, set.join_next()).await {
            Ok(Some(Ok((order, answer)))) => {
                pending.remove(&order);
                let (outcome, candidate) = match answer {
                    Some(a) => judge(query, order, a),
                    None => (Outcome::TimedOut, None),
                };
                reports.extend(report(entries, order, outcome));
                candidates.extend(candidate);
            }
            Ok(None) => break,
            Ok(Some(Err(_))) => {
                // A provider task panicked. Its order is unknown, so what is still pending is
                // reported as failed below.
                tracing::warn!("lyrics provider task failed");
                break;
            }
            Err(_) => {
                budget_spent = true;
                break;
            }
        }
    }
    set.abort_all();
    for order in pending {
        let outcome = if budget_spent {
            Outcome::TimedOut
        } else {
            Outcome::Error(LyricsError::Unavailable("provider task failed".to_owned()))
        };
        reports.extend(report(entries, order, outcome));
    }
    (candidates, reports)
}

/// Run the chain for one query.
///
/// `prior` holds candidates the caller already has (for example fresh cache hits). Their
/// `chain_order` must be the index of their provider in `entries`; the caller marks those
/// providers as `enabled: false` in the slice it passes so they are not asked again. Prior
/// candidates compete in the final pick, except that a local hit ends the run first.
pub async fn run_chain(
    entries: &[ChainEntry],
    query: &LyricsQuery,
    options: ChainOptions,
    prior: &[Candidate],
) -> ChainResult {
    run_inner(entries, query, options, prior, true).await
}

/// Like [`run_chain`], but a local hit does not end the run: every enabled provider is asked and
/// every acceptable answer is reported (for the provider picker). est is still the pick.
pub async fn run_chain_all(
    entries: &[ChainEntry],
    query: &LyricsQuery,
    options: ChainOptions,
    prior: &[Candidate],
) -> ChainResult {
    run_inner(entries, query, options, prior, false).await
}

async fn run_inner(
    entries: &[ChainEntry],
    query: &LyricsQuery,
    options: ChainOptions,
    prior: &[Candidate],
    stop_on_local: bool,
) -> ChainResult {
    let deadline = Instant::now() + options.global_budget;
    let active: Vec<(usize, &ChainEntry)> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.enabled)
        .collect();
    let tried: Vec<ProviderId> = active.iter().map(|(_, e)| e.provider.id()).collect();
    let mut reports: Vec<(usize, ProviderReport)> = Vec::new();
    let shared_query = Arc::new(query.clone());
    let mut local_hits: Vec<Candidate> = Vec::new();

    // Stage 1: local providers, one after another, no network.
    for &(order, entry) in active.iter().filter(|(_, e)| e.provider.id() == LOCAL_ID) {
        let limit = options
            .provider_timeout
            .min(deadline.saturating_duration_since(Instant::now()));
        let answer = fetch_limited(
            Arc::clone(&entry.provider),
            Arc::clone(&shared_query),
            Vec::new(),
            limit,
        )
        .await;
        let (outcome, candidate) = match answer {
            Some(a) => judge(query, order, a),
            None => (Outcome::TimedOut, None),
        };
        reports.extend(report(entries, order, outcome));
        if candidate.is_some() && stop_on_local {
            return finish(candidate, tried, reports);
        }
        local_hits.extend(candidate);
    }

    // Stage 2: remote providers in parallel, one request per host.
    let remote: Vec<(usize, &ChainEntry)> = active
        .iter()
        .copied()
        .filter(|(_, e)| e.provider.id() != LOCAL_ID)
        .collect();
    let (fresh, remote_reports) =
        run_remote(entries, &remote, &shared_query, options, deadline).await;
    reports.extend(remote_reports);

    let mut candidates: Vec<Candidate> = prior.to_vec();
    candidates.extend(local_hits);
    candidates.extend(fresh);
    let best = pick_best(&candidates).cloned();
    finish(best, tried, reports)
}
fn finish(
    best: Option<Candidate>,
    tried: Vec<ProviderId>,
    mut reports: Vec<(usize, ProviderReport)>,
) -> ChainResult {
    reports.sort_by_key(|(order, _)| *order);
    ChainResult {
        best,
        tried,
        reports: reports.into_iter().map(|(_, r)| r).collect(),
    }
}

#[cfg(test)]
mod tests {
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
}
