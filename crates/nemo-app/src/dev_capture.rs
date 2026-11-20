//! The developer ring (dev-tools spec): a bounded in-memory store of redacted log and provider
//! trace events that the hidden developer panel reads.
//!
//! Nothing here is written to disk or sent anywhere. Every text is redacted at capture, and only
//! an allow-list of structured fields is kept.

#![allow(
    dead_code,
    unused_imports,
    reason = "the probe and the panel that read the ring arrive in the next dev-tools PRs"
)]

mod layer;
mod redact;
mod ring;

pub use layer::{ALLOWED_FIELDS, CaptureLayer, TRACE_TARGET, capture_filter};
pub use redact::Redactor;
pub use ring::{CapturedEvent, DevRing, RING_CAPACITY};
