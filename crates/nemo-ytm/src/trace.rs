//! Provider trace events of this crate (dev-tools spec). A fixed vocabulary: provider, a short
//! operation name, the host, a status word and the time. Never an address, an id, a title or a
//! cookie.

use std::future::Future;
use std::time::{Duration, Instant};

use nemo_core::account::AccountError;
use nemo_core::catalog::CatalogError;
use nemo_core::dev::{catalog_outcome, elapsed_ms, host_of};
use nemo_core::stream::{ResolvedStream, StreamError};

/// The host every catalog call goes to.
const MUSIC_HOST: &str = "music.youtube.com";

/// Run one catalog call and trace it: `provider` is `ytm` (anonymous) or `ytm_account` (signed
/// in), `op` is the method name.
pub(crate) async fn catalog<T, F>(
    provider: &'static str,
    op: &'static str,
    work: F,
) -> Result<T, CatalogError>
where
    F: Future<Output = Result<T, CatalogError>>,
{
    let started = Instant::now();
    let result = work.await;
    nemo_core::nemo_trace!(
        provider,
        op,
        host = MUSIC_HOST,
        status = catalog_outcome(&result),
        ms = elapsed_ms(started)
    );
    result
}

/// One stage of a sign-in and its outcome. Stage names: `signin_window`, `signin_check`,
/// `signin_store`.
pub(crate) fn signin<T>(stage: &'static str, result: &Result<T, AccountError>) {
    let outcome = match result {
        Ok(_) => "ok",
        Err(AccountError::Cancelled) => "cancelled",
        Err(AccountError::Blocked) => "blocked",
        Err(AccountError::Rejected) => "rejected",
        Err(AccountError::Unavailable(_)) => "unavailable",
        Err(AccountError::Storage(_)) => "storage",
        Err(AccountError::Network(_)) => "network",
        Err(AccountError::LimitReached) => "limit_reached",
        Err(AccountError::UnknownAccount) => "unknown_account",
    };
    nemo_core::nemo_trace!("ytm", stage, status = outcome);
}

/// One stream resolve by one tier of the chain: `native`, `in_process` or `yt_dlp`.
pub(crate) fn tier(
    tier: &'static str,
    answer: &Result<ResolvedStream, StreamError>,
    began: Instant,
) {
    let status = stream_status(answer);
    nemo_core::nemo_trace!(
        "resolver",
        "stream_url",
        tier = tier,
        status = status,
        ms = elapsed_ms(began)
    );
}

/// A stream lookup in the cache: `hit` (memory), `disk` (kept by an earlier run) or `miss`.
pub(crate) fn cache(status: &'static str) {
    nemo_core::nemo_trace!("resolver", "cache", status = status);
}

/// A tier that stops being asked for `pause`.
pub(crate) fn resting(tier: &'static str, pause: Duration) {
    nemo_core::nemo_trace!(
        "resolver",
        "rest",
        tier = tier,
        status = "resting",
        rest = pause.as_secs()
    );
}

/// One `yt-dlp` run (`run_fast`, `run_full`, `warm_up`) and how it ended.
pub(crate) fn ytdlp(
    op: &'static str,
    answer: &Result<ResolvedStream, StreamError>,
    began: Instant,
) {
    nemo_core::nemo_trace!(
        "ytdlp",
        op,
        status = stream_status(answer),
        ms = elapsed_ms(began)
    );
}

/// The `yt-dlp` version warm-up run.
pub(crate) fn ytdlp_warm_up(ok: bool, began: Instant) {
    let status = if ok { "ok" } else { "failed" };
    nemo_core::nemo_trace!("ytdlp", "warm_up", status = status, ms = elapsed_ms(began));
}

/// One step of the in-process resolver: `player` (prepare the player script), `player_request`
/// or `solve`.
pub(crate) fn cipher_step(op: &'static str, ok: bool, began: Instant) {
    let status = if ok { "ok" } else { "failed" };
    nemo_core::nemo_trace!(
        "resolver",
        op,
        tier = "in_process",
        status = status,
        ms = elapsed_ms(began)
    );
}

/// The in-process resolver switched itself off for `pause` after a failure.
pub(crate) fn solver_off(pause: Duration) {
    nemo_core::nemo_trace!(
        "resolver",
        "rest",
        tier = "in_process",
        status = "off",
        rest = pause.as_secs()
    );
}

/// The native player request: the HTTP status, or `connect_failed`, `cut_off` or `too_large`.
pub(crate) fn player_http(url: &str, status: u16, began: Instant, bytes: usize) {
    nemo_core::nemo_trace!(
        "ytm",
        "player",
        host = host_of(url),
        status = status,
        ms = elapsed_ms(began),
        bytes = bytes
    );
}

/// The native player request failed before an HTTP status came.
pub(crate) fn player_failed(url: &str, kind: &'static str, began: Instant) {
    nemo_core::nemo_trace!(
        "ytm",
        "player",
        host = host_of(url),
        status = kind,
        ms = elapsed_ms(began)
    );
}

/// The first window of a stream the player opened: the media host and the HTTP status (or a
/// fixed kind).
pub(crate) fn stream_open(url: &str, status: Option<u16>, began: Instant) {
    let host = host_of(url);
    let ms = elapsed_ms(began);
    match status {
        Some(code) => {
            nemo_core::nemo_trace!("ytm", "stream_open", host = host, status = code, ms = ms);
        }
        None => {
            nemo_core::nemo_trace!(
                "ytm",
                "stream_open",
                host = host,
                status = "failed",
                ms = ms
            );
        }
    }
}

/// The fixed word for what a resolve came to.
fn stream_status(answer: &Result<ResolvedStream, StreamError>) -> &'static str {
    match answer {
        Ok(_) => "ok",
        Err(StreamError::NotAvailable) => "refused",
        Err(StreamError::ResolverMissing) => "missing",
        Err(StreamError::Network(_)) => "network",
        Err(StreamError::Parse(_)) => "parse",
    }
}

/// One request whose answer is an HTTP status or a failure text of this crate (`Err`).
pub(crate) fn http(
    provider: &'static str,
    op: &'static str,
    host: &str,
    status: Result<u16, &str>,
    began: Instant,
) {
    let ms = elapsed_ms(began);
    match status {
        Ok(code) => nemo_core::nemo_trace!(provider, op, host = host, status = code, ms = ms),
        Err(text) => nemo_core::nemo_trace!(
            provider,
            op,
            host = host,
            status = failure_kind(text),
            ms = ms
        ),
    }
}

/// One request whose answer is only a success or a failure.
pub(crate) fn fetch_result(
    provider: &'static str,
    op: &'static str,
    host: &str,
    ok: bool,
    began: Instant,
) {
    let status = if ok { "ok" } else { "failed" };
    nemo_core::nemo_trace!(
        provider,
        op,
        host = host,
        status = status,
        ms = elapsed_ms(began)
    );
}

/// The result of the check of a stored stream address (`true`: it still answers).
pub(crate) fn stored_check(alive: bool, began: Instant) {
    let status = if alive { "ok" } else { "stale" };
    nemo_core::nemo_trace!(
        "resolver",
        "stored_check",
        status = status,
        ms = elapsed_ms(began)
    );
}

/// A fixed word for one of this crate's failure texts, which are fixed sentences already.
fn failure_kind(text: &str) -> &'static str {
    if text.contains("timed out") {
        "timeout"
    } else if text.contains("too large") {
        "too_large"
    } else if text.contains("cancelled") {
        "cancelled"
    } else {
        "failed"
    }
}
