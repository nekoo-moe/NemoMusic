//! The Listen Together room client (listen-together spec, ADR 0004).
//!
//! This crate depends on `nemo-core` only. No server address is built in: the user types one.
//! The codec, the room and the drift loop come in later tasks.
//!
//! - [`address`]: [`ServerAddress`], the validated server address.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod address;

pub use address::ServerAddress;
