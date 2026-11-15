//! Playlist import (playlist-import design): matching songs from an exported playlist file
//! against the YouTube Music catalog.
//!
//! - [`normalize`]: text normalization for matching.
//! - [`score`]: the score and confidence of a catalog result.

pub mod normalize;
pub mod score;
