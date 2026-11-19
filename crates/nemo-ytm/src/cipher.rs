//! Resolving streams in process, with a local JavaScript engine (cold-start spec).
//!
//! Some `player` clients answer with ciphered stream addresses: a `signatureCipher` whose
//! signature, and an `n` parameter, are computed by functions inside YouTube's player script.
//! `yt-dlp` runs them in a subprocess. This module runs them in the app instead, as SimpMusic and
//! Metrolist do, so a song the plain native request refuses can still be resolved without
//! starting `yt-dlp` (about 2 s here):
//!
//! - [`JsSolver`] runs the `yt-dlp` EJS solver scripts (Unlicense, vendored in `assets/ejs`) in an
//!   embedded QuickJS on one dedicated thread, with a memory limit and a time limit.
//! - [`PlayerStore`] finds the current player script (`iframe_api`, re-checked every 6 hours),
//!   downloads it once and keeps what the solver preprocessed from it, with its signature
//!   timestamp, in memory and in a private file.
//! - [`CipherResolver`] asks the `tv` client (no PO token) with that timestamp, solves what the
//!   answer carries and gives the stream address.
//!
//! Hard limits (`coc.md`): no PO token is made or sent, no attestation is imitated, no cookie
//! or session leaves the app, nothing is decoded by a remote service, and no URL, id or script
//! text is logged. A format that is DRM protected is skipped.

mod player;
mod resolver;
mod solver;

pub use player::{HttpPlayerSource, PLAYER_CHECK_EVERY, PlayerInfo, PlayerSource, PlayerStore};
pub use resolver::{CipherResolver, PlayerRequest, TvRequest};
pub use solver::{
    ChallengeSolver, Challenges, JsSolver, MEMORY_LIMIT, SOLVE_TIMEOUT, Script, SolveError, Solved,
};

#[cfg(test)]
mod tests;
