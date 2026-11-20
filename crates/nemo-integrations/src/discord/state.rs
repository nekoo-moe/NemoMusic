//! What the app shows on Discord, and how each state looks (discord-presence spec, criteria 1 to
//! 12). [`build`] is pure: it holds every text, image and privacy rule.

use nemo_core::track::MediaUrl;

use super::activity::{Activity, ActivityKind, Image};
use super::text::fit;

/// Asset key of the app icon, the large picture when no cover address is usable.
pub const ASSET_APP: &str = "nemo";
/// Asset key of the small picture of a paused song.
pub const ASSET_PAUSED: &str = "paused";
/// Tooltip of the app icon.
const APP_NAME: &str = "NemoMusic";
/// Most bytes of a picture address Discord takes.
const MAX_COVER_BYTES: usize = 256;
/// The hosts a cover address may have: the cover hosts of the README egress list.
const COVER_HOSTS: &[&str] = &[
    "i.ytimg.com",
    "*.ggpht.com",
    "*.googleusercontent.com",
    "*.sndcdn.com",
];

/// Where a song comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// YouTube Music.
    YouTubeMusic,
    /// SoundCloud.
    SoundCloud,
    /// A file on this computer.
    Local,
}

impl Source {
    /// The asset key of the small picture.
    #[must_use]
    pub const fn asset(self) -> &'static str {
        match self {
            Self::YouTubeMusic => "youtube-music",
            Self::SoundCloud => "soundcloud",
            Self::Local => "local",
        }
    }

    /// The name shown in the tooltip of the small picture.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::YouTubeMusic => "YouTube Music",
            Self::SoundCloud => "SoundCloud",
            Self::Local => "Local file",
        }
    }
}

/// Where the sound comes out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Place {
    /// This computer.
    #[default]
    ThisDevice,
    /// A Cast or DLNA device.
    Cast,
    /// A Listen Together room.
    Together,
}

/// The part of the app the user browses while nothing plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Browsing {
    /// The Library.
    Library,
    /// Search.
    Search,
}

/// The facts of one song that Discord may show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Song {
    /// Title.
    pub title: String,
    /// Artist names joined into one line.
    pub artist: String,
    /// Where the song comes from.
    pub source: Source,
    /// The address of the cover, if the app has one. Used only when it passes the cover rules.
    pub cover: Option<String>,
    /// The codec name of a lossless local file (`FLAC`), if known. Never set for other sources.
    pub codec: Option<String>,
}

/// What the app does now, as far as Discord may know it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenceState {
    /// A song plays. `start` is the time (Unix seconds) the song would have started at the
    /// current position; `end` is `start` plus the length, when the length is known.
    Listening {
        /// The song.
        song: Song,
        /// Start time in Unix seconds.
        start: i64,
        /// End time in Unix seconds.
        end: Option<i64>,
        /// Where the sound comes out.
        place: Place,
        /// A sleep timer runs.
        sleep_timer: bool,
    },
    /// A song is paused.
    Paused {
        /// The song.
        song: Song,
        /// Where the sound would come out.
        place: Place,
        /// A sleep timer runs.
        sleep_timer: bool,
    },
    /// A video plays or is paused.
    Video {
        /// Title of the video.
        title: String,
        /// Start time in Unix seconds (ignored while paused).
        start: i64,
        /// End time in Unix seconds.
        end: Option<i64>,
        /// The video is paused.
        paused: bool,
    },
    /// The app is open and nothing plays.
    Idle {
        /// Since when (Unix seconds).
        since: i64,
    },
    /// Nothing plays and the user is in the Library or in Search.
    Browsing {
        /// Which part.
        section: Browsing,
        /// Since when (Unix seconds).
        since: i64,
    },
    /// Nothing played for a while and the window is out of focus (the sleeping rule).
    Sleeping,
}

impl PresenceState {
    /// `true` for a state that holds a title, which the private form hides.
    #[must_use]
    pub const fn has_title(&self) -> bool {
        matches!(
            self,
            Self::Listening { .. } | Self::Paused { .. } | Self::Video { .. }
        )
    }
}

/// `text`, or `fallback` when it holds nothing but white space.
fn or(text: &str, fallback: &str) -> String {
    if text.trim().is_empty() {
        fallback.to_owned()
    } else {
        text.to_owned()
    }
}

fn image(key: &str, text: &str) -> Image {
    Image {
        key: key.to_owned(),
        text: fit(text),
    }
}

/// Build what Discord shows for `state`. With `private`, every state that holds a title shows
/// only `Listening to NemoMusic`.
#[must_use]
pub fn build(state: &PresenceState, private: bool) -> Activity {
    if private && state.has_title() {
        return private_activity();
    }
    let mut a = match state {
        PresenceState::Listening {
            song,
            start,
            end,
            place,
            sleep_timer,
        } => Activity {
            kind: ActivityKind::Listening,
            details: or(&song.title, "Unknown title"),
            state: Some(or(&song.artist, "Unknown artist")),
            start: Some(*start),
            end: *end,
            large: Some(large_image(song, *place, *sleep_timer)),
            small: Some(source_image(song)),
        },
        PresenceState::Paused {
            song,
            place,
            sleep_timer,
        } => Activity {
            kind: ActivityKind::Listening,
            details: or(&song.title, "Unknown title"),
            state: Some(format!(
                "Paused \u{b7} {}",
                or(&song.artist, "Unknown artist")
            )),
            start: None,
            end: None,
            large: Some(large_image(song, *place, *sleep_timer)),
            small: Some(image(ASSET_PAUSED, "Paused")),
        },
        PresenceState::Video {
            title,
            start,
            end,
            paused,
        } => Activity {
            kind: ActivityKind::Watching,
            details: or(title, "Unknown video"),
            state: Some(if *paused {
                "Watching a video \u{b7} Paused".to_owned()
            } else {
                "Watching a video".to_owned()
            }),
            start: (!paused).then_some(*start),
            end: if *paused { None } else { *end },
            large: Some(image(ASSET_APP, APP_NAME)),
            small: None,
        },
        PresenceState::Idle { since } => quiet("Idle", Some(*since)),
        PresenceState::Browsing { section, since } => quiet(
            match section {
                Browsing::Library => "Browsing Library",
                Browsing::Search => "Searching",
            },
            Some(*since),
        ),
        PresenceState::Sleeping => Activity {
            state: Some("Sleeping".to_owned()),
            ..quiet("Away", None)
        },
    };
    finish(&mut a);
    a
}

/// A state without a song: the app icon, `Nothing playing`, and an optional start time.
fn quiet(details: &str, since: Option<i64>) -> Activity {
    Activity {
        kind: ActivityKind::Playing,
        details: details.to_owned(),
        state: Some("Nothing playing".to_owned()),
        start: since,
        end: None,
        large: Some(image(ASSET_APP, APP_NAME)),
        small: None,
    }
}

/// The private form: no title, artist, cover, source, time, device, room or sleep timer.
fn private_activity() -> Activity {
    let mut a = Activity {
        kind: ActivityKind::Listening,
        details: "Listening to NemoMusic".to_owned(),
        state: None,
        start: None,
        end: None,
        large: Some(image(ASSET_APP, APP_NAME)),
        small: None,
    };
    finish(&mut a);
    a
}

fn large_image(song: &Song, place: Place, sleep_timer: bool) -> Image {
    let mut facts = Vec::new();
    match place {
        Place::ThisDevice => {}
        Place::Cast => facts.push("Listening on a cast device"),
        Place::Together => facts.push("Listening together"),
    }
    if sleep_timer {
        facts.push("Sleep timer on");
    }
    let tip = if facts.is_empty() {
        APP_NAME.to_owned()
    } else {
        facts.join(" \u{b7} ")
    };
    let key = song
        .cover
        .as_deref()
        .and_then(cover_address)
        .unwrap_or(ASSET_APP);
    image(key, &tip)
}

fn source_image(song: &Song) -> Image {
    let tip = match (&song.codec, song.source) {
        (Some(codec), Source::Local) => format!("{}, {codec}", song.source.name()),
        _ => song.source.name().to_owned(),
    };
    image(song.source.asset(), &tip)
}

/// The cover address when Discord may use it: `https`, on a cover host, 256 bytes at most.
fn cover_address(url: &str) -> Option<&str> {
    (url.len() <= MAX_COVER_BYTES && MediaUrl::parse(url, COVER_HOSTS).is_ok()).then_some(url)
}

/// Fit every text and make the times consistent.
fn finish(a: &mut Activity) {
    a.details = fit(&a.details);
    a.state = a.state.as_deref().map(fit);
    a.start = a.start.filter(|s| *s >= 0);
    a.end = a.end.filter(|e| a.start.is_some_and(|s| *e > s));
}

#[cfg(test)]
mod tests;
