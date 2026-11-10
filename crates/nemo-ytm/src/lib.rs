//! YouTube Music client: sign-in, stream resolver, SponsorBlock and timed lyrics.
//!
//! Access is unofficial and may breach YouTube's Terms of Service (see `coc.md`).
//! Stream resolution is anonymous: no cookies are ever passed to `yt-dlp`.
//!
//! Services run their network work on a `tokio` runtime handle given at construction and return
//! futures that any executor can poll (see the app-mvp addendum). No URL is ever logged.
//! Account cookies, names and ids are never logged either.

pub mod account;
pub mod catalog;
pub mod credentials;
pub mod home_feed;
pub mod http;
pub mod images;
pub mod session;
pub mod sponsorblock;
pub mod stream;
pub mod ytdlp;

#[cfg(test)]
#[allow(
    missing_docs,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::cast_possible_truncation,
    reason = "test helper"
)]
mod test_server;
