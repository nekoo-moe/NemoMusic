//! Dispatching reducer events and pumping loads for [`App`].

use super::super::views;
use super::art::ArtEntry;
use super::message::{Job, Message};
use super::model::{DetailKey, DetailState, HomePhase};
use super::{ART_PARALLEL, App, GATE_MESSAGE, ROWS_STEP};
use crate::state::{Effect, Event};
use std::collections::VecDeque;

impl App {
    /// Run an event through the reducer and its effects; returns jobs to start.
    pub fn dispatch(&mut self, event: Event) -> Vec<Job> {
        let mut jobs = Vec::new();
        let mut pending = VecDeque::from([event]);
        while let Some(ev) = pending.pop_front() {
            for fx in self.ui.apply(ev) {
                match fx {
                    Effect::Send(cmd) => {
                        self.backend.player.send(cmd);
                        pending.push_back(Event::Snapshot(self.backend.player.snapshot()));
                        pending.push_back(Event::Position(self.backend.player.position()));
                    }
                    Effect::RefreshSnapshot => {
                        pending.push_back(Event::Snapshot(self.backend.player.snapshot()));
                        pending.push_back(Event::Position(self.backend.player.position()));
                    }
                    Effect::LoadCover(t) => jobs.push(Job::Cover(t)),
                    Effect::LoadLyrics(t) => jobs.push(Job::Lyrics(t)),
                    Effect::SaveLyricsOffset {
                        track_key,
                        offset_ms,
                    } => self.backend.lyrics.set_offset_ms(&track_key, offset_ms),
                    Effect::ToggleFrameReadout => self.readout = !self.readout,
                    Effect::Debounce(seq) => jobs.push(Job::Debounce(seq)),
                    Effect::RunSearch(query) => {
                        if self.ui.youtube_enabled() {
                            jobs.push(Job::Search(query));
                        } else {
                            // Nothing leaves the computer before the notice is accepted.
                            pending.push_back(Event::SearchDone {
                                query,
                                result: Err(GATE_MESSAGE.to_owned()),
                            });
                        }
                    }
                    Effect::FocusSearch => self.focus_search = true,
                    Effect::SettingsChanged(settings) => {
                        if let Some(cb) = &self.on_settings {
                            cb(settings);
                        }
                    }
                    Effect::Resolve { kind, track } => {
                        if self.ui.youtube_enabled()
                            || track.origin == nemo_core::audio::Origin::Local
                        {
                            jobs.push(Job::Resolve { kind, track });
                        }
                    }
                }
            }
        }
        jobs
    }

    /// Start whatever the current state asks for and has not asked for yet: the library listing,
    /// Home shelves (after the notice), the detail page of the route, and pictures in view.
    pub(super) fn after_update(&mut self) -> Vec<Job> {
        let mut jobs = Vec::new();
        if *self.ui.route() != self.last_route {
            self.last_route = self.ui.route().clone();
            self.rows_limit = ROWS_STEP;
            self.anim.restart_page();
            jobs.extend(self.sync_viewport());
        }
        self.sync_theme();
        self.refresh_daily();
        self.sync_bg();
        jobs.extend(self.check_sleep());
        self.sync_anim();
        jobs.extend(self.pump_loads());
        self.probe_art();
        jobs.extend(self.pump_art());
        jobs
    }

    pub(super) fn pump_loads(&mut self) -> Vec<Job> {
        let mut jobs = Vec::new();
        if !self.library_requested {
            self.library_requested = true;
            jobs.push(Job::Library);
        }
        if self.ui.youtube_enabled() && self.daily_requested != Some(self.signed_in()) {
            self.daily_requested = Some(self.signed_in());
            jobs.push(Job::AccountData);
        }
        if self.ui.youtube_enabled() && self.home.phase == HomePhase::Idle {
            self.home.phase = HomePhase::Loading;
            jobs.push(Job::HomeFeed);
        }
        if let Some(key) = DetailKey::of(self.ui.route())
            && !self.details.contains_key(&key)
            && (self.ui.youtube_enabled() || key.is_local())
        {
            self.details.insert(key.clone(), DetailState::Loading);
            jobs.push(Job::Detail(key));
        }
        jobs
    }

    /// Build the view once, only to learn which pictures it asks for.
    pub(super) fn probe_art(&mut self) {
        self.wants.borrow_mut().clear();
        drop(views::root(self));
        let wants = std::mem::take(&mut *self.wants.borrow_mut());
        self.art_queue.clear();
        let mut seen: Vec<String> = Vec::new();
        for spec in wants {
            if let Some(key) = spec.key()
                && !seen.contains(&key)
                && !self.art.contains(&key)
            {
                seen.push(key);
                self.art_queue.push_back(spec);
            }
        }
    }

    pub(super) fn pump_art(&mut self) -> Vec<Job> {
        let mut jobs = Vec::new();
        while self.art_inflight < ART_PARALLEL {
            let Some(spec) = self.art_queue.pop_front() else {
                break;
            };
            let Some(key) = spec.key() else { continue };
            if self.art.contains(&key) {
                continue;
            }
            self.art
                .insert(key, ArtEntry::Loading, |e| matches!(e, ArtEntry::Loading));
            self.art_inflight += 1;
            jobs.push(Job::Art(spec));
        }
        jobs
    }

    /// Handle a message; returns jobs to start. Shared by the window and the headless runner.
    pub fn update_core(&mut self, message: Message) -> Vec<Job> {
        let probe = !matches!(
            message,
            Message::Frame(_) | Message::Art { .. } | Message::SeekDrag(_) | Message::Noop
        );
        let art_arrived = matches!(message, Message::Art { .. });
        let mut jobs = self.handle(message);
        if probe {
            jobs.extend(self.after_update());
        } else if art_arrived {
            jobs.extend(self.pump_art());
        }
        jobs
    }
}
