//! The two listening areas of the app (source-workspaces spec): YouTube Music and SoundCloud.
//! One is active at a time and the sidebar, Library, Search, Home and Explore follow it; nothing
//! mixes. This module is pure: which pages each area offers and who owns a page id. The app
//! keeps the active area, and the settings keep the choice between starts.

use nemo_core::catalog::BrowseId;

use crate::route::{NavEntry, Route};

/// Every SoundCloud page id starts with this (the SoundCloud crate's `BrowseId`s).
pub const SOUNDCLOUD_ID_PREFIX: &str = "sc:";

/// The sidebar entries of YouTube Music, in order.
const YOUTUBE_NAV: [NavEntry; 4] = [
    NavEntry::Home,
    NavEntry::Explore,
    NavEntry::Search,
    NavEntry::Library,
];

/// The sidebar entries of SoundCloud, in order. The first one is the landing page: the Library
/// holds the likes and playlists the person came for, and SoundCloud has no Home or Explore in
/// the app (nothing wires `nemo-soundcloud`'s home and charts into a screen yet).
const SOUNDCLOUD_NAV: [NavEntry; 2] = [NavEntry::Library, NavEntry::Search];

/// The listening area the whole window follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Source {
    /// YouTube Music, and the local files that live beside it.
    #[default]
    YouTube,
    /// The signed-in SoundCloud account.
    SoundCloud,
}

impl Source {
    /// The choices in switch order.
    pub const ALL: [Self; 2] = [Self::YouTube, Self::SoundCloud];

    /// The text of the choice. It is also what the settings file stores.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::YouTube => "YouTube Music",
            Self::SoundCloud => "SoundCloud",
        }
    }

    /// The choice with this label; `None` for anything else (an older or newer file).
    #[must_use]
    pub fn from_label(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.label() == text)
    }

    /// The place of the choice in [`Self::ALL`].
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::YouTube => 0,
            Self::SoundCloud => 1,
        }
    }

    /// The sidebar entries this area offers, in order.
    #[must_use]
    pub const fn nav(self) -> &'static [NavEntry] {
        match self {
            Self::YouTube => &YOUTUBE_NAV,
            Self::SoundCloud => &SOUNDCLOUD_NAV,
        }
    }

    /// The page the area opens on: Home for YouTube Music, the first entry for SoundCloud.
    #[must_use]
    pub fn landing(self) -> Route {
        self.nav()
            .first()
            .map_or(Route::Library, |entry| entry.route())
    }

    /// The place of `entry` in [`Self::nav`], if the area offers it.
    #[must_use]
    pub fn nav_index(self, entry: NavEntry) -> Option<usize> {
        self.nav().iter().position(|e| *e == entry)
    }

    /// `true` if the area can show `route`. Home, Explore and the mood pages exist only in
    /// YouTube Music; every other page opens in both (an album page opened from Now Playing
    /// stays open whichever service the song came from).
    #[must_use]
    pub fn allows(self, route: &Route) -> bool {
        match route {
            Route::Home | Route::Explore | Route::Mood { .. } => self == Self::YouTube,
            _ => true,
        }
    }

    /// `true` if a page with this id belongs to the area. SoundCloud ids carry the
    /// `sc:` prefix; everything else (YouTube Music pages and local albums) is YouTube Music's.
    #[must_use]
    pub fn owns_page(self, id: &BrowseId) -> bool {
        id.0.starts_with(SOUNDCLOUD_ID_PREFIX) == (self == Self::SoundCloud)
    }

    /// `true` for a detail page that belongs to the other area (opened from Now Playing).
    #[must_use]
    pub fn is_foreign(self, route: &Route) -> bool {
        match route {
            Route::Album(id) | Route::Artist(id) | Route::Playlist(id) => !self.owns_page(id),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(id: &str) -> BrowseId {
        BrowseId(id.to_owned())
    }

    #[test]
    fn there_are_two_areas_and_youtube_music_is_the_default() {
        assert_eq!(Source::ALL.len(), 2);
        assert_eq!(Source::default(), Source::YouTube);
        assert_eq!(Source::YouTube.label(), "YouTube Music");
        assert_eq!(Source::SoundCloud.label(), "SoundCloud");
    }

    #[test]
    fn a_label_round_trips_and_anything_else_is_unknown() {
        for s in Source::ALL {
            assert_eq!(Source::from_label(s.label()), Some(s));
            assert_eq!(Source::ALL[s.index()], s);
        }
        assert_eq!(Source::from_label(""), None);
        assert_eq!(Source::from_label("All"), None);
        assert_eq!(Source::from_label("soundcloud"), None);
    }

    #[test]
    fn youtube_music_keeps_its_four_entries_and_lands_on_home() {
        assert_eq!(
            Source::YouTube.nav(),
            [
                NavEntry::Home,
                NavEntry::Explore,
                NavEntry::Search,
                NavEntry::Library
            ]
        );
        assert_eq!(Source::YouTube.landing(), Route::Home);
    }

    #[test]
    fn soundcloud_has_library_and_search_only_and_lands_on_its_first_entry() {
        assert_eq!(
            Source::SoundCloud.nav(),
            [NavEntry::Library, NavEntry::Search]
        );
        assert_eq!(Source::SoundCloud.landing(), Route::Library);
        assert_eq!(Source::SoundCloud.nav_index(NavEntry::Search), Some(1));
        assert_eq!(Source::SoundCloud.nav_index(NavEntry::Home), None);
        assert_eq!(Source::SoundCloud.nav_index(NavEntry::Explore), None);
    }

    #[test]
    fn home_explore_and_moods_belong_to_youtube_music_alone() {
        let mood = Route::Mood {
            id: page("m"),
            title: "Chill".to_owned(),
        };
        for route in [Route::Home, Route::Explore, mood] {
            assert!(Source::YouTube.allows(&route));
            assert!(!Source::SoundCloud.allows(&route));
        }
        for route in [
            Route::Library,
            Route::Search(String::new()),
            Route::Settings,
            Route::Queue,
            Route::NowPlaying,
            Route::Album(page("x")),
        ] {
            assert!(Source::YouTube.allows(&route) && Source::SoundCloud.allows(&route));
        }
    }

    #[test]
    fn a_page_belongs_to_the_area_its_id_names() {
        assert!(Source::SoundCloud.owns_page(&page("sc:playlist:1")));
        assert!(!Source::YouTube.owns_page(&page("sc:playlist:1")));
        assert!(Source::YouTube.owns_page(&page("MPREb_x")));
        assert!(Source::YouTube.owns_page(&page("local-album:a")));
        assert!(!Source::SoundCloud.owns_page(&page("MPREb_x")));
    }

    #[test]
    fn only_a_detail_page_of_the_other_area_is_foreign() {
        let sc = Route::Playlist(page("sc:playlist:1"));
        let yt = Route::Album(page("MPREb_x"));
        assert!(Source::YouTube.is_foreign(&sc) && !Source::YouTube.is_foreign(&yt));
        assert!(Source::SoundCloud.is_foreign(&yt) && !Source::SoundCloud.is_foreign(&sc));
        assert!(!Source::SoundCloud.is_foreign(&Route::Library));
        assert!(!Source::SoundCloud.is_foreign(&Route::NowPlaying));
    }
}
