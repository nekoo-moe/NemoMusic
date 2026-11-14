//! The shared state behind [`LyricsHub`](super::LyricsHub): cache, chain and the lookup logic.

use std::sync::{Mutex, MutexGuard};

use nemo_core::lyrics::{
    LyricsCandidate, LyricsCredit, LyricsDoc, LyricsError, LyricsQuery, ProviderId,
};
use nemo_core::services::LyricsLookup;

use super::track_key;
use crate::cache::{EntryStatus, LyricsCache};
use crate::chain::{ChainEntry, ChainOptions, Outcome, run_chain, run_chain_all};
use crate::local::LOCAL_ID;
use crate::pick::Candidate;

pub(super) struct Inner {
    pub(super) cache: LyricsCache,
    pub(super) chain: Mutex<Vec<ChainEntry>>,
    pub(super) options: ChainOptions,
}

/// Sort key of the picker: best granularity, then score, then chain order.
fn sort_candidates(list: &mut [(usize, LyricsCandidate)]) {
    list.sort_by(|a, b| {
        b.1.granularity
            .cmp(&a.1.granularity)
            .then_with(|| b.1.score.cmp(&a.1.score))
            .then_with(|| a.0.cmp(&b.0))
    });
}

impl Inner {
    pub(super) fn chain(&self) -> MutexGuard<'_, Vec<ChainEntry>> {
        self.chain
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The provider the user picked for this track, if it is still in the chain.
    pub(super) fn chosen_id(&self, key: &str) -> Option<ProviderId> {
        let name = self.cache.choice(key).ok().flatten()?;
        self.chain()
            .iter()
            .map(|e| e.provider.id())
            .find(|id| id.as_str() == name)
    }

    /// Answer cached fresh rows: which providers need no question, and their hits.
    fn read_cache(&self, key: &str, entries: &mut [ChainEntry]) -> Vec<Candidate> {
        let mut prior = Vec::new();
        for (order, entry) in entries.iter_mut().enumerate() {
            let id = entry.provider.id();
            if !entry.enabled || id == LOCAL_ID {
                continue;
            }
            match self.cache.get(key, id) {
                Ok(Some(row)) => {
                    entry.enabled = false;
                    if let EntryStatus::Hit { doc, score } = row.status {
                        prior.push(Candidate {
                            doc,
                            score,
                            chain_order: order,
                        });
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(provider = id.as_str(), error = %e, "lyrics cache read failed");
                }
            }
        }
        prior
    }

    /// Store what the providers answered.
    fn store(&self, key: &str, reports: &[crate::chain::ProviderReport]) {
        for report in reports {
            let id = report.provider;
            if id == LOCAL_ID {
                continue;
            }
            let stored = match &report.outcome {
                Outcome::Hit { doc, score, credit } => {
                    let put = self.cache.put_hit(key, id, doc, *score);
                    match credit {
                        Some(c) if put.is_ok() => self.cache.put_credit(key, id, c),
                        _ => put,
                    }
                }
                Outcome::Miss | Outcome::Rejected => self.cache.put_miss(key, id),
                Outcome::Error(e) => {
                    let retry_after = match e {
                        LyricsError::RateLimited { retry_after } => *retry_after,
                        _ => None,
                    };
                    self.cache.put_error(key, id, retry_after)
                }
                Outcome::TimedOut => Ok(()),
            };
            if let Err(e) = stored {
                tracing::warn!(provider = id.as_str(), error = %e, "lyrics cache write failed");
            }
        }
    }

    fn candidate(
        &self,
        key: &str,
        entries: &[ChainEntry],
        doc: LyricsDoc,
        score: u8,
        credit: Option<LyricsCredit>,
    ) -> LyricsCandidate {
        let risk = entries
            .iter()
            .find(|e| e.provider.id() == doc.provider)
            .and_then(|e| e.provider.info().risk_note);
        let credit = credit.or_else(|| self.cache.credit(key, doc.provider).ok().flatten());
        let source_note = credit
            .as_ref()
            .and_then(|c| c.note.clone())
            .or_else(|| risk.map(str::to_owned));
        LyricsCandidate {
            provider: doc.provider,
            granularity: doc.granularity,
            score,
            source_note,
            credit,
            doc,
        }
    }

    pub(super) async fn lookup(&self, query: LyricsQuery) -> LyricsLookup {
        let key = track_key(&query);
        let entries = self.chain().clone();
        let tried: Vec<ProviderId> = entries
            .iter()
            .filter(|e| e.enabled)
            .map(|e| e.provider.id())
            .collect();

        // The user's pick wins when its lyrics are already cached.
        let chosen = self.chosen_id(&key);
        if let Some(id) = chosen.filter(|id| *id != LOCAL_ID)
            && let Ok(Some(row)) = self.cache.get(&key, id)
            && let EntryStatus::Hit { doc, .. } = row.status
        {
            return LyricsLookup {
                doc: Some(doc),
                tried,
            };
        }

        // Cache first. Providers answered by a fresh row are switched off for the chain run.
        let mut to_ask = entries.clone();
        let prior = self.read_cache(&key, &mut to_ask);

        let result = run_chain(&to_ask, &query, self.options, &prior).await;
        self.store(&key, &result.reports);

        // A fresh answer from the picked provider beats the automatic pick.
        let picked = chosen.and_then(|id| {
            result.reports.iter().find_map(|r| match &r.outcome {
                Outcome::Hit { doc, .. } if r.provider == id => Some(doc.clone()),
                _ => None,
            })
        });
        let mut lookup = result.into_lookup();
        if picked.is_some() {
            lookup.doc = picked;
        }
        lookup.tried = tried;
        lookup
    }

    pub(super) async fn lookup_all(&self, query: LyricsQuery) -> Vec<LyricsCandidate> {
        let key = track_key(&query);
        let entries = self.chain().clone();
        let mut to_ask = entries.clone();
        let prior = self.read_cache(&key, &mut to_ask);

        let result = run_chain_all(&to_ask, &query, self.options, &prior).await;
        self.store(&key, &result.reports);

        let mut list: Vec<(usize, LyricsCandidate)> = prior
            .into_iter()
            .map(|c| {
                (
                    c.chain_order,
                    self.candidate(&key, &entries, c.doc, c.score, None),
                )
            })
            .collect();
        for report in result.reports {
            if let Outcome::Hit { doc, score, credit } = report.outcome {
                let order = entries
                    .iter()
                    .position(|e| e.provider.id() == report.provider)
                    .unwrap_or(usize::MAX);
                list.push((order, self.candidate(&key, &entries, doc, score, credit)));
            }
        }
        sort_candidates(&mut list);
        list.into_iter().map(|(_, c)| c).collect()
    }
}
