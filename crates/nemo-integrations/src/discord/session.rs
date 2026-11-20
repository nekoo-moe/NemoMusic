//! Folds play events and window facts into the [`PresenceState`] to show.
//!
//! Pure: the wall time is a parameter. The start time of a song is `wall - position`, fixed when
//! the event arrives, so an update that waits for its slot still shows the right progress.

use std::path::Path;
use std::time::Duration;

use nemo_core::audio::Origin;
use nemo_core::hooks::PlayHookEvent;
use nemo_core::player::PlayState;
use nemo_core::track::{Artwork, Track, pick_thumbnail};

use super::state::{Browsing, Place, PresenceState, Song, Source};

/// Width in pixels the cover is picked for: the large picture of the Discord card.
const COVER_PX: u32 = 512;

/// The song that plays, as far as Discord may show it. Holds no file path.
#[derive(Debug, Clone)]
struct Current {
    song: Song,
    duration: Option<Duration>,
}

/// What the app knows about playback and the window.
#[derive(Debug, Clone)]
pub struct Session {
    current: Option<Current>,
    state: PlayState,
    /// Position of the song at `anchor`.
    position: Duration,
    /// Wall time (Unix seconds) at which the song was at `position`.
    anchor: i64,
    /// Since when nothing plays (Unix seconds).
    idle_since: i64,
    place: Place,
    sleep_timer: bool,
    section: Option<Browsing>,
}

impl Session {
    /// A session of an app that was just opened, at wall time `wall` (Unix seconds).
    #[must_use]
    pub const fn new(wall: i64) -> Self {
        Self {
            current: None,
            state: PlayState::Stopped,
            position: Duration::ZERO,
            anchor: wall,
            idle_since: wall,
            place: Place::ThisDevice,
            sleep_timer: false,
            section: None,
        }
    }

    /// Take one play event, which arrived at wall time `wall`.
    pub fn on_play(&mut self, event: &PlayHookEvent, wall: i64) {
        match event {
            PlayHookEvent::TrackStarted { track, .. } => {
                self.current = Some(Current {
                    song: song_of(track),
                    duration: track.duration,
                });
                self.position = Duration::ZERO;
                self.anchor = wall;
            }
            PlayHookEvent::StateChanged { state, position } => {
                self.position = *position;
                self.anchor = wall;
                match state {
                    PlayState::Buffering => {}
                    PlayState::Stopped => {
                        self.state = PlayState::Stopped;
                        self.current = None;
                        self.idle_since = wall;
                    }
                    other => self.state = *other,
                }
            }
            PlayHookEvent::PositionJumped { position } => {
                self.position = *position;
                self.anchor = wall;
            }
            PlayHookEvent::DurationKnown(length) => {
                if let Some(current) = &mut self.current {
                    current.duration = Some(*length);
                }
            }
            PlayHookEvent::TrackEnded(_) => {}
        }
    }

    /// Where the sound comes out.
    pub const fn set_place(&mut self, place: Place) {
        self.place = place;
    }

    /// A sleep timer runs or not.
    pub const fn set_sleep_timer(&mut self, on: bool) {
        self.sleep_timer = on;
    }

    /// The part the user browses.
    pub const fn set_section(&mut self, section: Option<Browsing>) {
        self.section = section;
    }

    /// Whether a song plays now (not paused, not stopped).
    #[must_use]
    pub fn playing(&self) -> bool {
        self.current.is_some() && self.state == PlayState::Playing
    }

    /// What to show. `asleep` is the verdict of the sleeping rule.
    #[must_use]
    pub fn state(&self, asleep: bool) -> PresenceState {
        if asleep {
            return PresenceState::Sleeping;
        }
        match (&self.current, self.state) {
            (Some(current), PlayState::Playing) => {
                let start = self.anchor.saturating_sub(secs(self.position));
                PresenceState::Listening {
                    song: current.song.clone(),
                    start,
                    end: current.duration.map(|d| start.saturating_add(secs(d))),
                    place: self.place,
                    sleep_timer: self.sleep_timer,
                }
            }
            (Some(current), PlayState::Paused) => PresenceState::Paused {
                song: current.song.clone(),
                place: self.place,
                sleep_timer: self.sleep_timer,
            },
            _ => match self.section {
                Some(section) => PresenceState::Browsing {
                    section,
                    since: self.idle_since,
                },
                None => PresenceState::Idle {
                    since: self.idle_since,
                },
            },
        }
    }
}

fn secs(d: Duration) -> i64 {
    i64::try_from(d.as_secs()).unwrap_or(i64::MAX)
}

/// The facts of `track` that Discord may show. A local file's path is dropped; only a lossless
/// codec named by its extension is kept.
fn song_of(track: &Track) -> Song {
    let source = match track.origin {
        Origin::Local => Source::Local,
        Origin::YoutubeMusic => Source::YouTubeMusic,
        Origin::SoundCloud => Source::SoundCloud,
    };
    let cover = match &track.artwork {
        Artwork::Remote(sizes) => pick_thumbnail(sizes, COVER_PX).map(|t| t.url.clone()),
        Artwork::None | Artwork::LocalFile(_) => None,
    };
    let codec = (source == Source::Local)
        .then(|| track.local_path.as_deref().and_then(lossless_codec))
        .flatten();
    Song {
        title: track.title.clone(),
        artist: track.artists.join(", "),
        source,
        cover,
        codec,
    }
}

/// The name of a lossless codec, from the file extension only.
fn lossless_codec(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let name = match ext.as_str() {
        "flac" => "FLAC",
        "wav" => "WAV",
        "aiff" | "aif" => "AIFF",
        "ape" => "APE",
        "wv" => "WavPack",
        _ => return None,
    };
    Some(name.to_owned())
}

#[cfg(test)]
mod tests;
