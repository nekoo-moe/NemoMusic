//! The message handler of [`App`].

use super::account::shuffle;
use super::art::ArtEntry;
use super::message::{Job, Message};
use super::model::{
    AccountMsg, DetailData, DetailKey, DetailState, HomePhase, ListSource, NpTab,
};
use super::option::{CreateMsg, SleepState};
use super::{App, ROWS_STEP};
use crate::collection_sort::CollectionSort;
use crate::explore::MOODS;
use crate::keys::{self};
use crate::route::Route;
use crate::state::{Event, LyricsState};
use crate::{home, layout};
use iced::widget::image;
use nemo_core::track::Artwork;
use std::time::Instant;

impl App {
    pub(super) fn handle(&mut self, message: Message) -> Vec<Job> {
        match message {
            Message::Ui(ev) => self.dispatch(ev),
            Message::Player(ev) => self.dispatch(Event::Player(ev)),
            Message::Frame(now) => self.on_frame(now),
            // The setup flow takes the keys: Enter is Next, Esc and the rest do nothing.
            Message::Key(key, _) if self.ui.setup.active() && !self.account.open => {
                self.setup_key(key)
            }
            Message::Key(key, mods) => match keys::map(key, mods, self.ui.text_focused) {
                // Esc closes the sign-in sheet first.
                Some(keys::KeyAction::Close) if self.account.open => {
                    self.account_msg(AccountMsg::Close)
                }
                Some(keys::KeyAction::Close) if self.sort_menu.is_some() => {
                    self.sort_menu = None;
                    Vec::new()
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
                Some(keys::KeyAction::Refresh) => self.refresh_msg(),
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
            Message::Rescaled(factor) => self.rescale(factor),
            Message::Focus(on) => {
                self.focused = on;
                // The first frame after a pause must not see the pause as one long frame.
                self.last_frame = None;
                Vec::new()
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
            Message::Sc(msg) => self.sc_msg(msg),
            // A reload replaced the shelves already: a late start-up answer is older.
            Message::HomeFeed(_) | Message::HomeSeed { .. }
                if self.home.phase == HomePhase::Done =>
            {
                Vec::new()
            }
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
            Message::SearchDone {
                source,
                query,
                result,
            } => {
                if let Ok(r) = &result {
                    self.learn_local(r);
                }
                if source != self.source {
                    // The area was switched while it ran: the answer goes to the area that asked.
                    self.rest.search.done(&query, result);
                    return Vec::new();
                }
                self.dispatch(Event::SearchDone { query, result })
            }
            Message::SearchKindDone {
                source,
                query,
                kind,
                result,
            } => {
                if source != self.source {
                    self.rest.search.filter_done(&query, kind, result);
                    return Vec::new();
                }
                self.dispatch(Event::SearchFilterDone {
                    query,
                    kind,
                    result,
                })
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
                    let shuffled = std::mem::take(&mut self.pending_shuffle);
                    if ready {
                        jobs.extend(self.play_loaded(key, shuffled));
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
                if matches!(entry, ArtEntry::Ready(_)) {
                    self.micro.art.start(key.clone(), self.ui.reduced_motion);
                }
                self.art
                    .insert(key, entry, |e| matches!(e, ArtEntry::Loading));
                Vec::new()
            }
            Message::Cover {
                track,
                scale,
                built,
            } => self.on_cover(track, scale, built),
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
            Message::HoverTarget(_) | Message::HoverDue(_) | Message::PointerDown => {
                self.handle_hint(message)
            }
            Message::Shuffle(source) => {
                let mut tracks = self.tracks_of(&source);
                shuffle(&mut tracks);
                self.dispatch(Event::PlayTracks { tracks, start: 0 })
            }
            Message::SortMenu(key) => {
                self.sort_menu = key;
                Vec::new()
            }
            Message::SortCollection { key, sort } => self.sort_collection(&key, sort),
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
            Message::ScWrite(msg) => self.sc_write_msg(msg),
            Message::ToggleLike(track) => {
                self.ui.menu = None;
                if track.origin == nemo_core::audio::Origin::SoundCloud {
                    return self.sc_like(track);
                }
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
            Message::AddFolder => self
                .on_add_music_folder
                .iter()
                .map(|picker| Job::PickFolder(picker.clone()))
                .collect(),
            Message::FolderPicked(picked) => {
                // A cancelled dialog changes nothing; a folder already listed is not listed twice.
                if let Some(folder) = picked
                    && !self.music_folders.contains(&folder)
                {
                    self.music_folders.push(folder);
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
            Message::PlayCollection(key) => self.play_collection(key, false),
            Message::ShuffleCollection(key) => self.play_collection(key, true),
            Message::PickMood(i) => {
                let mood = MOODS[i.min(MOODS.len() - 1)];
                self.dispatch(Event::Go(Route::Search(mood.query.to_owned())))
            }
            Message::Explore(msg) => self.explore_msg(msg),
            Message::ExploreLoaded(shelves) => {
                self.explore_shelves.clone_from(shelves.as_ref());
                Vec::new()
            }
            Message::AccountLibrary(lib) => {
                self.account_library = Some(lib);
                Vec::new()
            }
            Message::ToggleMute => self.toggle_mute(),
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
            // The start-up bundle; once a reload ran, its answer is newer than this one.
            Message::AccountData(_) if self.refresh.generation() > 0 => Vec::new(),
            Message::AccountData(b) => {
                self.daily = b.daily.clone();
                self.explore.fill_daily(&mut self.daily);
                self.daily_gen += 1;
                if self.signed_in() {
                    self.set_account_library(&b);
                }
                Vec::new()
            }
            Message::SearchFor(q) => self.dispatch(Event::Go(Route::Search(q))),
            Message::Refresh => self.refresh_msg(),
            Message::Refreshed { generation, data } => self.on_refreshed(generation, data),
            // `Restart` is answered in `App::update`, with a task that closes the window.
            Message::TimerTick | Message::Noop | Message::Restart => Vec::new(),
        }
    }

    /// Play a page's songs, in order or shuffled; load the page first when needed.
    fn play_collection(&mut self, key: DetailKey, shuffled: bool) -> Vec<Job> {
        if let Some(DetailState::Ready(_)) = self.details.get(&key) {
            return self.play_loaded(key, shuffled);
        }
        self.pending_play = Some(key.clone());
        self.pending_shuffle = shuffled;
        if self.details.contains_key(&key) {
            return Vec::new();
        }
        self.details.insert(key.clone(), DetailState::Loading);
        vec![Job::Detail(key)]
    }

    /// Show a page in `sort` order, remember it in the settings and save them. The queue that
    /// plays now is left alone; the next Play or Shuffle uses the new order.
    fn sort_collection(&mut self, key: &DetailKey, sort: CollectionSort) -> Vec<Job> {
        self.sort_menu = None;
        let Some(page) = Self::sort_key(key) else {
            return Vec::new();
        };
        crate::collection_sort::remember(&mut self.ui.settings.collection_sorts, &page, sort);
        if let Some(cb) = &self.on_settings {
            cb(self.ui.settings.clone());
        }
        Vec::new()
    }

    /// Play the songs of a page that has loaded.
    fn play_loaded(&mut self, key: DetailKey, shuffled: bool) -> Vec<Job> {
        let mut tracks = self.tracks_of(&ListSource::Detail(key));
        if shuffled {
            shuffle(&mut tracks);
        }
        self.dispatch(Event::PlayTracks { tracks, start: 0 })
    }
}
