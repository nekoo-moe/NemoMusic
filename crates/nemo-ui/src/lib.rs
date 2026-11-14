//! User interface: theme tokens, views, widgets and album-art effects (see `design.md`).
//!
//! Pure logic (theme, palette, layout, motion, state reducer, search, home model, mock backend) is
//! always compiled and tested without a window. Everything that needs `iced` lives in [`gui`]
//! behind the `gui` feature.

#![allow(clippy::struct_excessive_bools)] // settings, UI state and the app are bundles of independent flags
#![cfg_attr(
    test,
    allow(
        clippy::duration_suboptimal_units,
        reason = "tests spell short durations in seconds, as the timings they check"
    )
)]

pub mod anim;
pub mod canvas;
pub mod copy;
pub mod cover;
pub mod daily;
pub mod explore;
pub mod format;
pub mod frame_stats;
pub mod home;
pub mod keys;
pub mod layout;
pub mod lru;
pub mod lyric_pieces;
pub mod lyrics_view;
pub mod mock;
pub mod mock_lyrics;
pub mod motion;
pub mod palette;
pub mod route;
pub mod scheme;
pub mod search;
pub mod settings;
pub mod signal;
pub mod state;
pub mod theme;

pub use settings::{GlassTier, UiSettings};

#[cfg(feature = "gui")]
pub mod gui;
#[cfg(feature = "gui")]
pub use gui::{UiOptions, run};
