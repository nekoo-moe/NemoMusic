//! User interface: theme tokens, views, widgets and album-art effects (see `design.md`).
//!
//! Pure logic (theme, palette, layout, motion, state reducer, mock backend) is always compiled and
//! tested without a window. Everything that needs `iced` lives in [`gui`] behind the `gui`
//! feature.

pub mod cover;
pub mod format;
pub mod frame_stats;
pub mod keys;
pub mod layout;
pub mod lyrics_view;
pub mod mock;
pub mod motion;
pub mod palette;
pub mod route;
pub mod state;
pub mod theme;

#[cfg(feature = "gui")]
pub mod gui;
#[cfg(feature = "gui")]
pub use gui::{UiOptions, run};
