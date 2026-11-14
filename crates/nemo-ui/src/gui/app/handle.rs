//! The message handler of [`App`].

use super::account::shuffle;
use super::art::ArtEntry;
use super::message::{Job, Message};
use super::model::{AccountMsg, DetailData, DetailState, HomePhase, ListSource, NpTab};
use super::option::{CreateMsg, SleepState};
use super::{App, ROWS_STEP};
use crate::explore::MOODS;
use crate::keys::{self};
use crate::route::Route;
use crate::state::{Event, LyricsState};
use crate::{home, layout};
use iced::widget::image;
use nemo_core::track::Artwork;
use std::sync::Arc;
use std::time::Instant;

impl App {
    pub(super) fn handle(&mut self, message: Message) -> Vec<Job> {
        match message {
            Message::Ui(ev) => self.dispatch(ev),
            Message::Player(ev) => self.dispatch(Event::Player(ev)),
            Message::Frame(now) => self.on_frame(now),
            Message::Key(key, mods) => match keys::map(key, mods, self.ui.text_focused) {
                // Esc closes the sign-in sheet first.
                Some(keys::KeyAction::Close) if self.account.open => {
                    self.account_msg(AccountMsg::Close)
                }
                Some(keys::KeyAction::Close) if self.create_open => {
                    self.create_msg(CreateMsg::Close)
                }
                Some(keys::KeyAction::Close) if self.shortcuts_open || self.tools_open => {
                    self.shortcuts_open = false;
                    self.tools_open = false;
                    Vec::new()
                }
                Some(keys::KeyAction::Close) if self.mini => self.handle(Message::ToggleMini),
                Some(keys::KeyAction::Close) if self.fullscreen => {
                    self.handle(Message::ToggleFullScreen)
                }
                Some(keys::KeyAction::ShowShortcuts) => {
                    self.handle(Message::Shortcuts(!self.shortcuts_open))
                }
                Some(keys::KeyAction::FullScreen) => self.handle(Message::ToggleFullScreen),
                Some(keys::KeyAction::Mini) => self.handle(Message::ToggleMini),
                Some(keys::KeyAction::Tools) => self.handle(Message::Tools(!self.tools_open)),
                Some(keys::KeyAction::LikeCurrent) => match self.ui.snapshot.current.clone() {
                    Some(t) => self.handle(Message::ToggleLike(t)),
                    None => Vec::new(),
                },
                Some(action) => self.dispatch(Event::Key(action)),
                None => Vec::new(),
            },
            Message::Resized(size) => {
                self.window = size;
                if !self.pane_user_set {
                    let (w, _) = self.window_px();
                    self.pane_open = layout::right_pushes(w);
                }
                self.sync_viewport()
            }
            Message::TogglePane => {
                self.pane_open = !self.pane_open;
                self.pane_user_set = true;
                if !self.pane_open {
                    self.ui.queue_open = false;
                }
                Vec::new()
            }
            Message::NpTab(tab) => {
                self.set_tab(tab);
                Vec::new()
            }
            Message::HomeFilter(filter) => {
                self.home_filter = filter;
                Vec::new()
            }
            Message::LibFilter(filter) => {
                self.lib_filter = filter;
                Vec::new()
            }
            Message::ShuffleQueue => self.shuffle_queue(),
            Message::OpenNowPlaying => {
                let (w, _) = self.window_px();
                if layout::right_pushes(w) {
                    self.pane_open = !self.pane_open || self.np_tab != NpTab::Lyrics;
                    self.pane_user_set = true;
                    self.np_tab = NpTab::Lyrics;
                    self.ui.queue_open = false;
                    Vec::new()
                } else {
                    self.dispatch(Event::Go(Route::NowPlaying))
                }
            }
            Message::Account(msg) => self.account_msg(msg),
            Message::HomeFeed(result) => match result {
                Ok(shelves) if !shelves.is_empty() => {
                    self.home.shelves.clone_from(shelves.as_ref());
                    self.home.phase = HomePhase::Done;
                    Vec::new()
                }
                // No feed (the real backend): build Home from calm seed searches.
                Ok(_) | Err(_) => vec![Job::HomeSeed(0)],
            },
            Message::HomeSeed { index, result } => {
                if let Ok(r) = result {
                    self.learn_local(&r);
                    let title = home::SEED_SHELVES[index.min(home::SEED_SHELVES.len() - 1)].0;
                    if let Some(shelf) = home::shelf_from_search(title, r.as_ref().clone()) {
                        self.home.shelves.push(shelf);
                    }
                }
                let next = index + 1;
                if next < home::SEED_SHELVES.len() {
                    vec![Job::HomeSeed(next)]
                } else {
                    self.home.phase = HomePhase::Done;
                    Vec::new()
                }
            }
            Message::Library(result) => {
                self.library_loaded = true;
                if let Ok(r) = result {
                    self.learn_local(&r);
                }
                Vec::new()
            }
            Message::SearchDone { query, result } => {
                if let Ok(r) = &result {
                    self.learn_local(r);
                }
                self.dispatch(Event::SearchDone { query, result })
            }
            Message::Detail { key, result } => {
                let state = match result {
                    Ok(d) => DetailState::Ready(d),
                    Err(e) => DetailState::Failed(e),
                };
                let ready = matches!(state, DetailState::Ready(_));
                let mut jobs = Vec::new();
                if let DetailState::Ready(d) = &state
                    && !self.page_seeds.contains_key(&key)
                {
                    let artwork = match d.as_ref() {
                        DetailData::Album(a) => Artwork::Remote(a.summary.thumbnails.clone()),
                        DetailData::Playlist(p) => Artwork::Remote(p.summary.thumbnails.clone()),
                        DetailData::Artist(a) => Artwork::Remote(a.summary.thumbnails.clone()),
                    };
                    jobs.push(Job::PageSeed {
                        key: key.clone(),
                        artwork,
                    });
                }
                self.details.insert(key.clone(), state);
                if self.pending_play.as_ref() == Some(&key) {
                    self.pending_play = None;
                    if ready {
                        let tracks = self.tracks_of(&ListSource::Detail(key));
                        jobs.extend(self.dispatch(Event::PlayTracks { tracks, start: 0 }));
                    }
                }
                jobs
            }
            Message::Art { key, pixels } => {
                self.art_inflight = self.art_inflight.saturating_sub(1);
                let entry = match pixels {
                    Ok(b) => {
                        ArtEntry::Ready(image::Handle::from_rgba(b.width, b.height, b.rgba.clone()))
                    }
                    Err(_) => ArtEntry::Missing,
                };
                self.art
                    .insert(key, entry, |e| matches!(e, ArtEntry::Loading));
                Vec::new()
            }
            Message::Cover { track, built } => self.on_cover(track, built),
            Message::Lyrics {
                track,
                lookup,
                offset_ms,
            } => {
                self.lyric_candidates.clear();
                self.lyric_provider = 0;
                self.lyric_pinned = self.backend.lyrics.chosen(&track.0).is_some();
                self.lyric_credit = lookup
                    .doc
                    .as_ref()
                    .and_then(|d| self.backend.lyrics.credit(&track.0, d.provider));
                let current = self.ui.snapshot.current.clone();
                let mut jobs = self.dispatch(Event::LyricsReady {
                    track: track.clone(),
                    lookup: lookup.as_ref().clone(),
                    offset_ms,
                });
                if let Some(t) = current.filter(|t| t.id == track) {
                    jobs.push(Job::LyricsAll(t));
                }
                jobs.extend(self.sync_viewport());
                jobs
            }
            Message::LyricsCandidates { track, list } => {
                let current = self.ui.snapshot.current.as_ref().map(|t| &t.id);
                if current == Some(&track) {
                    let shown = match &self.ui.lyrics {
                        LyricsState::Ready(d) => Some(d.provider),
                        _ => None,
                    };
                    self.lyric_provider = shown
                        .and_then(|p| list.iter().position(|c| c.provider == p))
                        .unwrap_or(0);
                    self.lyric_candidates.clone_from(list.as_ref());
                }
                self.sync_viewport()
            }
            Message::SeekDrag(f) => {
                self.seek_drag = Some(f);
                Vec::new()
            }
            Message::SeekRelease => match self.seek_drag.take() {
                Some(f) => self.dispatch(Event::SeekFraction(f)),
                None => Vec::new(),
            },
            Message::PlayAt { source, index } => {
                let tracks = self.tracks_of(&source);
                self.dispatch(Event::PlayTracks {
                    tracks,
                    start: index,
                })
            }
            Message::Shuffle(source) => {
                let mut tracks = self.tracks_of(&source);
                shuffle(&mut tracks);
                self.dispatch(Event::PlayTracks { tracks, start: 0 })
            }
            Message::CompactTab(tab) => {
                self.compact_tab = tab;
                Vec::new()
            }
            Message::ShowMore => {
                self.rows_limit += ROWS_STEP;
                Vec::new()
            }
            Message::SettingsSection(s) => {
                self.settings_section = s;
                Vec::new()
            }
            Message::SettingsSectionGo(s) => {
                self.settings_section = s;
                self.dispatch(Event::Go(Route::Settings))
            }
            Message::RetryDetail(key) => {
                self.details.remove(&key);
                Vec::new()
            }
            Message::ToggleLike(track) => {
                self.ui.menu = None;
                if !self.signed_in() {
                    return self.account_msg(AccountMsg::Open);
                }
                if track.video_id.is_none() {
                    return Vec::new();
                }
                let liked = !self.is_liked(&track);
                self.likes.insert(track.id.clone(), liked);
                vec![Job::Rate { track, liked }]
            }
            Message::LikeDone {
                track,
                liked,
                result,
            } => {
                if result.is_err() {
                    self.likes.insert(track, !liked);
                    return self.dispatch(Event::Notice(
                        "Could not update your like. Try again in a moment.".to_owned(),
                    ));
                }
                Vec::new()
            }
            Message::AddFolder => {
                if let Some(cb) = &self.on_add_music_folder {
                    cb();
                }
                Vec::new()
            }
            Message::ContentFilter(f) => {
                self.content_filter = f;
                Vec::new()
            }
            Message::LibTab(tab) => {
                self.lib_tab = tab;
                self.rows_limit = ROWS_STEP;
                Vec::new()
            }
            Message::LibSort(s) => {
                self.lib_sort = s;
                Vec::new()
            }
            Message::LibLayout(l) => {
                self.lib_layout = l;
                Vec::new()
            }
            Message::PlayCollection(key) => {
                if let Some(DetailState::Ready(_)) = self.details.get(&key) {
                    let tracks = self.tracks_of(&ListSource::Detail(key));
                    return self.dispatch(Event::PlayTracks { tracks, start: 0 });
                }
                self.pending_play = Some(key.clone());
                if self.details.contains_key(&key) {
                    return Vec::new();
                }
                self.details.insert(key.clone(), DetailState::Loading);
                vec![Job::Detail(key)]
            }
            Message::PickMood(i) => {
                let mood = MOODS[i.min(MOODS.len() - 1)];
                self.dispatch(Event::Go(Route::Search(mood.query.to_owned())))
            }
            Message::ExploreLoaded(shelves) => {
                self.explore_shelves.clone_from(shelves.as_ref());
                Vec::new()
            }
            Message::AccountLibrary(lib) => {
                self.account_library = Some(lib);
                Vec::new()
            }
            Message::Tools(on) => {
                self.tools_open = on;
                self.shortcuts_open = false;
                Vec::new()
            }
            Message::Shortcuts(on) => {
                self.shortcuts_open = on;
                self.tools_open = false;
                Vec::new()
            }
            Message::Create(msg) => self.create_msg(msg),
            Message::Sleep(choice) => {
                self.sleep = choice.map(|choice| SleepState {
                    choice,
                    deadline: choice.duration().map(|d| Instant::now() + d),
                    track: self.ui.snapshot.current.as_ref().map(|t| t.id.clone()),
                });
                let text = match choice {
                    Some(c) => format!("Sleep timer set: {}", c.label()),
                    None => "Sleep timer off".to_owned(),
                };
                self.dispatch(Event::Notice(text))
            }
            Message::Speed(p) => {
                if let Some(cb) = &self.hooks.on_set_speed {
                    self.speed_percent = p;
                    cb(p);
                }
                Vec::new()
            }
            Message::ToggleMini => {
                self.mini = !self.mini;
                self.fullscreen = false;
                self.tools_open = false;
                Vec::new()
            }
            Message::ToggleFullScreen => {
                self.fullscreen = !self.fullscreen;
                self.mini = false;
                self.tools_open = false;
                if self.fullscreen {
                    self.dispatch(Event::Go(Route::NowPlaying))
                } else {
                    self.dispatch(Event::Back)
                }
            }
            Message::StartRadio(track) => {
                self.ui.menu = None;
                if track.origin == nemo_core::audio::Origin::YoutubeMusic
                    && !self.ui.youtube_enabled()
                {
                    return Vec::new();
                }
                vec![Job::Radio(track)]
            }
            Message::RadioReady { seed, tracks } => {
                let mut queue = vec![seed.clone()];
                queue.extend(tracks.into_iter().filter(|t| t.id != seed.id).take(24));
                let n = queue.len();
                let mut jobs = self.dispatch(Event::PlayTracks {
                    tracks: queue,
                    start: 0,
                });
                jobs.extend(self.dispatch(Event::Notice(format!(
                    "Radio from \u{201c}{}\u{201d}: {n} songs from search results",
                    seed.title
                ))));
                jobs
            }
            Message::CopyLink(_) => {
                // The clipboard write is a task; `update` runs it.
                self.ui.menu = None;
                Vec::new()
            }
            Message::QueueMove { index, up } => self.queue_move(index, up),
            Message::PickLyricsProvider(i) => self.show_lyrics_candidate(i, true),
            Message::LyricsAuto => self.show_lyrics_candidate(0, false),
            Message::SetLyricsSource(id, on) => {
                self.backend.lyrics.set_provider_enabled(id, on);
                self.lyric_sources = self.backend.lyrics.providers();
                Vec::new()
            }
            Message::PageSeed { key, seed } => {
                self.page_seeds.insert(key, seed);
                Vec::new()
            }
            Message::AccountData(b) => {
                self.daily = b.daily.clone();
                self.daily_gen += 1;
                let lib = &b.library;
                let any = !(lib.playlists.is_empty()
                    && lib.albums.is_empty()
                    && lib.artists.is_empty()
                    && lib.songs.is_empty());
                if self.signed_in() && any {
                    self.account_library = Some(Arc::new(b.library.clone()));
                }
                Vec::new()
            }
            Message::SearchFor(q) => self.dispatch(Event::Go(Route::Search(q))),
            Message::TimerTick | Message::Noop => Vec::new(),
        }
    }
}
