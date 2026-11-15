//! OS notifications: the text of each notification, the Linux notifier, and later the Windows
//! one, behind the `nemo-core` `OsNotifier`.
//!
//! - [`compose`]: the plain text of a release notification. Pure.
//! - `linux` (Linux only): `LinuxNotifier`, freedesktop notifications over `zbus`.

pub mod compose;
#[cfg(target_os = "linux")]
pub mod linux;

pub use compose::{release_notification, sanitize};
#[cfg(target_os = "linux")]
pub use linux::LinuxNotifier;
