//! Discord Rich Presence (discord-presence spec): every state the app shows to the local Discord
//! app, as pure rules plus a thin I/O shell.
//!
//! - [`text`]: [`fit`] cuts and pads a line to the 2 to 128 bytes Discord accepts.
//! - [`activity`]: [`Activity`], the thing Discord shows, and its JSON.
//! - [`state`]: [`PresenceState`] and [`build`], which holds every text and image rule.
//! - [`limiter`]: [`RateLimiter`], at most one update per slot, the newest winning.
//! - [`away`]: [`AwayRule`], when the app shows `Away`.
//!
//! Nothing here reads a clock or does I/O: callers pass instants and wall times in.

pub mod activity;
pub mod away;
pub mod limiter;
pub mod state;
pub mod text;

pub use activity::{Activity, ActivityKind};
pub use away::AwayRule;
pub use limiter::RateLimiter;
pub use state::{Browsing, Place, PresenceState, Song, Source, build};
pub use text::fit;
