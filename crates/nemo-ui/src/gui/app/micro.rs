//! Motion of the small details (app-motion spec, task 3): the sliding pill of the Now Playing
//! tabs and of the sidebar, the like heart, the title fade on a track change and the 150 ms
//! fade of an arriving picture. Pure: time is passed in. With reduced motion every value is at
//! its target in the same update, and a settled `Micro` asks for no frames.

use nemo_core::track::{Track, TrackId};

use super::App;
use crate::anim::{EFFECTS_FAST, SPATIAL_FAST};
use crate::motion::{Presence, Spring, emphasized_decelerate};
use crate::theme::motion::SLOW_MS;

/// Fade of an arriving picture, in ms (`design.md` section 9).
pub const ART_FADE_MS: f32 = 150.0;
/// Pictures that may fade at once; later arrivals just appear.
const ART_FADES_MAX: usize = 64;
/// Pictures that arrived a moment ago, each with the time since (the keys of the art cache).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArtFades {
    items: Vec<(String, f32)>,
}

impl ArtFades {
    /// A picture arrived: fade it in (not with reduced motion).
    pub fn start(&mut self, key: String, reduced: bool) {
        if !reduced && self.items.len() < ART_FADES_MAX {
            self.items.push((key, 0.0));
        }
    }

    /// Opacity of the picture with this key: 1 unless it is fading in.
    #[must_use]
    pub fn opacity(&self, key: &str) -> f32 {
        self.items
            .iter()
            .find(|(k, _)| k == key)
            .map_or(1.0, |(_, ms)| emphasized_decelerate(ms / ART_FADE_MS))
    }

    /// Advance by `dt_ms`; finished fades are dropped.
    pub fn advance(&mut self, dt_ms: f32) {
        for (_, ms) in &mut self.items {
            *ms += dt_ms.max(0.0);
        }
        self.items.retain(|(_, ms)| *ms < ART_FADE_MS);
    }

    /// `true` when no picture fades.
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.items.is_empty()
    }
}

/// What the details should show now.
#[derive(Debug, Clone, PartialEq)]
pub struct Targets {
    /// Index of the open Now Playing tab.
    pub tab: usize,
    /// Index into the area's sidebar entries of the one that is current, if any.
    pub nav: Option<usize>,
    /// The playing song is liked.
    pub liked: bool,
    /// The playing song.
    pub track: Option<TrackId>,
}

/// See the module docs.
#[derive(Debug, Clone, PartialEq)]
pub struct Micro {
    /// Position of the tab pill in tab widths (0 to 2).
    pub tab: Spring,
    /// Position of the sidebar pill in entry steps (0 to 3).
    pub nav: Spring,
    /// Opacity of the sidebar pill (it fades when no entry is current).
    pub nav_on: Spring,
    /// Fill of the like heart, 0 outline to 1 filled.
    pub heart: Spring,
    /// The title and artist of the playing song fading in.
    pub title: Presence,
    /// Pictures that are fading in.
    pub art: ArtFades,
    track: Option<TrackId>,
    primed: bool,
}

impl Default for Micro {
    fn default() -> Self {
        let unit = |t: (f32, f32)| Spring::tuned(0.0, t.0, t.1);
        let mut title = Presence::new(SLOW_MS, SLOW_MS);
        title.set(true, true);
        Self {
            tab: unit(SPATIAL_FAST),
            nav: unit(SPATIAL_FAST),
            nav_on: unit(EFFECTS_FAST),
            heart: unit(EFFECTS_FAST),
            title,
            art: ArtFades::default(),
            track: None,
            primed: false,
        }
    }
}

impl Micro {
    fn springs(&mut self) -> [&mut Spring; 4] {
        [
            &mut self.tab,
            &mut self.nav,
            &mut self.nav_on,
            &mut self.heart,
        ]
    }

    /// Point everything at `t`. The first call and a new song snap what must not travel (the
    /// heart of another song, the pill when it comes back from nowhere).
    pub fn sync(&mut self, t: &Targets, reduced: bool) {
        // Indexes and flags are tiny, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let index = |i: usize| i as f32;
        let unit = |on: bool| if on { 1.0 } else { 0.0 };
        self.tab.target = index(t.tab);
        match t.nav {
            Some(i) => {
                let hidden = self.nav_on.target < 0.5 && self.nav_on.is_settled();
                if hidden || !self.primed {
                    self.nav.snap(index(i));
                }
                self.nav.target = index(i);
                self.nav_on.target = 1.0;
            }
            None => self.nav_on.target = 0.0,
        }
        if !self.primed || t.track != self.track {
            self.heart.snap(unit(t.liked));
            if self.primed && t.track.is_some() {
                self.title.restart(reduced);
            }
            self.track.clone_from(&t.track);
        }
        self.heart.target = unit(t.liked);
        if !self.primed {
            for s in self.springs() {
                s.finish();
            }
            self.primed = true;
        }
        if reduced {
            self.finish();
        }
    }

    /// Advance by `dt_ms` milliseconds.
    pub fn advance(&mut self, dt_ms: f32) {
        let dt = dt_ms.clamp(0.0, 50.0);
        for s in self.springs() {
            s.step(dt / 1000.0);
        }
        self.title.advance(dt_ms);
        self.art.advance(dt_ms);
    }

    /// Jump everything to its end.
    pub fn finish(&mut self) {
        for s in self.springs() {
            s.finish();
        }
        self.title.finish();
        self.art = ArtFades::default();
    }

    /// `true` when nothing here moves (no frames are needed for it).
    #[must_use]
    pub fn settled(&self) -> bool {
        self.title.is_settled()
            && self.art.is_settled()
            && [&self.tab, &self.nav, &self.nav_on, &self.heart]
                .iter()
                .all(|s| s.is_settled())
    }

    /// `true` when `id` is the song the heart follows.
    #[must_use]
    pub fn follows(&self, id: &TrackId) -> bool {
        self.track.as_ref() == Some(id)
    }
}

impl App {
    /// Fill of the like heart of `track` (0 outline, 1 filled): the spring for the playing song,
    /// the plain state for any other.
    #[must_use]
    pub fn heart_fill(&self, track: &Track) -> f32 {
        if self.micro.follows(&track.id) {
            self.micro.heart.value.clamp(0.0, 1.0)
        } else if self.is_liked(track) {
            1.0
        } else {
            0.0
        }
    }

    /// Opacity of the title and artist of the playing song (they fade in on a track change).
    #[must_use]
    pub fn title_fade(&self) -> f32 {
        self.micro.title.value()
    }

    /// Point the details at the current state. Runs after every update.
    pub(super) fn sync_micro(&mut self) {
        let nav = self
            .nav_selected()
            .and_then(|e| self.source.nav_index(e));
        let tab = super::NpTab::ALL
            .iter()
            .position(|t| *t == self.np_tab)
            .unwrap_or(0);
        let track = self.ui.snapshot.current.clone();
        let liked = track
            .as_ref()
            .is_some_and(|t| t.video_id.is_some() && self.signed_in() && self.is_liked(t));
        let targets = Targets {
            tab,
            nav,
            liked,
            track: track.map(|t| t.id),
        };
        let reduced = self.ui.reduced_motion;
        self.micro.sync(&targets, reduced);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets(tab: usize, nav: Option<usize>, liked: bool, id: &str) -> Targets {
        Targets {
            tab,
            nav,
            liked,
            track: Some(TrackId(id.to_owned())),
        }
    }

    fn run(d: &mut Micro, ms: f32) {
        let mut left = ms;
        while left > 0.0 {
            d.advance(16.0);
            left -= 16.0;
        }
    }

    #[test]
    fn a_picture_fades_in_over_150_ms_then_is_forgotten() {
        let mut f = ArtFades::default();
        assert!((f.opacity("a") - 1.0).abs() < 1e-6, "unknown key: opaque");
        f.start("a".into(), false);
        assert!(f.opacity("a") < 0.01 && !f.is_settled());
        f.advance(75.0);
        let mid = f.opacity("a");
        assert!(mid > 0.3 && mid < 1.0, "{mid}");
        f.advance(74.0);
        assert!(!f.is_settled(), "149 ms: still fading");
        f.advance(1.0);
        assert!(f.is_settled());
        assert!((f.opacity("a") - 1.0).abs() < 1e-6);
    }

    #[test]
    fn reduced_motion_shows_a_picture_at_once() {
        let mut f = ArtFades::default();
        f.start("a".into(), true);
        assert!(f.is_settled() && (f.opacity("a") - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_tab_pill_slides_to_the_new_tab_and_stops() {
        let mut d = Micro::default();
        d.sync(&targets(0, Some(0), false, "x"), false);
        assert!(d.settled());
        d.sync(&targets(2, Some(0), false, "x"), false);
        assert!((d.tab.target - 2.0).abs() < 1e-6);
        assert!(!d.settled());
        run(&mut d, 400.0);
        assert!(d.settled());
        assert!((d.tab.value - 2.0).abs() < 1e-3);
    }

    #[test]
    fn the_nav_pill_slides_between_entries_and_fades_when_none_is_current() {
        let mut d = Micro::default();
        d.sync(&targets(0, Some(0), false, "x"), false);
        d.sync(&targets(0, Some(3), false, "x"), false);
        assert!((d.nav.target - 3.0).abs() < 1e-6);
        run(&mut d, 400.0);
        assert!((d.nav.value - 3.0).abs() < 1e-3);
        d.sync(&targets(0, None, false, "x"), false);
        run(&mut d, 400.0);
        assert!(d.nav_on.value < 0.01, "faded out in place");
        assert!((d.nav.value - 3.0).abs() < 1e-3, "it does not travel");
        // It comes back where the entry is, not from where it left.
        d.sync(&targets(0, Some(1), false, "x"), false);
        assert!((d.nav.value - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_heart_fills_on_a_like_and_does_not_travel_to_another_song() {
        let mut d = Micro::default();
        d.sync(&targets(0, None, false, "a"), false);
        d.sync(&targets(0, None, true, "a"), false);
        assert!(!d.settled());
        run(&mut d, 20.0);
        assert!(d.heart.value > 0.0 && d.heart.value < 1.0, "morphing");
        run(&mut d, 300.0);
        assert!(d.settled() && (d.heart.value - 1.0).abs() < 1e-3);
        d.sync(&targets(0, None, false, "b"), false);
        assert!(
            d.heart.value.abs() < 1e-6,
            "a new song starts at its own state"
        );
    }

    #[test]
    fn a_new_song_fades_its_title_in_over_400_ms() {
        let mut d = Micro::default();
        d.sync(&targets(0, None, false, "a"), false);
        assert!(
            d.settled() && (d.title.value() - 1.0).abs() < 1e-6,
            "first song: no fade"
        );
        d.sync(&targets(0, None, false, "b"), false);
        assert!(d.title.value() < 0.01 && !d.settled());
        run(&mut d, 380.0);
        assert!(!d.settled());
        run(&mut d, 40.0);
        assert!(d.settled());
    }

    #[test]
    fn reduced_motion_reaches_every_target_in_the_same_update() {
        let mut d = Micro::default();
        d.sync(&targets(0, Some(0), false, "a"), true);
        d.sync(&targets(2, Some(3), true, "b"), true);
        assert!(d.settled());
        assert!((d.tab.value - 2.0).abs() < 1e-6 && (d.nav.value - 3.0).abs() < 1e-6);
        assert!((d.heart.value - 1.0).abs() < 1e-6 && (d.title.value() - 1.0).abs() < 1e-6);
    }
}
