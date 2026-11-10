//! Lyrics parsers, match scoring, provider chain and cache (ADR 0002).
//!
//! - [`lrc`]: LRC and enhanced LRC parser.
//! - [`plain`]: unsynced text.
//! - [`normalize`](mod@normalize): string normalization for matching.
//! - [`score`]: match score and wrong-match rejection.
//! - [`pick`]: choose the best candidate.
//! - [`lrclib`], [`local`]: providers.
//! - [`chain`]: run providers with timeouts and a per-host limit.
//! - [`cache`]: SQLite cache and per-track offsets.
//! - [`service`]: `LyricsHub`, the `LyricsService` implementation.

pub mod cache;
pub mod chain;
pub mod local;
pub mod lrc;
pub mod lrclib;
pub mod normalize;
pub mod pick;
pub mod plain;
pub mod score;
pub mod service;

pub use lrc::parse_lrc;
pub use normalize::normalize;
pub use pick::{Candidate, pick_best};
pub use plain::parse_plain;
pub use score::{MatchScore, reject_wrong_match, score_match};

pub use cache::LyricsCache;
pub use chain::{ChainEntry, ChainOptions, ChainResult, run_chain};
pub use local::LocalProvider;
pub use lrclib::LrclibProvider;
pub use service::{LyricsHub, track_key};
