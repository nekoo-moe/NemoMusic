//! OS media controls, Last.fm scrobbling, Discord presence and OS notifications (integrations
//! spec, ADR 0004).
//!
//! `nemo-app` builds one [`Integrations`] with [`build`] and hands it to the UI as the
//! `nemo-core` `IntegrationsService` and to the play hub as a `PlayObserver`. This crate
//! depends on `nemo-core` only.
//!
//! - [`info`]: the settings rows of the three integrations.
//! - [`lastfm`]: the signed Last.fm API client and the scrobble queue.
//! - [`notify`]: the text of OS notifications.
//! - [`service`]: the service and its observer fan-out. The workers (media controls, Last.fm,
//!   Discord) come in later tasks; until then every switch is in memory only and nothing
//!   leaves the computer.
//! - [`store`]: the integrations database (`integrations.sqlite3`): switches and the queue.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod info;
pub mod lastfm;
pub mod notify;
pub mod service;
pub mod store;

pub use service::{DISCORD_APPLICATION_ID, Integrations, build};
