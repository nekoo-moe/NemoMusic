//! Cast and DLNA discovery and control, and the LAN-only stream server (cast spec, ADR 0004).
//!
//! `nemo-app` builds one [`CastService`]. All network work of this crate runs on the service's
//! own tokio runtime; the audio thread is never touched. This crate depends on `nemo-core`
//! only. Discovery, sessions and the server come in later tasks; until then the service holds
//! its runtime and does nothing else.
//!
//! - [`service`]: the service, its runtime holder and [`CastError`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod service;

pub use service::{CastError, CastService};
