//! Provider trace events of this crate (dev-tools spec). A fixed vocabulary: the provider, the
//! host, a status word and the time. Never the address (it carries the track metadata), a title
//! or an artist.

use std::time::Instant;

use nemo_core::dev::elapsed_ms;
use nemo_core::lyrics::LyricsError;

/// The fixed word for a failed call.
fn error_kind(error: &LyricsError) -> &'static str {
    match error {
        LyricsError::RateLimited { .. } => "rate_limited",
        LyricsError::Network(_) => "network",
        LyricsError::Parse(_) => "parse",
        LyricsError::Unavailable(_) => "unavailable",
    }
}

/// One lyrics request: `provider` is `lyrics:<name>`, `status` the HTTP status or the error.
pub(crate) fn call(
    provider: &'static str,
    host: &str,
    status: Result<u16, &LyricsError>,
    began: Instant,
) {
    let ms = elapsed_ms(began);
    match status {
        Ok(code) => nemo_core::nemo_trace!(provider, "lyrics", host = host, status = code, ms = ms),
        Err(e) => {
            nemo_core::nemo_trace!(
                provider,
                "lyrics",
                host = host,
                status = error_kind(e),
                ms = ms
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_kinds_are_fixed_words() {
        let secret = LyricsError::Network("https://x/?artist=Someone".to_owned());
        assert_eq!(error_kind(&secret), "network");
        assert_eq!(
            error_kind(&LyricsError::RateLimited { retry_after: None }),
            "rate_limited"
        );
    }
}
