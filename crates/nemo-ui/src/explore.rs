//! Pure models for the Explore page, the header content chips, the Library page options, the
//! sleep timer and the shortcut sheet. No GUI dependency.
//!
//! Hook points for the parent: moods and genres are a fixed list here (each one runs a catalog
//! search for its query today); when the catalog gains real mood and genre calls, feed their
//! results through [`crate::gui::app::Message::ExploreLoaded`] and the page shows them instead.

use std::time::Duration;

/// A mood or genre tile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mood {
    /// Tile label.
    pub label: &'static str,
    /// What the catalog is asked for when the tile is pressed.
    pub query: &'static str,
    /// Tile hue in degrees (the tile colour is derived from it for the active surface mode).
    pub hue: f64,
}

/// Moods first, then genres (the YouTube Music and Spotify browse pages use the same idea).
pub const MOODS: [Mood; 16] = [
    Mood {
        label: "Energize",
        query: "energizing upbeat music",
        hue: 40.0,
    },
    Mood {
        label: "Feel good",
        query: "feel good songs",
        hue: 85.0,
    },
    Mood {
        label: "Relax",
        query: "relaxing music",
        hue: 200.0,
    },
    Mood {
        label: "Workout",
        query: "workout music",
        hue: 25.0,
    },
    Mood {
        label: "Commute",
        query: "commute music",
        hue: 250.0,
    },
    Mood {
        label: "Party",
        query: "party music",
        hue: 330.0,
    },
    Mood {
        label: "Focus",
        query: "focus music",
        hue: 170.0,
    },
    Mood {
        label: "Romance",
        query: "romantic songs",
        hue: 5.0,
    },
    Mood {
        label: "Sad",
        query: "sad songs",
        hue: 270.0,
    },
    Mood {
        label: "Sleep",
        query: "sleep music",
        hue: 285.0,
    },
    Mood {
        label: "Pop",
        query: "pop hits",
        hue: 350.0,
    },
    Mood {
        label: "Hip-hop",
        query: "hip hop",
        hue: 60.0,
    },
    Mood {
        label: "Rock",
        query: "rock classics",
        hue: 15.0,
    },
    Mood {
        label: "Lo-fi",
        query: "lofi beats",
        hue: 150.0,
    },
    Mood {
        label: "Indie",
        query: "indie music",
        hue: 120.0,
    },
    Mood {
        label: "Electronic",
        query: "electronic music",
        hue: 225.0,
    },
];

/// Header chips of Home: what kind of content to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentFilter {
    /// Everything.
    #[default]
    All,
    /// Music (songs, albums, artists, playlists).
    Music,
    /// Videos. Hook point: the catalog has no video items yet, so the page says so.
    Videos,
    /// Podcasts. Hook point: same.
    Podcasts,
}

impl ContentFilter {
    /// All chips in order.
    pub const ALL: [Self; 4] = [Self::All, Self::Music, Self::Videos, Self::Podcasts];

    /// Chip label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Music => "Music",
            Self::Videos => "Videos",
            Self::Podcasts => "Podcasts",
        }
    }
}

/// Tabs of the Library page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibTab {
    /// Everything at a glance: account sections, then local files.
    #[default]
    Overview,
    /// Playlists.
    Playlists,
    /// Songs.
    Songs,
    /// Albums.
    Albums,
    /// Artists.
    Artists,
    /// Files from the music folders.
    Local,
}

impl LibTab {
    /// All tabs in order.
    pub const ALL: [Self; 6] = [
        Self::Overview,
        Self::Playlists,
        Self::Songs,
        Self::Albums,
        Self::Artists,
        Self::Local,
    ];

    /// Tab label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Playlists => "Playlists",
            Self::Songs => "Songs",
            Self::Albums => "Albums",
            Self::Artists => "Artists",
            Self::Local => "Local",
        }
    }
}

/// Sort orders of the Library page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibSort {
    /// The order the account or the scan gave.
    #[default]
    Recent,
    /// By title, A to Z.
    Title,
    /// By artist, A to Z.
    Artist,
}

impl LibSort {
    /// All orders in menu order.
    pub const ALL: [Self; 3] = [Self::Recent, Self::Title, Self::Artist];

    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Recent => "Recent",
            Self::Title => "Title",
            Self::Artist => "Artist",
        }
    }

    /// The next order (the sort button cycles).
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Recent => Self::Title,
            Self::Title => Self::Artist,
            Self::Artist => Self::Recent,
        }
    }
}

/// List or grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibLayout {
    /// Rows.
    #[default]
    List,
    /// Cards.
    Grid,
}

/// Sort `items` in place by `key` (case-insensitive, accents kept as they are).
pub fn sort_by_key<T>(
    items: &mut [T],
    sort: LibSort,
    title: impl Fn(&T) -> String,
    artist: impl Fn(&T) -> String,
) {
    match sort {
        LibSort::Recent => {}
        LibSort::Title => items.sort_by_key(|i| title(i).to_lowercase()),
        LibSort::Artist => {
            items.sort_by_key(|i| (artist(i).to_lowercase(), title(i).to_lowercase()));
        }
    }
}

/// Sleep timer choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepChoice {
    /// After this many minutes.
    Minutes(u32),
    /// When the current song ends.
    EndOfSong,
}

impl SleepChoice {
    /// The choices in menu order.
    pub const ALL: [Self; 5] = [
        Self::Minutes(15),
        Self::Minutes(30),
        Self::Minutes(45),
        Self::Minutes(60),
        Self::EndOfSong,
    ];

    /// Menu label.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Minutes(m) => format!("{m} min"),
            Self::EndOfSong => "End of song".to_owned(),
        }
    }

    /// The wait, or `None` for the end of the song.
    #[must_use]
    pub fn duration(self) -> Option<Duration> {
        match self {
            Self::Minutes(m) => Some(Duration::from_mins(u64::from(m))),
            Self::EndOfSong => None,
        }
    }
}

/// `12:34` or `1:02:03` left on a sleep timer.
#[must_use]
pub fn timer_left(left: Duration) -> String {
    crate::format::clock(left)
}

/// Playback speeds offered when the engine supports them (percent). Off by default: any speed
/// other than 100 resamples the audio and ends bit-perfect output.
pub const SPEEDS: [u16; 5] = [75, 100, 125, 150, 200];

/// The shortcut sheet: (keys, what it does). Single letters are off while a text field has focus.
pub const SHORTCUTS: [(&str, &str); 16] = [
    ("Space", "Play or pause"),
    ("Left / Right", "Seek 5 s (Shift: 30 s)"),
    ("Up / Down", "Volume up or down 5%"),
    ("N / P", "Next or previous song"),
    ("/  or  Ctrl+K", "Search"),
    ("Ctrl+,", "Settings"),
    ("[  /  ]", "Lyrics earlier or later by 50 ms"),
    ("F", "Full-screen Now Playing with big lyrics"),
    ("M", "Mini player"),
    ("Q", "Show or hide the queue"),
    ("L", "Like the current song"),
    ("S", "Sleep timer and more"),
    ("Tab", "Move focus"),
    ("F3", "Frame-time readout"),
    ("Esc", "Close what is open, or go back"),
    ("?", "This sheet"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moods_have_distinct_labels_and_queries() {
        for (i, a) in MOODS.iter().enumerate() {
            assert!(!a.query.is_empty() && (0.0..360.0).contains(&a.hue));
            for b in MOODS.iter().skip(i + 1) {
                assert_ne!(a.label, b.label);
                assert_ne!(a.query, b.query);
            }
        }
    }

    #[test]
    fn sort_orders_by_title_then_artist() {
        let mut v = vec![("b", "z"), ("a", "z"), ("c", "a")];
        sort_by_key(
            &mut v,
            LibSort::Title,
            |i| i.0.to_owned(),
            |i| i.1.to_owned(),
        );
        assert_eq!(v[0].0, "a");
        sort_by_key(
            &mut v,
            LibSort::Artist,
            |i| i.0.to_owned(),
            |i| i.1.to_owned(),
        );
        assert_eq!(v[0].0, "c");
        assert_eq!(LibSort::Recent.next().next().next(), LibSort::Recent);
    }

    #[test]
    fn sleep_choices_have_durations() {
        assert_eq!(
            SleepChoice::Minutes(15).duration(),
            Some(Duration::from_mins(15))
        );
        assert_eq!(SleepChoice::EndOfSong.duration(), None);
        assert_eq!(SleepChoice::Minutes(45).label(), "45 min");
    }

    #[test]
    fn speed_off_is_the_default_one() {
        assert!(SPEEDS.contains(&100));
    }
}
