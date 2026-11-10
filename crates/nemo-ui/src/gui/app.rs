//! The iced application: state, messages, update, subscriptions. The pure reducer in
//! [`crate::state`] decides what happens; this file runs its effects against the [`Backend`].

use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use iced::widget::image;
use iced::{Element, Size, Subscription, Task, keyboard, stream, window};
use nemo_core::catalog::{AlbumSummary, ArtistSummary, PlaylistSummary};
use nemo_core::lyrics::LyricsQuery;
use nemo_core::player::{PlayerCommand, PlayerEvent};
use nemo_core::services::{Backend, LyricsLookup, PlayerControl};
use nemo_core::track::{Artwork, Track, TrackId};

use super::style;
use super::views;
use crate::cover::{self, CoverData};
use crate::frame_stats::FrameStats;
use crate::keys::{self, Key, Mods};
use crate::layout;
use crate::palette::Palette;
use crate::route::Route;
use crate::state::{Effect, Event, UiState};
use crate::theme::layout as theme_layout;

/// Options for [`run`](super::run).
#[derive(Debug, Clone)]
pub struct UiOptions {
    /// Honour the OS "reduce motion" setting: no cross-fade, no lyric glide.
    pub reduced_motion: bool,
    /// Window size at start, logical px.
    pub window_size: (f32, f32),
    /// Show the frame-time readout at start (F3 toggles it).
    pub show_frame_readout: bool,
    /// First screen.
    pub start_route: Route,
}

impl Default for UiOptions {
    fn default() -> Self {
        Self {
            reduced_motion: false,
            window_size: theme_layout::DEFAULT_WINDOW,
            show_frame_readout: false,
            start_route: Route::Home,
        }
    }
}

/// State of one remote image.
#[derive(Debug, Clone)]
pub enum ArtState {
    /// Requested, not yet here.
    Loading,
    /// Decoded handles: corners cut for cards, and a circle for artists.
    Ready {
        /// Rounded square (radius `shape.md`).
        square: image::Handle,
        /// Circle.
        circle: image::Handle,
    },
    /// There is no art (or it failed): draw the placeholder.
    Missing,
}

/// The current track's cover, analysed.
#[derive(Debug, Clone)]
pub struct CoverView {
    /// Track this cover belongs to.
    pub track: TrackId,
    /// Full cover with `shape.lg` corners (hero and Now Playing).
    pub art: image::Handle,
    /// Cover with `shape.md` corners for the 64 px slot in the bar.
    pub art_bar: image::Handle,
    /// Colours sampled left, middle, right from the blurred backdrop.
    pub strip: [crate::theme::Rgba; 3],
    /// Pre-blurred backdrop texture.
    pub backdrop: image::Handle,
    /// Glass tint alpha, or `None` for the tonal fallback.
    pub tint_alpha: Option<f32>,
    /// Scrim alpha over the Now Playing backdrop.
    pub scrim_alpha: f32,
}

/// What Home shows, from `catalog.search("")`.
#[derive(Debug, Clone, Default)]
pub struct HomeData {
    /// Albums shelf.
    pub albums: Vec<AlbumSummary>,
    /// Artists shelf.
    pub artists: Vec<ArtistSummary>,
    /// Playlists shelf.
    pub playlists: Vec<PlaylistSummary>,
    /// Songs shelf.
    pub songs: Vec<Track>,
}

/// Lower area of compact Now Playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompactTab {
    /// Three lyric lines.
    #[default]
    Lyrics,
    /// The queue.
    Queue,
}

/// Messages of the application.
#[derive(Debug, Clone)]
pub enum Message {
    /// A reducer event from a widget.
    Ui(Event),
    /// A frame was drawn.
    Frame(Instant),
    /// A key press.
    Key(Key, Mods),
    /// The window changed size.
    Resized(Size),
    /// Player event from the subscription.
    Player(PlayerEvent),
    /// Home data arrived.
    HomeLoaded(Result<Arc<HomeData>, String>),
    /// An image arrived.
    Art {
        /// Cache key.
        key: String,
        /// PNG/JPEG/WebP bytes.
        bytes: Result<Arc<ArtBytes>, String>,
    },
    /// A cover was analysed.
    Cover {
        /// Track.
        track: TrackId,
        /// Result.
        built: Result<Arc<CoverBuilt>, String>,
    },
    /// Lyrics arrived.
    Lyrics {
        /// Track.
        track: TrackId,
        /// Lookup answer.
        lookup: Arc<LyricsLookup>,
        /// Stored offset.
        offset_ms: i64,
    },
    /// The seek slider moved (0 to 1).
    SeekDrag(f32),
    /// The seek slider was released.
    SeekRelease,
    /// Play song `index` of the Home songs shelf.
    PlayShelfSong(usize),
    /// Switch the compact Now Playing tab.
    CompactTab(CompactTab),
    /// Nothing.
    Noop,
}

/// An image with its corners baked in, built off the UI thread.
#[derive(Debug)]
pub struct ArtBytes {
    /// PNG with `shape.md` corners.
    pub square: Vec<u8>,
    /// PNG as a circle.
    pub circle: Vec<u8>,
}

/// A cover with its analysis, built off the UI thread.
#[derive(Debug)]
pub struct CoverBuilt {
    /// PNG with `shape.lg` corners.
    pub bytes: Vec<u8>,
    /// PNG with `shape.md` corners for the bar slot.
    pub bar: Vec<u8>,
    /// Palette and backdrop.
    pub data: CoverData,
}

/// Work that needs the backend; run as a task (or blocking, for snapshots).
#[derive(Debug, Clone)]
pub enum Job {
    /// Load the Home data.
    LoadHome,
    /// Load one image.
    Art {
        /// Cache key.
        key: String,
        /// Where it comes from.
        artwork: Artwork,
        /// Slot size in px.
        px: u32,
    },
    /// Load and analyse a cover.
    Cover(Track),
    /// Look up lyrics.
    Lyrics(Track),
}

/// Cache key of an artwork (first thumbnail address or file path).
#[must_use]
pub fn art_key(artwork: &Artwork) -> Option<String> {
    match artwork {
        Artwork::None => None,
        Artwork::Remote(t) => t.first().map(|t| t.url.clone()),
        Artwork::LocalFile(p) => Some(p.display().to_string()),
    }
}

/// Run a job to completion.
pub async fn run_job(job: Job, backend: Backend) -> Message {
    match job {
        Job::LoadHome => {
            let result = backend
                .catalog
                .search("")
                .await
                .map(|r| {
                    Arc::new(HomeData {
                        albums: r.albums,
                        artists: r.artists,
                        playlists: r.playlists,
                        songs: r.songs,
                    })
                })
                .map_err(|e| e.to_string());
            Message::HomeLoaded(result)
        }
        Job::Art { key, artwork, px } => {
            let bytes = match backend.images.image(&artwork, px).await {
                Ok(raw) => cover::round_corners(&raw, 12.0 / 184.0)
                    .and_then(|square| {
                        cover::round_corners(&raw, 0.5).map(|circle| ArtBytes { square, circle })
                    })
                    .map(Arc::new)
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e.to_string()),
            };
            Message::Art { key, bytes }
        }
        Job::Cover(track) => {
            let built = match backend.images.image(&track.artwork, 640).await {
                Ok(raw) => cover::analyze(&raw)
                    .and_then(|data| {
                        let bytes = cover::round_corners(&raw, 16.0 / 480.0)?;
                        let bar = cover::round_corners(&raw, 12.0 / 64.0)?;
                        Ok(Arc::new(CoverBuilt { bytes, bar, data }))
                    })
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e.to_string()),
            };
            Message::Cover {
                track: track.id,
                built,
            }
        }
        Job::Lyrics(track) => {
            let lookup = backend
                .lyrics
                .lookup(&LyricsQuery::from_track(&track))
                .await;
            let offset_ms = backend.lyrics.offset_ms(&track.id.0);
            Message::Lyrics {
                track: track.id,
                lookup: Arc::new(lookup),
                offset_ms,
            }
        }
    }
}

/// The application state.
pub struct App {
    pub(crate) backend: Backend,
    pub(crate) ui: UiState,
    pub(crate) window: Size,
    pub(crate) home: Option<Result<Arc<HomeData>, String>>,
    pub(crate) art: HashMap<String, ArtState>,
    pub(crate) cover: Option<CoverView>,
    pub(crate) seek_drag: Option<f32>,
    pub(crate) compact_tab: CompactTab,
    pub(crate) frame_stats: FrameStats,
    pub(crate) readout: bool,
    last_frame: Option<Instant>,
}

impl App {
    /// Build the application; returns the jobs to start.
    #[must_use]
    pub fn new(backend: Backend, options: &UiOptions) -> (Self, Vec<Job>) {
        let ui = UiState::new(
            options.start_route.clone(),
            backend.player.snapshot(),
            options.reduced_motion,
        );
        let mut app = Self {
            backend,
            ui,
            window: Size::new(options.window_size.0, options.window_size.1),
            home: None,
            art: HashMap::new(),
            cover: None,
            seek_drag: None,
            compact_tab: CompactTab::default(),
            frame_stats: FrameStats::default(),
            readout: options.show_frame_readout,
            last_frame: None,
        };
        let mut jobs = vec![Job::LoadHome];
        // The reducer starts without a current track; feeding the snapshot requests cover and
        // lyrics.
        app.ui.snapshot = nemo_core::player::PlayerSnapshot::default();
        jobs.extend(app.dispatch(Event::Snapshot(app.backend.player.snapshot())));
        jobs.extend(app.dispatch(Event::Position(app.backend.player.position())));
        jobs.extend(app.sync_viewport());
        (app, jobs)
    }

    /// Window title.
    #[must_use]
    pub fn title(&self) -> String {
        match &self.ui.snapshot.current {
            Some(t) if self.ui.is_playing() => format!("{} \u{b7} NemoMusic", t.title),
            _ => "NemoMusic".to_owned(),
        }
    }

    /// The iced theme.
    #[must_use]
    pub fn theme(&self) -> iced::Theme {
        style::theme()
    }

    /// The accent palette to draw now (mid cross-fade it is a mix).
    #[must_use]
    pub fn accent(&self) -> Palette {
        self.ui.accent.current()
    }

    /// Current window size in logical px (rounded).
    #[must_use]
    pub fn window_px(&self) -> (u32, u32) {
        // Window sizes are small positive numbers.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        (
            self.window.width.max(0.0) as u32,
            self.window.height.max(0.0) as u32,
        )
    }

    fn sync_viewport(&mut self) -> Vec<Job> {
        let (w, h) = self.window_px();
        let v = views::now_playing::lyrics_viewport_height(w, h);
        self.dispatch(Event::LyricsViewport(v))
    }

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
                }
            }
        }
        jobs
    }

    /// Request any art that Home shows and is not cached yet.
    fn art_jobs(&mut self, home: &HomeData) -> Vec<Job> {
        let (w, _) = self.window_px();
        // 2x the card for sharp covers on HiDPI screens.
        let px = layout::grid(w, false).card.max(layout::CARD_MIN) * 2;
        let mut artworks: Vec<Artwork> = Vec::new();
        artworks.extend(
            home.albums
                .iter()
                .map(|a| Artwork::Remote(a.thumbnails.clone())),
        );
        artworks.extend(
            home.artists
                .iter()
                .map(|a| Artwork::Remote(a.thumbnails.clone())),
        );
        artworks.extend(
            home.playlists
                .iter()
                .map(|p| Artwork::Remote(p.thumbnails.clone())),
        );
        artworks.extend(home.songs.iter().map(|t| t.artwork.clone()));
        artworks
            .into_iter()
            .filter_map(|artwork| {
                let key = art_key(&artwork)?;
                if self.art.contains_key(&key) {
                    return None;
                }
                self.art.insert(key.clone(), ArtState::Loading);
                Some(Job::Art { key, artwork, px })
            })
            .collect()
    }
    /// Handle a message; returns jobs to start. Shared by the window and the headless runner.
    pub fn update_core(&mut self, message: Message) -> Vec<Job> {
        match message {
            Message::Ui(ev) => self.dispatch(ev),
            Message::Player(ev) => self.dispatch(Event::Player(ev)),
            Message::Frame(now) => {
                self.frame_stats.record(now);
                let dt = self.last_frame.map_or(16.0, |p| {
                    now.saturating_duration_since(p).as_secs_f32() * 1000.0
                });
                self.last_frame = Some(now);
                let mut jobs = Vec::new();
                if self.ui.is_playing() {
                    jobs.extend(self.dispatch(Event::Position(self.backend.player.position())));
                }
                jobs.extend(self.dispatch(Event::Frame { dt_ms: dt }));
                if !self.ui.needs_frames() {
                    // Idle: the next burst of frames must not see this gap as a long frame.
                    self.last_frame = None;
                }
                jobs
            }
            Message::Key(key, mods) => match keys::map(key, mods, self.ui.text_focused) {
                Some(action) => self.dispatch(Event::Key(action)),
                None => Vec::new(),
            },
            Message::Resized(size) => {
                self.window = size;
                self.sync_viewport()
            }
            Message::HomeLoaded(result) => {
                let jobs = match &result {
                    Ok(home) => {
                        let home = Arc::clone(home);
                        self.art_jobs(&home)
                    }
                    Err(_) => Vec::new(),
                };
                self.home = Some(result);
                jobs
            }
            Message::Art { key, bytes } => {
                let state = match bytes {
                    Ok(b) => ArtState::Ready {
                        square: image::Handle::from_bytes(b.square.clone()),
                        circle: image::Handle::from_bytes(b.circle.clone()),
                    },
                    Err(_) => ArtState::Missing,
                };
                self.art.insert(key, state);
                Vec::new()
            }
            Message::Cover { track, built } => {
                let is_current = self.ui.snapshot.current.as_ref().map(|t| &t.id) == Some(&track);
                match built {
                    Ok(b) if is_current => {
                        self.cover = Some(CoverView {
                            track: track.clone(),
                            art: image::Handle::from_bytes(b.bytes.clone()),
                            art_bar: image::Handle::from_bytes(b.bar.clone()),
                            strip: b.data.backdrop_strip,
                            backdrop: image::Handle::from_bytes(b.data.backdrop_png.clone()),
                            tint_alpha: cover::glass_tint_alpha(b.data.backdrop_lightest),
                            scrim_alpha: cover::backdrop_scrim_alpha(b.data.backdrop_lightest),
                        });
                        self.dispatch(Event::CoverReady {
                            track,
                            palette: b.data.palette,
                        })
                    }
                    Err(_) if is_current => {
                        // No cover: neutral placeholder, fallback accent.
                        self.cover = None;
                        self.dispatch(Event::CoverReady {
                            track,
                            palette: Palette::fallback(),
                        })
                    }
                    Ok(_) | Err(_) => Vec::new(),
                }
            }
            Message::Lyrics {
                track,
                lookup,
                offset_ms,
            } => self.dispatch(Event::LyricsReady {
                track,
                lookup: lookup.as_ref().clone(),
                offset_ms,
            }),
            Message::SeekDrag(f) => {
                self.seek_drag = Some(f);
                Vec::new()
            }
            Message::SeekRelease => match self.seek_drag.take() {
                Some(f) => self.dispatch(Event::SeekFraction(f)),
                None => Vec::new(),
            },
            Message::PlayShelfSong(index) => {
                let songs = match &self.home {
                    Some(Ok(h)) => h.songs.clone(),
                    _ => return Vec::new(),
                };
                self.dispatch(Event::Command(PlayerCommand::SetQueue {
                    tracks: songs,
                    start: index,
                }))
            }
            Message::CompactTab(tab) => {
                self.compact_tab = tab;
                Vec::new()
            }
            Message::Noop => Vec::new(),
        }
    }

    fn jobs_to_task(&self, jobs: Vec<Job>) -> Task<Message> {
        Task::batch(jobs.into_iter().map(|job| {
            let backend = self.backend.clone();
            Task::perform(run_job(job, backend), |m| m)
        }))
    }

    /// iced `update`.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        let jobs = self.update_core(message);
        self.jobs_to_task(jobs)
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
        if self.ui.needs_frames() || self.readout {
            subs.push(window::frames().map(Message::Frame));
        }
        Subscription::batch(subs)
    }

    /// Finish animations and cross-fades (headless runs).
    pub fn finish_animations(&mut self) {
        for _ in 0..240 {
            self.dispatch(Event::Frame { dt_ms: 16.0 });
        }
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

fn map_key(event: keyboard::Event) -> Option<Message> {
    use keyboard::key::Named;
    let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
        return None;
    };
    let mapped = match key.as_ref() {
        keyboard::Key::Named(Named::Space) => Key::Space,
        keyboard::Key::Named(Named::ArrowLeft) => Key::Left,
        keyboard::Key::Named(Named::ArrowRight) => Key::Right,
        keyboard::Key::Named(Named::ArrowUp) => Key::Up,
        keyboard::Key::Named(Named::ArrowDown) => Key::Down,
        keyboard::Key::Named(Named::Escape) => Key::Escape,
        keyboard::Key::Named(Named::F3) => Key::F3,
        keyboard::Key::Character(c) => Key::Char(c.chars().next()?.to_ascii_lowercase()),
        _ => return None,
    };
    Some(Message::Key(
        mapped,
        Mods {
            ctrl: modifiers.control() || modifiers.command(),
            shift: modifiers.shift(),
        },
    ))
}

/// Subscription identity: one stream per player.
struct PlayerKey(Arc<dyn PlayerControl>);

impl Hash for PlayerKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // The address of the player object identifies it.
        std::ptr::addr_of!(*self.0).cast::<()>().hash(state);
    }
}

/// Poll the player's event receiver on a plain thread (the trait is non-blocking and
/// runtime-agnostic) and forward real events only, so an idle player causes no UI work.
fn player_events(key: &PlayerKey) -> impl iced::futures::Stream<Item = Message> + use<> {
    let player = Arc::clone(&key.0);
    stream::channel(
        64,
        async move |mut tx: iced::futures::channel::mpsc::Sender<Message>| {
            let mut rx = player.subscribe();
            thread::spawn(move || {
                loop {
                    let mut sent = false;
                    while let Some(ev) = rx.try_next() {
                        if tx.try_send(Message::Player(ev)).is_err() && tx.is_closed() {
                            return;
                        }
                        sent = true;
                    }
                    if !sent && tx.is_closed() {
                        return;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            });
        },
    )
}
