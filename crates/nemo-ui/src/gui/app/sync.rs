//! Keeping the derived state of [`App`] in step: viewport, theme, background, chips, animation.

use super::super::{fx, style, views};
use super::App;
use super::message::Job;
use super::model::{DetailKey, DetailState, NpTab};
use crate::explore::{ContentFilter, LibTab};
use crate::layout;
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
        // The credit line and wrapped footer rows take room under the lyrics.
        let v = (v - views::lyrics::footer_extra(self, width)).max(60.0);
        let rows = match &self.ui.lyrics {
            LyricsState::Ready(doc) => views::lyrics::line_rows(doc, width, !full),
            _ => Vec::new(),
        };
        let mut jobs = self.dispatch(Event::LyricsDense(!full));
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
                && (self.ui.youtube_enabled() || k.is_local())
        })
    }

    /// `true` while anything animates; the window subscribes to frames only then.
    #[must_use]
    pub fn needs_frames(&self) -> bool {
        self.ui.needs_frames()
            || self.anim.active()
            || self.account_busy()
            || !self.scheme_fade.is_done()
            || !self.bg_fade.is_done()
            || self.detail_loading()
            || self
                .chip_pills
                .iter()
                .any(|(x, w)| !x.is_settled() || !w.is_settled())
            || !self.queue_anim.is_settled()
    }

    /// Point every animation at the state it should show. Cheap; runs after every update.
    /// Apply the theme settings when they changed: corner scale at once, surfaces and accent as
    /// one animated cross-fade.
    pub(super) fn sync_theme(&mut self) {
        let s = &self.ui.settings;
        let key = (s.theme, s.base, s.accent_intensity, s.corner_style);
        if self.theme_key == Some(key) {
            return;
        }
        let first = self.theme_key.is_none();
        self.theme_key = Some(key);
        let (theme, base, intensity, corner) = key;
        shape::set_scale_percent(corner.percent());
        let reduced = self.ui.reduced_motion || first;
        self.scheme_fade.retarget(base.neutrals(), reduced);
        color::set_active(self.scheme_fade.current());
        let palette = scheme::accent_palette(theme, base, intensity, self.cover_seed);
        self.ui.accent.retarget(palette, reduced);
    }

    /// Point the background field at the colour it should have now: a fixed theme keeps its own
    /// seed; Dynamic uses the page being viewed (album or playlist) and otherwise the song.
    pub(super) fn sync_bg(&mut self) {
        let s = &self.ui.settings;
        let seed = s.theme.seed().or_else(|| {
            DetailKey::of(self.ui.route())
                .and_then(|k| self.page_seeds.get(&k).copied())
                .or(self.cover_seed)
        });
        let colors = bg_colors(seed, s.base);
        let reduced = self.ui.reduced_motion;
        self.bg_fade.retarget(colors, reduced);
    }

    pub(super) fn sync_chips(&mut self) {
        use super::super::views::kit::chip_geometry;
        let home: Vec<&str> = ContentFilter::ALL.iter().map(|f| f.label()).collect();
        let home_sel = ContentFilter::ALL
            .iter()
            .position(|f| *f == self.content_filter)
            .unwrap_or(0);
        let lib: Vec<&str> = LibTab::ALL.iter().map(|t| t.label()).collect();
        let lib_sel = LibTab::ALL
            .iter()
            .position(|t| *t == self.lib_tab)
            .unwrap_or(0);
        for (slot, (labels, sel)) in [(home, home_sel), (lib, lib_sel)].into_iter().enumerate() {
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
        // The queue flag and the pane tab stay one fact.
        if self.ui.queue_open && !(self.pane_open && self.np_tab == NpTab::Queue) {
            self.pane_open = true;
            self.np_tab = NpTab::Queue;
        } else if !self.ui.queue_open && self.np_tab == NpTab::Queue {
            self.np_tab = NpTab::Lyrics;
        }
        let reduced = self.ui.reduced_motion;
        fx::set_reduced_motion(reduced);
        fx::set_focus_color(style::col(self.ui.accent.current().primary));
        self.anim.set_reduced(reduced);
        let unit = |on: bool| if on { 1.0 } else { 0.0 };
        self.anim.pane.target = unit(self.pane_open);
        self.anim.capsule.target = unit(self.ui.snapshot.current.is_some());
        self.anim.play.target = unit(self.ui.is_playing() && !self.ui.is_buffering());
        self.anim.sheet.target = unit(self.account.open);
        if !self.account.open {
            self.anim.sheet.snap(0.0);
        }
        let line = self.ui.active_lyric();
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
