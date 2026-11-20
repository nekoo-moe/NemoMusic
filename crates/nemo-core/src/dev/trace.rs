//! The provider trace event: one convention, one macro, a closed list of field names.

use std::time::Instant;

use crate::catalog::CatalogError;
use crate::source_account::SourceAccountError;

/// The `tracing` target of provider trace events. The developer ring takes events of this
/// target at `debug` level, whatever `NEMO_LOG` says.
pub const TRACE_TARGET: &str = "nemo_trace";

/// The field names a trace event may carry besides its message. `nemo_trace!` accepts only
/// these, and the capture layer of `nemo-app` keeps only these (and a few log fields).
pub const TRACE_FIELDS: [&str; 9] = [
    "provider", "op", "host", "status", "ms", "bytes", "tier", "attempt", "rest",
];

/// Emit one provider trace event (target `nemo_trace`, level `debug`).
///
/// The first two arguments are the provider (`"ytm"`, `"soundcloud"`, `"lyrics:lrclib"`,
/// `"sponsorblock"`, `"ytdlp"`, `"resolver"`, and so on) and a fixed short operation name
/// (`"search"`, `"stream_url"`, `"home"`, `"me"`, `"lyrics"`). The rest are `name = value` pairs
/// from [`TRACE_FIELDS`]: `host` (host only), `status` (an HTTP status or a fixed error kind),
/// `ms`, `bytes`, `tier` (`native`, `in_process`, `yt_dlp`), `attempt` and `rest` (backoff
/// seconds). Any other name does not compile.
///
/// Never pass a URL, query, id, title, token, cookie or user name. The calling crate needs a
/// direct dependency on `tracing`. Do not call it from an audio callback.
///
/// ```text
/// nemo_core::nemo_trace!("ytm", "search", host = "music.youtube.com", status = 200_u16, ms = 41_u64);
/// ```
#[macro_export]
macro_rules! nemo_trace {
    ($provider:expr, $op:expr $(, $key:ident = $value:expr)* $(,)?) => {{
        $( $crate::__nemo_trace_field!($key); )*
        ::tracing::debug!(
            target: "nemo_trace",
            provider = $provider,
            op = $op,
            $( $key = $value, )*
            "trace"
        );
    }};
}

/// Checks at compile time that a `nemo_trace!` key is one of [`TRACE_FIELDS`].
#[doc(hidden)]
#[macro_export]
macro_rules! __nemo_trace_field {
    (host) => {};
    (status) => {};
    (ms) => {};
    (bytes) => {};
    (tier) => {};
    (attempt) => {};
    (rest) => {};
    ($other:ident) => {
        compile_error!(concat!(
            "`",
            stringify!($other),
            "` is not a trace field; see nemo_core::dev::TRACE_FIELDS"
        ));
    };
}

/// Milliseconds since `since`, for the `ms` field.
#[must_use]
pub fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The host of a URL text, without scheme, user info, port, path, query or fragment, for the
/// `host` field. Gives `unknown` for text that has no host. It does not validate the host.
#[must_use]
pub fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = if host.starts_with('[') {
        host.find(']').map_or(host, |end| &host[..=end])
    } else {
        host.split(':').next().unwrap_or_default()
    };
    if host.is_empty() { "unknown" } else { host }
}

/// A fixed outcome name for one sign-in step, for the `status` field: `ok`, or the kind of
/// the error. Never the error text, which can hold details.
#[must_use]
pub fn account_outcome<T>(result: &Result<T, SourceAccountError>) -> &'static str {
    match result {
        Ok(_) => "ok",
        Err(SourceAccountError::Cancelled) => "cancelled",
        Err(SourceAccountError::Rejected) => "rejected",
        Err(SourceAccountError::Unavailable(_)) => "unavailable",
        Err(SourceAccountError::Storage(_)) => "storage",
        Err(SourceAccountError::Network(_)) => "network",
    }
}

/// A fixed outcome name for one catalog call, for the `status` field: `ok`, or the kind of the
/// error. Never the error text.
#[must_use]
pub fn catalog_outcome<T>(result: &Result<T, CatalogError>) -> &'static str {
    match result {
        Ok(_) => "ok",
        Err(CatalogError::Network(_)) => "network",
        Err(CatalogError::Parse(_)) => "parse",
        Err(CatalogError::Unavailable(_)) => "unavailable",
        Err(CatalogError::NotFound) => "not_found",
        Err(CatalogError::Rejected(_)) => "rejected",
        Err(CatalogError::RateLimited) => "rate_limited",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_is_all_that_is_kept() {
        assert_eq!(
            host_of("https://music.youtube.com/youtubei/v1/search?key=x"),
            "music.youtube.com"
        );
        assert_eq!(
            host_of("https://user:pw@api-v2.soundcloud.com:443/me#f"),
            "api-v2.soundcloud.com"
        );
        assert_eq!(host_of("http://[::1]:8080/x"), "[::1]");
        assert_eq!(host_of("example.com/path"), "example.com");
    }

    #[test]
    fn text_without_a_host_is_unknown() {
        assert_eq!(host_of(""), "unknown");
        assert_eq!(host_of("https://"), "unknown");
        assert_eq!(host_of("https:///path"), "unknown");
        assert_eq!(host_of("/just/a/path"), "unknown");
    }

    #[test]
    fn the_field_list_has_no_free_text_name() {
        for name in TRACE_FIELDS {
            assert!(
                matches!(
                    name,
                    "provider"
                        | "op"
                        | "host"
                        | "status"
                        | "ms"
                        | "bytes"
                        | "tier"
                        | "attempt"
                        | "rest"
                ),
                "{name}"
            );
        }
    }

    #[test]
    fn sign_in_outcomes_are_fixed_names() {
        let secret = || "token abc123".to_owned();
        assert_eq!(account_outcome(&Ok::<(), _>(())), "ok");
        assert_eq!(
            account_outcome::<()>(&Err(SourceAccountError::Cancelled)),
            "cancelled"
        );
        assert_eq!(
            account_outcome::<()>(&Err(SourceAccountError::Rejected)),
            "rejected"
        );
        for error in [
            SourceAccountError::Unavailable(secret()),
            SourceAccountError::Storage(secret()),
            SourceAccountError::Network(secret()),
        ] {
            let name = account_outcome::<()>(&Err(error));
            assert!(!name.contains("abc123"));
        }
    }

    #[test]
    fn catalog_outcomes_are_fixed_names() {
        assert_eq!(catalog_outcome(&Ok::<(), _>(())), "ok");
        assert_eq!(
            catalog_outcome::<()>(&Err(CatalogError::RateLimited)),
            "rate_limited"
        );
        let name = catalog_outcome::<()>(&Err(CatalogError::Network("https://x/?k=SECRET".into())));
        assert_eq!(name, "network");
    }

    #[test]
    fn elapsed_ms_is_small_for_a_fresh_instant() {
        assert!(elapsed_ms(Instant::now()) < 1000);
    }
}
