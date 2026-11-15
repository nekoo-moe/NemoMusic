//! YouTube Music client: sign-in, stream resolver, SponsorBlock and timed lyrics.
//!
//! Access is unofficial and may breach YouTube's Terms of Service (see `coc.md`).
//! Stream resolution is anonymous: no cookies are ever passed to `yt-dlp`.
//!
//! Services run their network work on a `tokio` runtime handle given at construction and return
//! futures that any executor can poll (see the app-mvp addendum). No URL is ever logged.
//! Account cookies, names and ids are never logged either.
#![cfg_attr(
    test,
    allow(
        clippy::duration_suboptimal_units,
        unknown_lints,
        reason = "tests spell short durations in seconds; clippy 1.97 does not know \
                  assert_is_empty, which the test modules allow for newer clippy"
    )
)]

pub mod account;
pub mod catalog;
pub mod credentials;
pub mod diagnostics;
pub(crate) mod feeds;
pub mod home_feed;
pub mod http;
pub mod images;
pub mod innertube;
pub mod offline;
pub(crate) mod pages;
pub mod raw;
pub mod resolver;
pub mod session;
pub mod sponsorblock;
pub mod stream;
pub mod web_search;
pub mod ytdlp;
pub(crate) mod ytdlp_install;

#[cfg(test)]
#[allow(
    missing_docs,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::cast_possible_truncation,
    reason = "test helper"
)]
mod test_server;
