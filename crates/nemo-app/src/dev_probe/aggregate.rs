//! Pure functions that turn the captured events into the rows and cards of a [`DevSnapshot`].
//!
//! [`DevSnapshot`]: nemo_core::dev::DevSnapshot

use std::collections::{BTreeMap, VecDeque};

use nemo_core::dev::{
    DevAccount, DevLevel, DevLogLine, DevProvider, DevResolver, DevTier, DevTraceRow, TRACE_TARGET,
};
use tracing::Level;

use crate::dev_capture::CapturedEvent;

/// How many calls the percentiles look at.
const WINDOW: usize = 50;

/// The tiers of the stream resolver, in the order the chain asks them.
const TIERS: [&str; 3] = ["native", "in_process", "yt_dlp"];

/// What a trace status word means for the call counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Class {
    /// The call worked.
    Ok,
    /// The call failed.
    Failed,
    /// Not a call: a cache lookup or a note about a rest.
    Note,
}

/// Classify one event by its operation and status.
pub(super) fn classify(op: &str, status: &str) -> Class {
    if matches!(op, "cache" | "rest") {
        return Class::Note;
    }
    let worked = match status.parse::<u16>() {
        Ok(code) => code < 400,
        Err(_) => matches!(status, "ok" | "hit" | "disk" | "miss" | "stale" | ""),
    };
    if worked { Class::Ok } else { Class::Failed }
}

fn number(event: &CapturedEvent, name: &str) -> Option<u64> {
    event.field(name)?.parse().ok()
}

fn is_trace(event: &CapturedEvent) -> bool {
    event.target == TRACE_TARGET
}

/// The provider trace, newest first, at most `limit` rows.
pub(super) fn trace_rows(events: &[CapturedEvent], limit: usize) -> Vec<DevTraceRow> {
    events
        .iter()
        .rev()
        .filter(|e| is_trace(e))
        .take(limit)
        .map(|e| {
            let op = e.field("op").unwrap_or_default();
            let status = e.field("status").unwrap_or_default();
            DevTraceRow {
                at_ms: e.at_ms,
                provider: e.field("provider").unwrap_or("unknown").to_owned(),
                op: op.to_owned(),
                host: e.field("host").unwrap_or_default().to_owned(),
                status: status.to_owned(),
                ms: number(e, "ms"),
                tier: e.field("tier").map(str::to_owned),
                bytes: number(e, "bytes"),
                attempt: number(e, "attempt"),
                rest_s: number(e, "rest"),
                failed: classify(op, status) == Class::Failed,
            }
        })
        .collect()
}

fn level_of(level: Level) -> DevLevel {
    if level == Level::ERROR {
        DevLevel::Error
    } else if level == Level::WARN {
        DevLevel::Warn
    } else if level == Level::INFO {
        DevLevel::Info
    } else if level == Level::DEBUG {
        DevLevel::Debug
    } else {
        DevLevel::Trace
    }
}

/// The log lines (everything but the provider trace), newest first, at most `limit`.
pub(super) fn log_lines(events: &[CapturedEvent], limit: usize) -> Vec<DevLogLine> {
    events
        .iter()
        .rev()
        .filter(|e| !is_trace(e))
        .take(limit)
        .map(|e| {
            let mut text = e.message.clone();
            for (name, value) in &e.fields {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(name);
                text.push('=');
                text.push_str(value);
            }
            DevLogLine {
                at_ms: e.at_ms,
                level: level_of(e.level),
                target: e.target.clone(),
                text,
            }
        })
        .collect()
}

#[derive(Default)]
struct Tally {
    calls: u64,
    errors: u64,
    last_ok_ms: Option<u64>,
    last_error_kind: Option<String>,
    recent_ms: VecDeque<u64>,
    resting_until_ms: Option<u64>,
}

/// The nearest-rank percentile `pct` (1 to 100) of `sorted`.
fn percentile(sorted: &[u64], pct: usize) -> Option<u64> {
    let rank = (sorted.len() * pct).div_ceil(100).max(1);
    sorted.get(rank - 1).copied()
}

/// One card per provider seen in the trace, sorted by name.
pub(super) fn providers(events: &[CapturedEvent]) -> Vec<DevProvider> {
    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    for event in events.iter().filter(|e| is_trace(e)) {
        let name = event.field("provider").unwrap_or("unknown").to_owned();
        let tally = tallies.entry(name).or_default();
        let op = event.field("op").unwrap_or_default();
        let status = event.field("status").unwrap_or_default();
        if let Some(rest) = number(event, "rest").filter(|r| *r > 0) {
            tally.resting_until_ms = Some(event.at_ms.saturating_add(rest.saturating_mul(1000)));
        }
        match classify(op, status) {
            Class::Note => continue,
            Class::Ok => tally.last_ok_ms = Some(event.at_ms),
            Class::Failed => {
                tally.errors += 1;
                tally.last_error_kind = Some(status.to_owned());
            }
        }
        tally.calls += 1;
        if let Some(ms) = number(event, "ms") {
            if tally.recent_ms.len() == WINDOW {
                tally.recent_ms.pop_front();
            }
            tally.recent_ms.push_back(ms);
        }
    }
    tallies
        .into_iter()
        .map(|(name, tally)| {
            let mut sorted: Vec<u64> = tally.recent_ms.iter().copied().collect();
            sorted.sort_unstable();
            DevProvider {
                name,
                calls: tally.calls,
                errors: tally.errors,
                last_ok_ms: tally.last_ok_ms,
                last_error_kind: tally.last_error_kind,
                p50_ms: percentile(&sorted, 50),
                p95_ms: percentile(&sorted, 95),
                resting_until_ms: tally.resting_until_ms,
            }
        })
        .collect()
}

/// The tier counts and the cache counts of the stream resolver. The facts that no event
/// carries (solver state, `yt-dlp`) stay at their `Unknown` defaults.
pub(super) fn resolver(events: &[CapturedEvent]) -> DevResolver {
    let mut out = DevResolver {
        tiers: TIERS
            .iter()
            .map(|name| DevTier {
                name: (*name).to_owned(),
                ..DevTier::default()
            })
            .collect(),
        ..DevResolver::default()
    };
    for event in events
        .iter()
        .filter(|e| is_trace(e) && e.field("provider") == Some("resolver"))
    {
        let status = event.field("status").unwrap_or_default();
        match event.field("op") {
            Some("stream_url") => {
                let class = classify("stream_url", status);
                if let Some(tier) = out
                    .tiers
                    .iter_mut()
                    .find(|t| Some(t.name.as_str()) == event.field("tier"))
                {
                    if class == Class::Failed {
                        tier.failed += 1;
                    } else {
                        tier.ok += 1;
                    }
                }
            }
            Some("cache") => match status {
                "hit" => out.cache_hits += 1,
                "disk" => out.cache_disk_hits += 1,
                "miss" => out.cache_misses += 1,
                _ => {}
            },
            _ => {}
        }
    }
    out
}

/// Fill the last sign-in stage of `account` from the newest `signin_*` event of `provider`.
pub(super) fn last_signin(events: &[CapturedEvent], provider: &str, account: &mut DevAccount) {
    let newest = events.iter().rev().find(|e| {
        is_trace(e)
            && e.field("provider") == Some(provider)
            && e.field("op").is_some_and(|op| op.starts_with("signin_"))
    });
    if let Some(event) = newest {
        account.last_stage = event.field("op").map(str::to_owned);
        account.last_outcome = event.field("status").map(str::to_owned);
        account.last_at_ms = Some(event.at_ms);
    }
}

#[cfg(test)]
mod tests;
