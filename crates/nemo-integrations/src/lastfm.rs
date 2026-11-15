//! Last.fm scrobbling.
//!
//! - [`api`]: the signed Last.fm API client (`track.updateNowPlaying`, `track.scrobble`).
//! - [`queue`]: the local queue of finished plays that survives restarts.

pub mod api;
pub mod queue;

#[cfg(test)]
mod test_server;
