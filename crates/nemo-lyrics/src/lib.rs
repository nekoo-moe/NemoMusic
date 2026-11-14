//! Lyrics parsers, match scoring, provider chain and cache (ADR 0002).
//!
//! - [`lrc`]: LRC and enhanced LRC parser.
//! - [`plain`]: unsynced text.
//! - [`normalize`](mod@normalize): string normalization for matching.
//! - [`score`]: match score and wrong-match rejection.
//! - [`pick`]: choose the best candidate (Syllable > Word > Line > Unsynced, then score, then
//!   chain order).
//! - [`ttml`]: Apple-style TTML parser (line, word and syllable timing).
//! - [`lrclib`], [`local`], [`unison`], [`blapi`], [`simpmusic`], [`netease`]: providers. Open
//!   sources are on by default; grey ones are opt-in (see each module and ADR 0002). Each
//!   reports its finest level through `LyricsProvider::best_granularity`.
//! - [`http`]: shared request helpers.
//! - [`chain`]: run providers with timeouts and a per-host limit.
//! - [`cache`]: SQLite cache and per-track offsets.
//! - [`service`]: `LyricsHub`, the `LyricsService` implementation.
#![cfg_attr(
    test,
    allow(
        clippy::duration_suboptimal_units,
        reason = "tests spell short durations in seconds, as the timings they check"
    )
)]

pub mod blapi;
pub mod cache;
pub mod chain;
pub mod http;
pub mod local;
pub mod lrc;
pub mod lrclib;
pub mod netease;
pub mod normalize;
pub mod pick;
pub mod plain;
pub mod score;
pub mod service;
pub mod simpmusic;
pub mod ttml;
pub mod unison;

pub use lrc::parse_lrc;
pub use normalize::normalize;
pub use pick::{Candidate, pick_best};
pub use plain::parse_plain;
pub use score::{MatchScore, reject_wrong_match, score_match};

pub use cache::LyricsCache;
pub use chain::{ChainEntry, ChainOptions, ChainResult, run_chain, run_chain_all};
pub use local::LocalProvider;
pub use lrclib::LrclibProvider;
pub use service::{LyricsHub, track_key};
pub use ttml::{TtmlDoc, parse_ttml, parse_ttml_detailed};
pub use unison::UnisonProvider;
