//! Running a [`Job`] against the backend.

use super::message::{AccountBundle, CoverBuilt, Job, Message};
use super::model::{AccountMsg, DetailData, DetailKind};
use crate::cover;
use crate::home;
use crate::route::Route;
use crate::search::DEBOUNCE_MS;
use crate::state::{Event, ResolveKind};
use nemo_core::lyrics::LyricsQuery;
use nemo_core::services::Backend;
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
        Job::Debounce(seq) => {
            sleep_without_blocking(DEBOUNCE_MS).await;
            Message::Ui(Event::SearchDebounced(seq))
        }
        Job::Detail(key) => {
            let result = match key.kind {
                DetailKind::Album => backend.catalog.album(&key.id).await.map(DetailData::Album),
                DetailKind::Artist => backend
                    .catalog
                    .artist(&key.id)
                    .await
                    .map(DetailData::Artist),
                DetailKind::Playlist => backend
                    .catalog
                    .playlist(&key.id)
                    .await
                    .map(DetailData::Playlist),
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
        Job::Cover(track) => {
            let built = match backend.images.image(&track.artwork, 640).await {
                Ok(raw) => cover::analyze(&raw)
                    .and_then(|data| {
                        let art = cover::bake(&raw, 480, 24.0 / 480.0)?;
                        let bar = cover::bake(&raw, 128, 12.0 / 48.0)?;
                        Ok(Arc::new(CoverBuilt { art, bar, data }))
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

/// Call the account and explore parts of the catalog one after the other. Every call may fail
/// (signed out, offline, not built): a failure just leaves that part empty.
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
    if let Ok(e) = c.explore().await {
        b.daily.new_releases = e.new_releases;
        b.daily.moods = e.moods;
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
