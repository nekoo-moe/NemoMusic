//! Developer tools (dev-tools spec): the provider trace convention, and later the read-only
//! contract the hidden developer panel reads.
//!
//! No secret can be expressed here. A trace event holds a fixed vocabulary only: provider, short
//! operation name, host, status, milliseconds and a few counters. Never a URL, query, id, title,
//! token, cookie or user name.

mod trace;

pub use trace::{TRACE_FIELDS, TRACE_TARGET, account_outcome, elapsed_ms, host_of};
