//! Provider trace events of this crate (dev-tools spec). A fixed vocabulary: provider, a short
//! operation name taken from the path, the host, the status and the time. Never the address,
//! the client id, a track id or a token.

use std::time::{Duration, Instant};

use nemo_core::dev::{elapsed_ms, host_of};

use crate::hls::fetch::Failure;
use crate::http::{ScAnswer, ScHttpError};

/// The provider name of every event of this crate.
const PROVIDER: &str = "soundcloud";

/// A short fixed name for what an address asks for, from its first path segment.
pub(crate) fn op_of(url: &str) -> &'static str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let path = rest.split_once('/').map_or("", |(_, path)| path);
    let first = path.split(['/', '?', '#']).next().unwrap_or_default();
    match first {
        "" => "site",
        "assets" => "script",
        "search" => "search",
        "resolve" => "resolve",
        "me" => "me",
        "tracks" => "track",
        "playlists" => "playlist",
        "users" => "user",
        "charts" | "mixed-selections" | "featured_tracks" => "home",
        "stream" => "stream",
        "media" => "stream_url",
        "likes" | "track_likes" | "playlist_likes" => "likes",
        _ => "api",
    }
}

/// One finished request attempt, whatever its outcome. `rest` is the backoff a 429 started.
pub(crate) fn sent(
    url: &str,
    result: &Result<ScAnswer, ScHttpError>,
    started: Instant,
    attempt: u64,
    rest: Option<Duration>,
) {
    let op = op_of(url);
    let host = host_of(url);
    let ms = elapsed_ms(started);
    match (result, rest) {
        (Ok(answer), Some(rest)) => nemo_core::nemo_trace!(
            PROVIDER,
            op,
            host = host,
            status = answer.status,
            ms = ms,
            attempt = attempt,
            bytes = answer.body.len(),
            rest = rest.as_secs()
        ),
        (Ok(answer), None) => nemo_core::nemo_trace!(
            PROVIDER,
            op,
            host = host,
            status = answer.status,
            ms = ms,
            attempt = attempt,
            bytes = answer.body.len()
        ),
        (Err(error), _) => nemo_core::nemo_trace!(
            PROVIDER,
            op,
            host = host,
            status = error_kind(*error),
            ms = ms,
            attempt = attempt
        ),
    }
}

/// A call that was not sent because a 429 backoff is running.
pub(crate) fn held_back(url: &str, rest: Duration) {
    nemo_core::nemo_trace!(
        PROVIDER,
        op_of(url),
        host = host_of(url),
        status = "backoff",
        rest = rest.as_secs()
    );
}

/// A playlist or segment fetch that failed (successes are not traced: one per few seconds of
/// playback would fill the ring).
pub(crate) fn stream_failed(url: &str, failure: Failure, started: Instant, attempt: u64) {
    let status = match failure {
        Failure::Gone => "gone",
        Failure::Busy => "busy",
        Failure::Network => "network",
    };
    nemo_core::nemo_trace!(
        PROVIDER,
        "hls",
        host = host_of(url),
        status = status,
        ms = elapsed_ms(started),
        attempt = attempt
    );
}

/// One stage of a sign-in and its outcome (`ok`, `cancelled`, `rejected`, ...). Stage names:
/// `signin_window`, `signin_check`, `signin_store`.
pub(crate) fn signin_stage(stage: &'static str, outcome: &'static str) {
    nemo_core::nemo_trace!(PROVIDER, stage, status = outcome);
}

/// The fixed name of a request error.
fn error_kind(error: ScHttpError) -> &'static str {
    match error {
        ScHttpError::Timeout => "timeout",
        ScHttpError::Failed => "failed",
        ScHttpError::TooLarge => "too_large",
        ScHttpError::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_operation_comes_from_the_first_path_segment() {
        let api = "https://api-v2.soundcloud.com";
        assert_eq!(
            op_of(&format!("{api}/search/tracks?q=x&client_id=abc")),
            "search"
        );
        assert_eq!(op_of(&format!("{api}/resolve?url=y")), "resolve");
        assert_eq!(op_of(&format!("{api}/me")), "me");
        assert_eq!(op_of(&format!("{api}/tracks/123")), "track");
        assert_eq!(op_of(&format!("{api}/mixed-selections")), "home");
        assert_eq!(op_of(&format!("{api}/something/else")), "api");
        assert_eq!(op_of("https://soundcloud.com/"), "site");
        assert_eq!(op_of("https://a-v2.sndcdn.com/assets/app.js"), "script");
    }
}
