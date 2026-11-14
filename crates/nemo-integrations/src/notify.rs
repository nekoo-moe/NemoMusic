//! OS notifications: the text of each notification, and later the Windows and Linux notifiers
//! behind the `nemo-core` `OsNotifier`.
//!
//! - [`compose`]: the plain text of a release notification. Pure.

pub mod compose;

pub use compose::{release_notification, sanitize};
