//! Routes and back/forward history. Pure: the content router in the GUI matches on [`Route`], so
//! Search, Album, Artist, Playlist, Queue and Settings only add a match arm.

use nemo_core::catalog::BrowseId;

/// A screen.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Route {
    /// Home feed.
    Home,
    /// Search with its query.
    Search(String),
    /// Local and saved library.
    Library,
    /// Queue.
    Queue,
    /// Settings.
    Settings,
    /// Album page.
    Album(BrowseId),
    /// Artist page.
    Artist(BrowseId),
    /// Playlist page.
    Playlist(BrowseId),
    /// Full-window Now Playing.
    NowPlaying,
}

/// The sidebar entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavEntry {
    /// Home.
    Home,
    /// Search.
    Search,
    /// Library.
    Library,
    /// Queue.
    Queue,
    /// Settings.
    Settings,
}

impl NavEntry {
    /// All entries in sidebar order.
    pub const ALL: [Self; 5] = [
        Self::Home,
        Self::Search,
        Self::Library,
        Self::Queue,
        Self::Settings,
    ];

    /// Label shown next to the icon and used as the accessible name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Search => "Search",
            Self::Library => "Library",
            Self::Queue => "Queue",
            Self::Settings => "Settings",
        }
    }

    /// The route this entry opens.
    #[must_use]
    pub fn route(self) -> Route {
        match self {
            Self::Home => Route::Home,
            Self::Search => Route::Search(String::new()),
            Self::Library => Route::Library,
            Self::Queue => Route::Queue,
            Self::Settings => Route::Settings,
        }
    }
}

impl Route {
    /// Which sidebar entry is highlighted on this route (pages under Library highlight Library).
    #[must_use]
    pub fn entry(&self) -> Option<NavEntry> {
        match self {
            Self::Home => Some(NavEntry::Home),
            Self::Search(_) => Some(NavEntry::Search),
            Self::Library | Self::Album(_) | Self::Artist(_) | Self::Playlist(_) => {
                Some(NavEntry::Library)
            }
            Self::Queue => Some(NavEntry::Queue),
            Self::Settings => Some(NavEntry::Settings),
            Self::NowPlaying => None,
        }
    }

    /// Title for the top bar.
    #[must_use]
    pub fn title(&self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Search(_) => "Search",
            Self::Library => "Library",
            Self::Queue => "Queue",
            Self::Settings => "Settings",
            Self::Album(_) => "Album",
            Self::Artist(_) => "Artist",
            Self::Playlist(_) => "Playlist",
            Self::NowPlaying => "Now Playing",
        }
    }
}

/// Back/forward history of routes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nav {
    stack: Vec<Route>,
    index: usize,
}

impl Default for Nav {
    fn default() -> Self {
        Self::new(Route::Home)
    }
}

impl Nav {
    /// History that starts on `route`.
    #[must_use]
    pub fn new(route: Route) -> Self {
        Self {
            stack: vec![route],
            index: 0,
        }
    }

    /// The current route.
    #[must_use]
    pub fn current(&self) -> &Route {
        &self.stack[self.index]
    }

    /// Go to `route`. Going to the current route changes nothing; forward history is dropped.
    pub fn go(&mut self, route: Route) {
        if *self.current() == route {
            return;
        }
        self.stack.truncate(self.index + 1);
        self.stack.push(route);
        self.index += 1;
    }

    /// `true` when [`Self::back`] would move.
    #[must_use]
    pub fn can_back(&self) -> bool {
        self.index > 0
    }

    /// `true` when [`Self::forward`] would move.
    #[must_use]
    pub fn can_forward(&self) -> bool {
        self.index + 1 < self.stack.len()
    }

    /// Step back; returns whether it moved.
    pub fn back(&mut self) -> bool {
        if self.can_back() {
            self.index -= 1;
            true
        } else {
            false
        }
    }

    /// Step forward; returns whether it moved.
    pub fn forward(&mut self) -> bool {
        if self.can_forward() {
            self.index += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_goes_back_and_forward() {
        let mut nav = Nav::default();
        assert!(!nav.can_back() && !nav.can_forward());
        nav.go(Route::Library);
        nav.go(Route::Settings);
        assert_eq!(nav.current(), &Route::Settings);
        assert!(nav.back());
        assert_eq!(nav.current(), &Route::Library);
        assert!(nav.can_forward());
        assert!(nav.forward());
        assert_eq!(nav.current(), &Route::Settings);
        assert!(!nav.forward());
    }

    #[test]
    fn going_somewhere_new_drops_forward_history() {
        let mut nav = Nav::default();
        nav.go(Route::Library);
        nav.back();
        nav.go(Route::Queue);
        assert!(!nav.can_forward());
        nav.back();
        assert_eq!(nav.current(), &Route::Home);
    }

    #[test]
    fn same_route_is_not_pushed_twice() {
        let mut nav = Nav::default();
        nav.go(Route::Home);
        assert!(!nav.can_back());
    }

    #[test]
    fn entries_open_their_routes_and_pages_highlight_library() {
        for entry in NavEntry::ALL {
            assert_eq!(entry.route().entry(), Some(entry), "{}", entry.label());
        }
        assert_eq!(
            Route::Album(BrowseId("x".into())).entry(),
            Some(NavEntry::Library)
        );
        assert_eq!(Route::NowPlaying.entry(), None);
    }
}
