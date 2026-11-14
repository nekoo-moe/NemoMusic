//! The user's own Spotify session, Spotify lyrics and Canvas animated covers (lyrics-plus spec,
//! ADR 0004).
//!
//! Empty for now: the session (`SpotifySession`, implementing the `nemo-core`
//! `SpotifyAccount`), the lyrics provider and the Canvas source come in later tasks, after the
//! sign-in spike. This crate depends on `nemo-core` only and has no network code yet.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
