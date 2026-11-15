//! The local play log and every listening-statistics aggregate (listening-stats spec,
//! ADR 0004).
//!
//! This crate depends on `nemo-core` only and has no network code. The database, the charts
//! and the `StatsService` implementation come in later tasks.
//!
//! - [`service`]: [`StatsStore`], [`StoreError`] and [`counts_as_play`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod service;

pub use service::{StatsStore, StoreError, counts_as_play};
