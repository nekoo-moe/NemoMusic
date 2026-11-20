//! Running a [`Job`] against the backend.

use super::art::device_px;
use super::message::{AccountBundle, CoverBuilt, ExploreMsg, Job, Message, RefreshData};
use super::model::{AccountMsg, DetailData, DetailKey, DetailKind, ScLibData, ScMsg};
use super::sc_write::ScWriteJob;
use crate::cover;
use crate::home;
use crate::refresh::{Start, Step};
use crate::route::Route;
use crate::search::DEBOUNCE_MS;
use crate::state::{Event, ResolveKind};
use nemo_core::account::SignOutReport;
use nemo_core::catalog::CatalogError;
use nemo_core::lyrics::LyricsQuery;
use nemo_core::secret::Secret;
use nemo_core::services::Backend;
use nemo_core::source_account::{SourceAccountError, SourceAccountInfo};
use nemo_core::source_writes::WriteError;
use nemo_core::track::{Artwork, Track};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// The link of a YouTube Music song.
pub(super) fn link_of(video_id: &str) -> String {
    format!("https://music.youtube.com/watch?v={video_id}")
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

pub(super) fn sleep_without_blocking(ms: u64) -> impl std::future::Future<Output = ()> {
    let (tx, rx) = iced::futures::channel::oneshot::channel::<()>();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(ms));
        let _ = tx.send(());
    });
    async move {
        let _ = rx.await;
    }
}

/// Run a job to completion.
pub async fn run_job(job: Job, backend: Backend) -> Message {
    match job {
        Job::PickFolder(picker) => Message::FolderPicked(picker.pick().await),
        Job::HomeFeed => Message::HomeFeed(
            backend
                .catalog
                .home()
                .await
                .map(Arc::new)
                .map_err(|e| e.to_string()),
        ),
        Job::HomeSeed(index) => {
            let (_, query) = home::SEED_SHELVES[index.min(home::SEED_SHELVES.len() - 1)];
            Message::HomeSeed {
                index,
                result: backend
                    .catalog
                    .search(query)
                    .await
                    .map(Arc::new)
                    .map_err(|e| e.to_string()),
            }
        }
        Job::Library => Message::Library(
            backend
                .catalog
                .search("")
                .await
                .map(Arc::new)
                .map_err(|e| e.to_string()),
        ),
        Job::Search(query) => {
            let result = backend
                .catalog
                .search(&query)
                .await
                .map(Arc::new)
                .map_err(|e| e.to_string());
            Message::SearchDone { query, result }
        }
        Job::SearchKind { query, kind } => {
            let result = backend
                .catalog
                .search_kind(&query, kind)
                .await
                .map(Arc::new)
                .map_err(|e| e.to_string());
            Message::Ui(Event::SearchFilterDone {
                query,
                kind,
                result,
            })
        }
        Job::HoverDelay(generation) => {
            sleep_without_blocking(super::hint::HOVER_HINT_MS).await;
            Message::HoverDue(generation)
        }
        Job::Debounce(seq) => {
            sleep_without_blocking(DEBOUNCE_MS).await;
            Message::Ui(Event::SearchDebounced(seq))
        }
        Job::Detail(key) => {
            let catalog = detail_catalog(&backend, &key);
            let result = match key.kind {
                DetailKind::Album => catalog.album(&key.id).await.map(DetailData::Album),
                DetailKind::Artist => catalog.artist(&key.id).await.map(DetailData::Artist),
                DetailKind::Playlist => catalog.playlist(&key.id).await.map(DetailData::Playlist),
            }
            .map(Arc::new)
            .map_err(|e| e.to_string());
            Message::Detail { key, result }
        }
        Job::Art(spec) => {
            let key = spec.key().unwrap_or_default();
            let pixels = match backend.images.image(&spec.artwork, spec.px()).await {
                Ok(raw) => cover::bake(&raw, spec.px(), spec.fraction())
                    .map(Arc::new)
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e.to_string()),
            };
            Message::Art { key, pixels }
        }
        Job::Cover(track, scale) => {
            // The largest cover slot (Now Playing, 480) and the bar slots (up to 96).
            let art_px = device_px(480, scale);
            let bar_px = device_px(96, scale);
            let built = match backend.images.image(&track.artwork, art_px).await {
                Ok(raw) => cover::analyze(&raw)
                    .and_then(|data| {
                        let art = cover::bake(&raw, art_px, 24.0 / 480.0)?;
                        let bar = cover::bake(&raw, bar_px, 12.0 / 48.0)?;
                        Ok(Arc::new(CoverBuilt { art, bar, data }))
                    })
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e.to_string()),
            };
            Message::Cover {
                track: track.id,
                scale,
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
        Job::LyricsAll(track) => {
            let list = backend
                .lyrics
                .lookup_all(&LyricsQuery::from_track(&track))
                .await;
            Message::LyricsCandidates {
                track: track.id,
                list: Arc::new(list),
            }
        }
        Job::SignIn => Message::Account(AccountMsg::SignedIn(backend.account.sign_in().await)),
        Job::SignInCookie(cookie) => Message::Account(AccountMsg::SignedIn(
            backend.account.sign_in_with_cookie(cookie).await,
        )),
        Job::SignOut => Message::Account(AccountMsg::SignedOut(backend.account.sign_out().await)),
        Job::ScSignIn => Message::Sc(ScMsg::WindowEnded(sc_sign_in(&backend, None).await)),
        Job::ScSignInToken(token) => {
            Message::Sc(ScMsg::SignedIn(sc_sign_in(&backend, Some(token)).await))
        }
        Job::ScSignOut => Message::Sc(ScMsg::SignedOut(sc_sign_out(&backend).await)),
        Job::ScWrite(job) => {
            let result = sc_write(&backend, &job).await;
            Message::ScWrite(super::sc_write::ScWriteMsg::Done(Box::new(
                super::sc_write::ScWriteDone { job, result },
            )))
        }
        Job::ScSearch(query) => {
            let result = match &backend.soundcloud {
                Some(c) => c
                    .search(&query)
                    .await
                    .map(Arc::new)
                    .map_err(|e| e.to_string()),
                None => Err(SC_NOT_RUNNING.to_owned()),
            };
            Message::SearchDone { query, result }
        }
        Job::ScSearchKind { query, kind } => {
            let result = match &backend.soundcloud {
                Some(c) => c
                    .search_kind(&query, kind)
                    .await
                    .map(Arc::new)
                    .map_err(|e| e.to_string()),
                None => Err(SC_NOT_RUNNING.to_owned()),
            };
            Message::Ui(Event::SearchFilterDone {
                query,
                kind,
                result,
            })
        }
        Job::ScLibrary { generation } => Message::Sc(ScMsg::LibraryLoaded {
            generation,
            result: sc_library(&backend).await.map(Arc::new),
        }),
        Job::Rate { track, liked } => {
            let result = match &track.video_id {
                Some(video) => backend
                    .catalog
                    .rate_song(video, liked)
                    .await
                    .map_err(|e| e.to_string()),
                None => Err("not a YouTube Music song".to_owned()),
            };
            Message::LikeDone {
                track: track.id,
                liked,
                result,
            }
        }
        Job::PageSeed { key, artwork } => match backend.images.image(&artwork, 96).await {
            Ok(raw) => match ::image::load_from_memory(&raw) {
                Ok(img) => {
                    let small = img.thumbnail(64, 64).to_rgba8();
                    Message::PageSeed {
                        key,
                        seed: crate::palette::seed_from_pixels(small.as_raw()),
                    }
                }
                Err(_) => Message::Noop,
            },
            Err(_) => Message::Noop,
        },
        Job::AccountData => Message::AccountData(Arc::new(account_bundle(&backend).await)),
        Job::Refresh(start) => refresh_step(start, &backend).await,
        Job::Explore => Message::Explore(ExploreMsg::Loaded(
            backend
                .catalog
                .explore()
                .await
                .map(Arc::new)
                .map_err(|e| e.to_string()),
        )),
        Job::Mood(id) => {
            let result = backend
                .catalog
                .mood_playlists(&id)
                .await
                .map(Arc::new)
                .map_err(|e| e.to_string());
            Message::Explore(ExploreMsg::MoodLoaded { id, result })
        }
        Job::Radio(seed) => {
            let query = format!(
                "{} {}",
                seed.artists.first().cloned().unwrap_or_default(),
                seed.title
            );
            let tracks = backend
                .catalog
                .search(query.trim())
                .await
                .map(|r| r.songs)
                .unwrap_or_default();
            Message::RadioReady { seed, tracks }
        }
        Job::Resolve { kind, track } => Message::Ui(resolve(&backend, kind, &track).await),
    }
}

/// Call the account parts of the catalog and the home feed one after the other. Every call may
/// fail (signed out, offline, not built): a failure just leaves that part empty. The Explore page
/// is its own job ([`Job::Explore`]); its new releases and moods reach the daily inputs there.
pub(super) async fn account_bundle(backend: &Backend) -> AccountBundle {
    use nemo_core::catalog::FeedItem;
    let c = &backend.catalog;
    let mut b = AccountBundle::default();
    if let Ok(v) = c.library_playlists().await {
        b.library.playlists = v;
        b.account_answered = true;
    }
    if let Ok(v) = c.liked_songs().await {
        b.library.songs.clone_from(&v);
        b.daily.liked = v;
        b.account_answered = true;
    }
    if let Ok(v) = c.library_albums().await {
        b.library.albums = v;
    }
    if let Ok(v) = c.library_artists().await {
        b.library.artists.clone_from(&v);
        b.daily.followed = v;
    }
    if let Ok(sections) = c.history().await {
        b.daily.history = sections.into_iter().flat_map(|s| s.tracks).collect();
    }
    if let Ok(feed) = c.home_feed().await {
        if b.daily.moods.is_empty() {
            b.daily.moods = feed.chips;
        }
        for shelf in &feed.shelves {
            for item in &shelf.items {
                match item {
                    FeedItem::Video(t) => b.daily.videos.push(t.clone()),
                    FeedItem::Podcast(p) => b.daily.podcasts.push(p.clone()),
                    _ => {}
                }
            }
        }
        b.daily.feed_shelves = feed.shelves;
    }
    b
}

/// Run one reload step. The account step is the same call chain as the start-up load.
async fn refresh_step(start: Start, backend: &Backend) -> Message {
    let data = match start.step {
        Step::Home => RefreshData::Home(match backend.catalog.home().await {
            Ok(shelves) => Ok(Some(Arc::new(shelves))),
            // No home page without a session: nothing to reload, and nothing failed.
            Err(CatalogError::Unavailable(_)) => Ok(None),
            Err(e) => Err(e.to_string()),
        }),
        Step::Account => RefreshData::Account(Arc::new(account_bundle(backend).await)),
    };
    Message::Refreshed {
        generation: start.generation,
        data,
    }
}

/// Find the page of a track's artist or album; fall back to a search for the name.
pub(super) async fn resolve(backend: &Backend, kind: ResolveKind, track: &Track) -> Event {
    let artist = track.artists.first().cloned().unwrap_or_default();
    let fallback = |q: String| Event::Go(Route::Search(q));
    match kind {
        ResolveKind::Artist => {
            if artist.is_empty() {
                return fallback(track.title.clone());
            }
            match backend.catalog.search(&artist).await {
                Ok(r) => r
                    .artists
                    .iter()
                    .find(|a| a.name.eq_ignore_ascii_case(&artist))
                    .or_else(|| r.artists.first())
                    .map_or_else(
                        || fallback(artist.clone()),
                        |a| Event::Go(Route::Artist(a.id.clone())),
                    ),
                Err(_) => fallback(artist),
            }
        }
        ResolveKind::Album => {
            let Some(album) = track.album.clone() else {
                return fallback(track.title.clone());
            };
            let query = format!("{album} {artist}").trim().to_owned();
            match backend.catalog.search(&query).await {
                Ok(r) => r
                    .albums
                    .iter()
                    .find(|a| a.title.eq_ignore_ascii_case(&album))
                    .or_else(|| r.albums.first())
                    .map_or_else(
                        || fallback(query.clone()),
                        |a| Event::Go(Route::Album(a.id.clone())),
                    ),
                Err(_) => fallback(query),
            }
        }
    }
}

/// The error of a SoundCloud account call when the app built no SoundCloud account.
fn no_sc_account() -> SourceAccountError {
    SourceAccountError::Unavailable("SoundCloud is not running".to_owned())
}

/// Sign in to SoundCloud through the window, or with a pasted `token`.
async fn sc_sign_in(
    backend: &Backend,
    token: Option<Secret<String>>,
) -> Result<SourceAccountInfo, SourceAccountError> {
    let Some(account) = &backend.soundcloud_account else {
        return Err(no_sc_account());
    };
    match token {
        Some(token) => account.sign_in_with_token(token).await,
        None => account.sign_in().await,
    }
}

/// Sign out of SoundCloud.
async fn sc_sign_out(backend: &Backend) -> Result<SignOutReport, SourceAccountError> {
    match &backend.soundcloud_account {
        Some(account) => account.sign_out().await,
        None => Err(no_sc_account()),
    }
}

/// Send one change of the SoundCloud account and report whether the server took it.
async fn sc_write(backend: &Backend, job: &ScWriteJob) -> Result<(), WriteError> {
    let Some(writes) = &backend.soundcloud_writes else {
        return Err(WriteError::NotSignedIn);
    };
    match job {
        ScWriteJob::Like { track, liked } => writes.set_track_liked(&track.id, *liked).await,
        ScWriteJob::Add { track, playlist } => writes.add_to_playlist(playlist, &track.id).await,
        ScWriteJob::Remove { track, playlist } => {
            writes.remove_from_playlist(playlist, &track.id).await
        }
        ScWriteJob::Create { title, first } => writes
            .create_playlist(title, first.as_ref().map(|t| &t.id))
            .await
            .map(|_| ()),
        ScWriteJob::Rename { playlist, title } => writes.rename_playlist(playlist, title).await,
        ScWriteJob::Unfollow { user } => writes.unfollow(user).await,
    }
}

/// Read the SoundCloud library one list after the other, at the client's pace. A list that fails
/// stays empty; when every list fails, the first reason is the answer.
async fn sc_library(backend: &Backend) -> Result<ScLibData, String> {
    let Some(c) = &backend.soundcloud else {
        return Err("SoundCloud is not running.".to_owned());
    };
    let mut data = ScLibData::default();
    let mut first_error = None;
    let mut answered = false;
    match c.liked_songs().await {
        Ok(v) => {
            data.likes = v;
            answered = true;
        }
        Err(e) => first_error = Some(e),
    }
    match c.library_playlists_owned().await {
        Ok((v, own)) => {
            data.playlists = v;
            data.own_playlists = own;
            answered = true;
        }
        Err(e) => first_error = first_error.or(Some(e)),
    }
    match c.library_albums().await {
        Ok(v) => {
            data.albums = v;
            answered = true;
        }
        Err(e) => first_error = first_error.or(Some(e)),
    }
    match c.library_artists().await {
        Ok(v) => {
            data.artists = v;
            answered = true;
        }
        Err(e) => first_error = first_error.or(Some(e)),
    }
    match c.history().await {
        Ok(v) => {
            data.history = v;
            answered = true;
        }
        Err(e) => first_error = first_error.or(Some(e)),
    }
    match (answered, first_error) {
        (true, _) | (false, None) => Ok(data),
        (false, Some(e)) => Err(e.to_string()),
    }
}

/// The catalog that answers a detail page: the SoundCloud one for a SoundCloud id, the app's
/// catalog (YouTube Music and local files) for every other.
fn detail_catalog<'a>(
    backend: &'a Backend,
    key: &DetailKey,
) -> &'a Arc<dyn nemo_core::catalog::Catalog> {
    match (&backend.soundcloud, key.is_soundcloud()) {
        (Some(soundcloud), true) => soundcloud,
        _ => &backend.catalog,
    }
}

/// What a SoundCloud search says when the app built no SoundCloud (the switch was turned off).
const SC_NOT_RUNNING: &str = "SoundCloud is not running.";
