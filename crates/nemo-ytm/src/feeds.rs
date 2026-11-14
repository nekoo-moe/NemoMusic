//! Tolerant readers for YouTube Music pages: home, explore, moods, library, history, playlists
//! and podcasts.
//!
//! `ytmapi-rs` 0.3.4 parses a fixed set of layouts and fails a whole page when one part is
//! unknown, so these readers work on the raw JSON instead. They look for the renderers they
//! know, classify each entry by its navigation endpoint (page type and id prefix, which do not
//! change with the account language) and drop what they cannot understand, so one odd entry
//! never hides the rest of a page. Layouts were checked against live answers on 2026-11-11 and
//! cross-checked with `ytmusicapi` (`parsers/browsing.py`, `parsers/library.py`,
//! `parsers/podcasts.py`, `mixins/explore.py`).

use std::time::Duration;

mod item;
mod json;
mod list;
mod playlist;
mod runs;
mod shelf;

pub(crate) use item::split_mood_id;
pub(crate) use json::{find_key, text_of, thumbnails_of};
pub(crate) use list::{
    Page, parse_library_albums, parse_library_artists, parse_library_playlists, parse_song_rows,
};
pub(crate) use playlist::{
    PlaylistHeader, parse_history, parse_playlist_header, parse_podcast, parse_watch_playlist,
};
pub(crate) use shelf::{parse_charts, parse_explore, parse_home_page, parse_mood_page};

/// Most shelves kept from one page.
const MAX_SHELVES: usize = 24;
/// Most items kept per shelf.
const MAX_ITEMS: usize = 60;
/// Longest `params` blob accepted in a mood id.
const MAX_PARAMS: usize = 512;
/// Most tracks kept from a watch playlist (a radio can be endless).
const MAX_WATCH_TRACKS: usize = 200;

const GREY_OUT: &str = "MUSIC_ITEM_RENDERER_DISPLAY_POLICY_GREY_OUT";

/// Pause between the parts of a long list, so a page of results is fetched at human pace.
pub(crate) const PAGE_PAUSE: Duration = Duration::from_millis(400);

#[cfg(test)]
#[allow(
    clippy::assert_is_empty,
    clippy::needless_pass_by_value,
    reason = "readable test assertions; helpers take JSON builders by value"
)]
mod tests;
