//! Keeping the derived state of [`App`] in step: viewport, theme, background, chips, animation.

use super::super::{fx, style, views};
use super::message::Job;
use super::model::{DetailKey, DetailState, NpTab};
use super::{App, ThemeKey};
use crate::explore::{ContentFilter, LibTab};
use crate::layout;
use crate::lyrics_view::metrics::Metrics;
use crate::route::Route;
use crate::scheme::{self, bg_colors};
use crate::state::{Event, LyricsState};
use crate::theme::{color, shape};
use nemo_core::account::AccountState;

impl App {
    pub(super) fn sync_viewport(&mut self) -> Vec<Job> {
        let (w, h) = self.window_px();
        let full = matches!(self.ui.route(), Route::NowPlaying);
        let v = if full {
            views::now_playing::lyrics_viewport_height(w, h)
        } else {
            layout::pane_lyrics_height(w, h)
        };
        let width = self.lyrics_width();
        // Wrapped footer rows take room under the lyrics.
        let v = (v - views::lyrics::footer_extra(self, width)).max(60.0);
        // Window sizes are small numbers, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let metrics = if full {
            Metrics::full(w as f32)
        } else {
            Metrics::pane(layout::right_width(w) as f32)
        };
        let rows = match &self.ui.lyrics {
            LyricsState::Ready(doc) => views::lyrics::line_rows(doc, width, metrics.size),
            _ => Vec::new(),
        };
        let mut jobs = self.dispatch(Event::LyricsMetrics(metrics));
        jobs.extend(self.dispatch(Event::LyricsViewport(v)));
        jobs.extend(self.dispatch(Event::LyricsRows(rows)));
        jobs
    }

    /// `true` while a sign-in or sign-out call runs (here or in the service).
    #[must_use]
    pub fn account_busy(&self) -> bool {
        self.account.busy || matches!(self.backend.account.state(), AccountState::SigningIn)
    }

    /// `true` while the page of the route is still loading (a skeleton is drawn).
    #[must_use]
    pub fn detail_loading(&self) -> bool {
        DetailKey::of(self.ui.route()).is_some_and(|k| {
            matches!(self.details.get(&k), None | Some(DetailState::Loading))
                && (self.ui.youtube_enabled() || k.is_local() || k.is_soundcloud())
        })
    }

    /// `true` while anything animates; the window subscribes to frames only then. The frame
    /// ticker is the window's only frame clock: the backdrop draws on its frames, so it is
    /// listed here and has no timer of its own (backdrop-calm spec).
    #[must_use]
    pub fn needs_frames(&self) -> bool {
        // While music is playing or lyrics are active, frames always tick so progress,
        // lyrics and playback state render smoothly across dual monitors and tabs.
        // When paused, frames only tick while focused and animating.
        if self.ui.needs_frames() {
            return true;
        }
        self.focused
            && (views::backdrop::moves(self)
                || self.anim.active()
                || self.account_busy()
                || self.sc_busy()
                || !self.scheme_fade.is_done()
                || !self.bg_fade.is_done()
                || !self.micro.settled()
                || self
                    .chip_pills
                    .iter()
                    .any(|(x, w)| !x.is_settled() || !w.is_settled())
                || !self.queue_anim.is_settled()
                || !self.setup_anim.is_settled()
                || !self.surfaces.settled()
                || self.dev.moving()
                || self.skeletons.needs_frames(self.ui.reduced_motion))
    }

    /// Point every animation at the state it should show. Cheap; runs after every update.
    /// Apply the theme settings when they changed: corner scale at once, surfaces and accent as
    /// one animated cross-fade.
    pub(super) fn sync_theme(&mut self) {
        let s = &self.ui.settings;
        let song = self.song_seed();
        let key = ThemeKey {
            theme: s.theme,
            base: s.base,
            intensity: s.accent_intensity,
            corner: s.corner_style,
            song,
        };
        if self.theme_key == Some(key) {
            return;
        }
        let first = self.theme_key.is_none();
        self.theme_key = Some(key);
        shape::set_scale_percent(key.corner.percent());
        let reduced = self.ui.reduced_motion || first;
        let palette = scheme::accent_palette(key.theme, key.base, key.intensity, song);
        self.scheme_fade
            .retarget(scheme::window_neutrals(key.base, song), reduced);
        color::set_active(self.scheme_fade.current());
        self.ui.accent.retarget(palette, reduced);
    }

    /// Point the background field at the colour it should have now: the song's seed while the
    /// song colours are on, else the theme's own (song-colour-theme rule table). It follows the
    /// playing song on every screen: an open album or playlist page no longer holds the window
    /// colour (living-backdrop spec).
    pub(super) fn sync_bg(&mut self) {
        let s = &self.ui.settings;
        let seed = scheme::field_seed(s.theme, self.song_seed());
        let colors = bg_colors(seed, s.base);
        let reduced = self.ui.reduced_motion;
        self.bg_fade.retarget(colors, reduced);
    }

    /// The seed of the playing cover while `Match colours to the playing song` is on.
    pub(super) fn song_seed(&self) -> Option<crate::palette::Seed> {
        scheme::song_seed(self.ui.settings.song_colour_theme, self.cover_seed)
    }

    pub(super) fn sync_chips(&mut self) {
        use super::super::views::kit::chip_geometry;
        let lang = self.ui.settings.language;
        let home: Vec<&str> = ContentFilter::ALL.iter().map(|f| f.label_localized(lang)).collect();
        let home_sel = ContentFilter::ALL
            .iter()
            .position(|f| *f == self.content_filter)
            .unwrap_or(0);
        let lib: Vec<&str> = LibTab::ALL.iter().map(|t| t.label_localized(lang)).collect();
        let lib_sel = LibTab::ALL
            .iter()
            .position(|t| *t == self.lib_tab)
            .unwrap_or(0);
        let with_videos = !self.search_is_soundcloud();
        let search = super::super::views::search::filter_labels_localized(with_videos, lang);
        let search_sel =
            super::super::views::search::filter_index(with_videos, self.ui.search.filter);
        // The SoundCloud library tabs (soundcloud-account spec).
        let sc_tabs: Vec<&str> = super::model::ScTab::ALL.iter().map(|t| t.label_localized(lang)).collect();
        let sc_sel = super::model::ScTab::ALL
            .iter()
            .position(|t| *t == self.sc_tab)
            .unwrap_or(0);
        // The listening-area switch of the account sheet and Settings.
        let areas: Vec<&str> = crate::workspace::Source::ALL
            .map(crate::workspace::Source::label)
            .to_vec();
        let area_sel = self.source.index();
        for (slot, (labels, sel)) in [
            (home, home_sel),
            (lib, lib_sel),
            (search, search_sel),
            (sc_tabs, sc_sel),
            (areas, area_sel),
        ]
        .into_iter()
        .enumerate()
        {
            let (x, w) = chip_geometry(&labels, sel);
            let (sx, sw) = &mut self.chip_pills[slot];
            sx.target = x;
            sw.target = w;
            if !self.chip_pills_init || self.ui.reduced_motion {
                sx.snap(x);
                sw.snap(w);
            }
        }
        self.chip_pills_init = true;
    }

    pub(super) fn sync_anim(&mut self) {
        self.sync_chips();
        self.sync_dev();
        self.sync_surfaces();
        // The queue flag and the pane tab stay one fact.
        if self.ui.queue_open && !(self.pane_open && self.np_tab == NpTab::Queue) {
            self.pane_open = true;
            self.np_tab = NpTab::Queue;
        } else if !self.ui.queue_open && self.np_tab == NpTab::Queue {
            self.np_tab = NpTab::Lyrics;
        }
        self.sync_micro();
        self.sync_skeletons();
        let reduced = self.ui.reduced_motion;
        fx::set_reduced_motion(reduced);
        fx::set_focus_color(style::col(self.ui.accent.current().primary));
        let unit = |on: bool| if on { 1.0 } else { 0.0 };
        self.anim.pane.target = unit(self.pane_open);
        self.anim.capsule.target = unit(self.ui.snapshot.current.is_some());
        self.anim.play.target = unit(self.ui.is_playing() && !self.ui.is_buffering());
        self.anim.sheet.target = unit(self.account.open);
        if !self.account.open {
            self.anim.sheet.snap(0.0);
        }
        self.anim.now_playing.target = unit(matches!(self.ui.route(), Route::NowPlaying));
        // After the targets: with reduced motion they are reached in this same update, so not
        // even one frame is asked for.
        self.anim.set_reduced(reduced);
        // The line drawn as active: none while the intro or gap note waits, so the last line
        // cross-fades back to the rest when a long break starts.
        let line = self.ui.lyric_focus().active;
        if line != self.last_line {
            self.prev_line = self.last_line;
            self.last_line = line;
            self.anim.restart_line();
        }
    }

    pub(super) fn set_tab(&mut self, tab: NpTab) {
        self.pane_open = true;
        self.pane_user_set = true;
        self.np_tab = tab;
        self.ui.queue_open = tab == NpTab::Queue;
    }
}
