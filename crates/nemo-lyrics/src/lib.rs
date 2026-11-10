//! Lyrics parsers, match scoring, provider chain and cache (ADR 0002).
//!
//! This part is pure logic: no network, no async, no database.
//! - [`lrc`]: LRC and enhanced LRC parser.
//! - [`plain`]: unsynced text.
//! - [`normalize`](mod@normalize): string normalization for matching.
//! - [`score`]: match score and wrong-match rejection.
//! - [`pick`]: choose the best candidate.

pub mod lrc;
pub mod normalize;
pub mod pick;
pub mod plain;
pub mod score;

pub use lrc::parse_lrc;
pub use normalize::normalize;
pub use pick::{Candidate, pick_best};
pub use plain::parse_plain;
pub use score::{MatchScore, reject_wrong_match, score_match};
