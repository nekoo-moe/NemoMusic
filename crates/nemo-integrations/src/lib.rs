//! OS media controls, Last.fm scrobbling, Discord presence and OS notifications (integrations
//! spec, ADR 0004).
//!
//! `nemo-app` builds one [`Integrations`] with [`build`] and hands it to the UI as the
//! `nemo-core` `IntegrationsService` and to the play hub as a `PlayObserver`. This crate
//! depends on `nemo-core` only.
//!
//! - [`info`]: the settings rows of the three integrations.
//! - [`notify`]: the text of OS notifications.
//! - [`service`]: the service and its observer fan-out. The workers (media controls, Last.fm,
//!   Discord) come in later tasks; until then every switch is in memory only and nothing
//!   leaves the computer.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod info;
pub mod notify;
pub mod service;

pub use service::{DISCORD_APPLICATION_ID, Integrations, build};
