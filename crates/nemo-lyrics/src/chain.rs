//! The provider chain (ADR 0002).
//!
//! 1. The local provider (id [`LOCAL_ID`]) runs first and alone. A usable local result ends the
//!    lookup: no query leaves the machine for lyrics the user already has.
//! 2. Remote providers then run in parallel. Each gets its own timeout
//!    ([`LyricsProvider::timeout`], default 4 s, including the wait for its host), the whole chain
//!    has a global budget (default 6 s, counted from the start) that also caps every provider's
//!    timeout, and at most one request per host is in flight.
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

/// Time limits of a chain run. Each provider's own limit is [`LyricsProvider::timeout`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainOptions {
    /// Longest the whole run may take. It also caps every provider's timeout.
    pub global_budget: Duration,
}

impl Default for ChainOptions {
    fn default() -> Self {
        Self {
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

/// Run the remote providers in parallel under their timeouts and the per-host limit.
async fn run_remote(
    entries: &[ChainEntry],
    remote: &[(usize, &ChainEntry)],
    query: &Arc<LyricsQuery>,
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
        let limit = entry
            .provider
            .timeout()
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
        let limit = entry
            .provider
            .timeout()
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
    let (fresh, remote_reports) = run_remote(entries, &remote, &shared_query, deadline).await;
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
mod tests;
