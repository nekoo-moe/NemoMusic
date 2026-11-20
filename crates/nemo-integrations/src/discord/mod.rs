//! Discord Rich Presence (discord-presence spec): every state the app shows to the local Discord
//! app, as pure rules plus a thin I/O shell.
//!
//! - [`text`]: [`fit`] cuts and pads a line to the 2 to 128 bytes Discord accepts.
//! - [`activity`]: [`Activity`], the thing Discord shows, and its JSON.
//! - [`state`]: [`PresenceState`] and [`build`], which holds every text and image rule.
//! - [`limiter`]: [`RateLimiter`], at most one update per slot, the newest winning.
//! - [`away`]: [`AwayRule`], when the app shows `Away`.
//! - [`session`]: folds play events and window facts into a [`PresenceState`].
//! - [`engine`]: [`Engine`], the pure brain of the worker (limiter, dedupe, connect schedule).
//! - [`ipc`]: the frame format and the socket or pipe of the local Discord app.
//! - [`worker`]: the one thread that owns the connection.
//!
//! Nothing here reads a clock or does I/O: callers pass instants and wall times in.

pub mod activity;
pub mod away;
pub mod engine;
pub mod ipc;
pub mod limiter;
pub mod session;
pub mod state;
pub mod text;
pub mod worker;

pub use activity::{Activity, ActivityKind};
pub use away::AwayRule;
pub use engine::{Action, Engine, Input, Timing};
pub use limiter::RateLimiter;
pub use session::Session;
pub use state::{Browsing, Place, PresenceState, Song, Source, build};
pub use text::fit;
pub use worker::{Config, Worker};
