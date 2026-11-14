//! Update, view and subscription of [`App`], and the smaller message steps behind them.

use super::super::views;
use super::art::{CanvasView, CoverView};
use super::job::{link_of, run_job};
use super::message::{CoverBuilt, Job, Message};
use super::model::AccountMsg;
use super::option::CreateMsg;
use super::subscribe::{PlayerKey, map_key, player_events, timer_ticks};
use super::{App, MINI_SIZE, SEARCH_ID};
use crate::cover::{self, Baked};
use crate::daily;
use crate::home;
use crate::keys::Key;
use crate::palette::Palette;
use crate::scheme;
use crate::state::Event;
use crate::theme::{color, layout as theme_layout};
use iced::widget::image;
use iced::{Element, Size, Subscription, Task, keyboard, window};
use nemo_core::catalog::{SearchResults, ShelfItem};
use nemo_core::player::PlayerCommand;
use nemo_core::services::LyricsLookup;
use nemo_core::track::{Track, TrackId};
use std::sync::Arc;
use std::time::Instant;

impl App {
    /// The "Create playlist" sheet.
    pub(super) fn create_msg(&mut self, msg: CreateMsg) -> Vec<Job> {
        match msg {
            CreateMsg::Open => {
                if !self.signed_in() {
                    return self.account_msg(AccountMsg::Open);
                }
                self.create_open = true;
                self.create_name.clear();
                self.ui.text_focused = true;
                Vec::new()
            }
            CreateMsg::Close => {
                self.create_open = false;
                self.ui.text_focused = false;
                Vec::new()
            }
            CreateMsg::Name(n) => {
                self.create_name = n.chars().take(80).collect();
                Vec::new()
            }
            CreateMsg::Confirm => {
                let name = self.create_name.trim().to_owned();
                if name.is_empty() {
                    return Vec::new();
                }
                self.create_open = false;
                self.ui.text_focused = false;
                let text = if let Some(cb) = &self.hooks.on_create_playlist {
                    cb(name.clone());
                    format!("Creating \u{201c}{name}\u{201d}\u{2026}")
                } else {
                    "Creating playlists is not connected yet.".to_owned()
                };
                self.dispatch(Event::Notice(text))
            }
        }
    }

    /// Move a queue entry after the current song one place up or down. Only the entries after
    /// the current song are rewritten, so the song keeps playing and earlier ones stay put.
    pub(super) fn queue_move(&mut self, index: usize, up: bool) -> Vec<Job> {
        let snap = self.ui.snapshot.clone();
        let Some(cur) = snap.queue_index else {
            return Vec::new();
        };
        let first = cur + 1;
        if index < first || index >= snap.queue.len() {
            return Vec::new();
        }
        let target = if up {
            if index == first {
                return Vec::new();
            }
            index - 1
        } else {
            if index + 1 >= snap.queue.len() {
                return Vec::new();
            }
            index + 1
        };
        let mut order: Vec<Track> = snap.queue[first..].to_vec();
        order.swap(index - first, target - first);
        self.queue_swap = (index, target);
        self.queue_anim.snap(1.0);
        self.queue_anim.target = 0.0;
        if self.ui.reduced_motion {
            self.queue_anim.snap(0.0);
        }
        let mut jobs = Vec::new();
        for i in (first..snap.queue.len()).rev() {
            jobs.extend(self.dispatch(Event::Command(PlayerCommand::RemoveAt(i))));
        }
        for t in order {
            jobs.extend(self.dispatch(Event::Command(PlayerCommand::Enqueue(t))));
        }
        jobs
    }

    /// Rebuild the daily sections when their inputs changed.
    pub(super) fn refresh_daily(&mut self) {
        let key = (
            self.daily_gen,
            self.signed_in(),
            self.day.day,
            self.ui.recent.len(),
            self.home.shelves.len(),
        );
        if self.daily_key == Some(key) {
            return;
        }
        self.daily_key = Some(key);
        let pool: Vec<Track> = self
            .home
            .shelves
            .iter()
            .flat_map(|s| s.items.iter())
            .filter_map(|i| match i {
                ShelfItem::Song(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        self.daily_sections = daily::build(
            &self.daily,
            &self.ui.recent,
            &pool,
            self.signed_in(),
            self.day,
        );
    }

    /// Pause when the sleep timer ran out (or the song ended, for "End of song").
    pub(super) fn check_sleep(&mut self) -> Vec<Job> {
        let Some(s) = self.sleep.clone() else {
            return Vec::new();
        };
        let now_id = self.ui.snapshot.current.as_ref().map(|t| t.id.clone());
        let due = match s.deadline {
            Some(d) => Instant::now() >= d,
            None => now_id != s.track,
        };
        if !due {
            return Vec::new();
        }
        self.sleep = None;
        let mut jobs = self.dispatch(Event::Command(PlayerCommand::Pause));
        jobs.extend(self.dispatch(Event::Notice(
            "Sleep timer ended. Playback paused.".to_owned(),
        )));
        jobs
    }

    pub(super) fn learn_local(&mut self, r: &SearchResults) {
        home::merge_albums(&mut self.local_albums, home::local_albums(r));
        home::merge_local_tracks(&mut self.local_tracks, r);
    }

    pub(super) fn on_frame(&mut self, now: Instant) -> Vec<Job> {
        self.frame_stats.record(now);
        let dt = self.last_frame.map_or(16.0, |p| {
            now.saturating_duration_since(p).as_secs_f32() * 1000.0
        });
        self.last_frame = Some(now);
        let mut jobs = Vec::new();
        self.anim.step(dt);
        self.scheme_fade.advance(dt);
        self.bg_fade.advance(dt);
        for (sx, sw) in &mut self.chip_pills {
            sx.step(dt.min(50.0) / 1000.0);
            sw.step(dt.min(50.0) / 1000.0);
        }
        self.queue_anim.step(dt.min(50.0) / 1000.0);
        if self.ui.is_playing() && !self.ui.reduced_motion {
            self.eq_phase = (self.eq_phase + dt.min(100.0) / 170.0) % (std::f32::consts::TAU * 8.0);
        }
        if self.detail_loading() && !self.ui.reduced_motion {
            self.shimmer = (self.shimmer + dt.min(100.0) / 1400.0) % 1.0;
        }
        color::set_active(self.scheme_fade.current());
        if (self.ui.is_buffering() || self.account_busy()) && !self.ui.reduced_motion {
            // One calm turn per 1.2 s.
            self.spin = (self.spin + dt.min(100.0) / 1200.0 * std::f32::consts::TAU)
                % std::f32::consts::TAU;
        }
        if self.ui.is_playing() {
            jobs.extend(self.dispatch(Event::Position(self.backend.player.position())));
        }
        jobs.extend(self.dispatch(Event::Frame { dt_ms: dt }));
        self.sync_anim();
        if !self.needs_frames() {
            // Idle: the next burst of frames must not see this gap as a long frame.
            self.last_frame = None;
        }
        jobs
    }

    /// Show lyrics candidate `i`. `remember` stores it as the user's pick for the song;
    /// otherwise the stored pick is cleared, so the best source (candidate 0) wins again.
    pub(super) fn show_lyrics_candidate(&mut self, i: usize, remember: bool) -> Vec<Job> {
        let Some(track) = self.ui.snapshot.current.clone() else {
            return Vec::new();
        };
        if remember {
            let Some(cand) = self.lyric_candidates.get(i) else {
                return Vec::new();
            };
            self.backend.lyrics.choose(&track.id.0, cand.provider);
        } else {
            self.backend.lyrics.clear_choice(&track.id.0);
        }
        self.lyric_pinned = remember;
        let Some(cand) = self.lyric_candidates.get(i).cloned() else {
            return self.sync_viewport();
        };
        self.lyric_provider = i;
        self.lyric_credit = cand
            .credit
            .clone()
            .or_else(|| self.backend.lyrics.credit(&track.id.0, cand.provider));
        let offset_ms = self.ui.lyrics_offset_ms;
        let mut jobs = self.dispatch(Event::LyricsReady {
            track: track.id,
            lookup: LyricsLookup {
                doc: Some(cand.doc),
                tried: Vec::new(),
            },
            offset_ms,
        });
        jobs.extend(self.sync_viewport());
        jobs
    }

    pub(super) fn on_cover(
        &mut self,
        track: TrackId,
        built: Result<Arc<CoverBuilt>, String>,
    ) -> Vec<Job> {
        let is_current = self.ui.snapshot.current.as_ref().map(|t| &t.id) == Some(&track);
        match built {
            Ok(b) if is_current => {
                let handle =
                    |p: &Baked| image::Handle::from_rgba(p.width, p.height, p.rgba.clone());
                self.prev_cover = self.cover.take();
                self.anim.restart_cover();
                self.cover = Some(CoverView {
                    track: track.clone(),
                    art: handle(&b.art),
                    art_bar: handle(&b.bar),
                    strip: b.data.backdrop_strip,
                    backdrop: image::Handle::from_bytes(b.data.backdrop_png.clone()),
                    tint_alpha: cover::glass_tint_alpha(b.data.backdrop_lightest),
                    scrim_alpha: cover::backdrop_scrim_alpha(b.data.backdrop_lightest),
                    canvas: CanvasView::new(&b.data.canvas),
                });
                self.cover_seed = Some(b.data.seed);
                let palette = self.themed_palette();
                self.dispatch(Event::CoverReady { track, palette })
            }
            Err(_) if is_current => {
                // No cover: neutral placeholder, fallback accent.
                self.cover = None;
                self.prev_cover = None;
                self.cover_seed = None;
                let palette = self.themed_palette();
                self.dispatch(Event::CoverReady { track, palette })
            }
            Ok(_) | Err(_) => Vec::new(),
        }
    }

    /// The accent palette for the current theme settings and cover.
    pub(super) fn themed_palette(&self) -> Palette {
        let s = &self.ui.settings;
        scheme::accent_palette(s.theme, s.base, s.accent_intensity, self.cover_seed)
    }

    pub(super) fn jobs_to_task(&self, jobs: Vec<Job>) -> Task<Message> {
        Task::batch(jobs.into_iter().map(|job| {
            let backend = self.backend.clone();
            Task::perform(run_job(job, backend), |m| m)
        }))
    }

    /// iced `update`.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Tab walks the focusable controls ([`fx::Fx`] draws the focus ring on the one that has it).
        if let Message::Key(Key::Tab, mods) = &message {
            return if mods.shift {
                iced::widget::operation::focus_previous()
            } else {
                iced::widget::operation::focus_next()
            };
        }
        let before = (self.mini, self.fullscreen);
        let copy = match &message {
            Message::CopyLink(t) => t.video_id.as_ref().map(|v| link_of(v.as_str())),
            _ => None,
        };
        let jobs = self.update_core(message);
        let mut tasks = vec![self.jobs_to_task(jobs)];
        if let Some(url) = copy {
            tasks.push(iced::clipboard::write(url));
            tasks.push(Task::done(Message::Ui(Event::Notice(
                "Link copied".to_owned(),
            ))));
        }
        if before != (self.mini, self.fullscreen) {
            tasks.push(self.window_mode_task(before));
        }
        if std::mem::take(&mut self.focus_search) {
            tasks.push(iced::widget::operation::focus(SEARCH_ID));
        }
        Task::batch(tasks)
    }

    /// Resize and re-mode the window after the mini player or full screen was toggled.
    pub(super) fn window_mode_task(&mut self, before: (bool, bool)) -> Task<Message> {
        let (was_mini, was_full) = before;
        let (mini, full) = (self.mini, self.fullscreen);
        if mini && !was_mini {
            self.normal_size = self.window;
        }
        let normal = self.normal_size;
        let min = theme_layout::MIN_WINDOW;
        window::oldest().and_then(move |id| {
            let mut t: Vec<Task<Message>> = Vec::new();
            if full != was_full {
                t.push(window::set_mode(
                    id,
                    if full {
                        window::Mode::Fullscreen
                    } else {
                        window::Mode::Windowed
                    },
                ));
            }
            if mini != was_mini {
                if mini {
                    t.push(window::set_min_size(
                        id,
                        Some(Size::new(MINI_SIZE.0, MINI_SIZE.1)),
                    ));
                    t.push(window::resize(id, Size::new(MINI_SIZE.0, MINI_SIZE.1)));
                    t.push(window::set_level(id, window::Level::AlwaysOnTop));
                } else {
                    t.push(window::set_level(id, window::Level::Normal));
                    t.push(window::set_min_size(id, Some(Size::new(min.0, min.1))));
                    t.push(window::resize(id, normal));
                }
            }
            Task::batch(t)
        })
    }

    /// iced `view`.
    #[must_use]
    pub fn view(&self) -> Element<'_, Message> {
        views::root(self)
    }

    /// iced `subscription`: keys, window size, player events always; frames only while
    /// something moves, so an idle window redraws nothing.
    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            keyboard::listen().filter_map(map_key),
            window::resize_events().map(|(_, size)| Message::Resized(size)),
            Subscription::run_with(PlayerKey(Arc::clone(&self.backend.player)), player_events),
        ];
        if self.sleep.is_some() {
            subs.push(Subscription::run(timer_ticks));
        }
        if self.needs_frames() || self.readout {
            subs.push(window::frames().map(Message::Frame));
        }
        Subscription::batch(subs)
    }

    /// Finish animations and cross-fades (headless runs).
    pub fn finish_animations(&mut self) {
        for _ in 0..240 {
            self.dispatch(Event::Frame { dt_ms: 16.0 });
        }
        self.sync_anim();
        self.bg_fade.finish();
        self.anim.finish();
    }

    /// Run jobs to completion on the calling thread (headless runs and tests).
    pub fn settle_blocking(&mut self, jobs: Vec<Job>) {
        let mut queue = jobs;
        while let Some(job) = queue.pop() {
            let msg =
                iced_test::futures::futures::executor::block_on(run_job(job, self.backend.clone()));
            queue.extend(self.update_core(msg));
        }
    }
}
