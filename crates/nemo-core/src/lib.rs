//! `NemoMusic` domain types, traits and events.
//!
//! This crate has no I/O, no async runtime and no GUI. Every other `nemo-*` crate may depend on
//! it; it depends on no other `nemo-*` crate (see `structure.md`).

pub mod account;
pub mod audio;
pub mod catalog;
pub mod lyrics;
pub mod player;
pub mod secret;
pub mod secret_store;
pub mod segment;
pub mod services;
pub mod stream;
pub mod track;

use std::future::Future;
use std::pin::Pin;

/// A boxed, sendable future. Lets traits stay object-safe without an `async-trait` dependency.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
