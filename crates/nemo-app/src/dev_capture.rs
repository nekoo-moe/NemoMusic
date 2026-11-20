//! The developer ring (dev-tools spec): a bounded in-memory store of redacted log and provider
//! trace events that the hidden developer panel reads.
//!
//! Nothing here is written to disk or sent anywhere. Every text is redacted at capture, and only
//! an allow-list of structured fields is kept.

mod layer;
mod redact;
mod ring;

pub use layer::{CaptureLayer, capture_filter};
pub use redact::Redactor;
pub use ring::{CapturedEvent, DevRing, RING_CAPACITY};
