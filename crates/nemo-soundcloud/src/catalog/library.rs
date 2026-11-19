//! The signed-in user's own SoundCloud library (soundcloud-account criteria 11 and 12): liked
//! tracks, the user's playlists and albums, liked playlists and albums, followed artists, the
//! feed and the play history.
//!
//! Every list is read page by page. A page answers `{collection, next_href}`; the next page is
//! requested only when the caller asks for it, and only when `next_href` is an address on the API
//! host (anything else is dropped, so the token and the client id never go elsewhere). The
//! one-call helpers behind the `Catalog` methods read at most [`MAX_PAGES`] pages, at the pace of
//! the client (one call at a time, 250 ms apart). Nothing is kept here: a read while signed out
//! answers "Sign in to SoundCloud", and signing out or another user's sign-in leaves nothing of
//! the earlier account to show. Reads happen when the library opens and on Refresh, never in the
//! background.

use std::future::Future;
use std::time::{SystemTime, UNIX_EPOCH};

use nemo_core::catalog::{
    AlbumSummary, ArtistSummary, CatalogError, HistorySection, PlaylistSummary,
};
use nemo_core::track::Track;
use serde::Deserialize;
use serde_json::Value;

use super::{Slot, SoundcloudCatalog, mapped, to_catalog};
use crate::map;
use crate::model::{Item, ScPlaylist, ScTrack, ScUser, parse_item};

/// Items asked for in one page.
const PAGE_LIMIT: &str = "50";
/// Most pages one `Catalog` call reads: 200 items per list.
pub const MAX_PAGES: usize = 4;
/// The text a read gives while nobody is signed in.
const SIGN_IN: &str = "Sign in to SoundCloud to see this";
/// Milliseconds in a day.
const DAY_MS: i64 = 86_400_000;

/// The address of the next page. It is always on the API host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextHref(String);

impl NextHref {
    /// The address, for the next call.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One page of a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    /// The items of this page, in the order SoundCloud gives them.
    pub items: Vec<T>,
    /// The next page, when there is one.
    pub next: Option<NextHref>,
}

/// The user's playlists and albums, own and liked, from one or more pages of the library.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Library {
    /// Playlists the user made.
    pub playlists: Vec<PlaylistSummary>,
    /// Albums the user made.
    pub albums: Vec<AlbumSummary>,
    /// Playlists the user liked.
    pub liked_playlists: Vec<PlaylistSummary>,
    /// Albums the user liked.
    pub liked_albums: Vec<AlbumSummary>,
}

impl Library {
    fn extend(&mut self, other: Self) {
        self.playlists.extend(other.playlists);
        self.albums.extend(other.albums);
        self.liked_playlists.extend(other.liked_playlists);
        self.liked_albums.extend(other.liked_albums);
    }
}

/// One played track and when (milliseconds since 1970).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Played {
    /// When it played.
    pub at_ms: i64,
    /// The track.
    pub track: Track,
}

/// The raw answer of a list call.
#[derive(Debug, Default, Deserialize)]
struct Collection {
    #[serde(default)]
    collection: Vec<Value>,
    next_href: Option<String>,
}

/// An item of the library, the history or the feed: a `track` or a `playlist` object with the
/// words that say what it is.
fn object(item: &Value, key: &str) -> Option<Value> {
    item.get(key).filter(|v| v.is_object()).cloned()
}

fn word<'a>(item: &'a Value, key: &str) -> &'a str {
    item.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn playlist_of(raw: Value) -> Option<ScPlaylist> {
    match parse_item(raw)? {
        Item::Playlist(p) => Some(p),
        _ => None,
    }
}

fn track_of(raw: Value) -> Option<ScTrack> {
    match parse_item(raw)? {
        Item::Track(t) => Some(t),
        _ => None,
    }
}

fn user_of(raw: Value) -> Option<ScUser> {
    match parse_item(raw)? {
        Item::User(u) => Some(u),
        _ => None,
    }
}

/// Sort the playlist items of a library page. Own and liked are told apart by the item's `type`
/// (`playlist`, `playlist-like`); an item of another type (a liked track, a system mix) is not
/// part of this list.
fn library_of(items: Vec<Value>) -> Library {
    let mut out = Library::default();
    for item in items {
        let liked = match word(&item, "type") {
            "playlist" => false,
            "playlist-like" => true,
            _ => continue,
        };
        let Some(p) = object(&item, "playlist").and_then(playlist_of) else {
            continue;
        };
        let art = super::first_track_art(&p);
        match (p.is_album, liked) {
            (false, false) => out.playlists.extend(map::playlist(&p, art.as_deref())),
            (false, true) => out
                .liked_playlists
                .extend(map::playlist(&p, art.as_deref())),
            (true, false) => out.albums.extend(map::album(&p, art.as_deref())),
            (true, true) => out.liked_albums.extend(map::album(&p, art.as_deref())),
        }
    }
    out
}

/// The label of the day a track played on, from how many whole days ago it was.
fn day_label(days_ago: i64) -> String {
    match days_ago {
        ..=0 => "Today".to_owned(),
        1 => "Yesterday".to_owned(),
        2..=6 => format!("{days_ago} days ago"),
        _ => "Earlier".to_owned(),
    }
}

/// Group plays (most recent first) into sections by day, as `now_ms` sees the days (UTC).
#[must_use]
pub fn sections(plays: Vec<Played>, now_ms: i64) -> Vec<HistorySection> {
    let today = now_ms.div_euclid(DAY_MS);
    let mut out: Vec<HistorySection> = Vec::new();
    for play in plays {
        let title = day_label(today - play.at_ms.div_euclid(DAY_MS));
        match out.last_mut() {
            Some(last) if last.title == title => last.tracks.push(play.track),
            _ => out.push(HistorySection {
                title,
                tracks: vec![play.track],
            }),
        }
    }
    out
}

/// Now, in milliseconds since 1970 (0 before 1970, which cannot happen).
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Read pages with `fetch` until there is no next page or [`MAX_PAGES`] were read.
async fn all<T, F, Fut>(mut fetch: F) -> Result<Vec<T>, CatalogError>
where
    F: FnMut(Option<NextHref>) -> Fut,
    Fut: Future<Output = Result<Page<T>, CatalogError>>,
{
    let mut out = Vec::new();
    let mut next = None;
    for _ in 0..MAX_PAGES {
        let page = fetch(next.take()).await?;
        out.extend(page.items);
        next = page.next;
        if next.is_none() {
            break;
        }
    }
    Ok(out)
}

impl SoundcloudCatalog {
    /// The signed-in user's id, or a plain "sign in" answer.
    fn me(&self) -> Result<u64, CatalogError> {
        self.client
            .auth()
            .user_id()
            .ok_or_else(|| CatalogError::Unavailable(SIGN_IN.to_owned()))
    }

    /// One page of the list at `path` (or the page `after`): the raw items and the next address.
    async fn list_page(
        &self,
        path: &str,
        after: Option<&NextHref>,
    ) -> Result<(Vec<Value>, Option<NextHref>), CatalogError> {
        let body = match after {
            Some(next) => self.client.get_api_url(next.as_str(), &[]).await,
            None => {
                self.client
                    .get(path, &[("limit", PAGE_LIMIT), ("linked_partitioning", "1")])
                    .await
            }
        }
        .map_err(to_catalog)?;
        let page: Collection = serde_json::from_slice(&body)
            .map_err(|_| CatalogError::Parse("unexpected answer from SoundCloud".to_owned()))?;
        let next = page
            .next_href
            .filter(|href| self.client.is_api_url(href))
            .map(NextHref);
        Ok((page.collection, next))
    }

    /// One page of the tracks the user liked.
    ///
    /// # Errors
    /// "Sign in to SoundCloud to see this" while signed out, and the errors of every catalog call.
    pub async fn likes_page(&self, after: Option<&NextHref>) -> Result<Page<Track>, CatalogError> {
        let me = self.me()?;
        let (items, next) = self
            .list_page(&format!("/users/{me}/track_likes"), after)
            .await?;
        let raw = items.iter().filter_map(|i| object(i, "track")).collect();
        Ok(Page {
            items: self.tracks_filled(raw).await?,
            next,
        })
    }

    /// One page of the library: the user's playlists and albums, own and liked.
    ///
    /// # Errors
    /// As [`SoundcloudCatalog::likes_page`].
    pub async fn library_page(
        &self,
        after: Option<&NextHref>,
    ) -> Result<Page<Library>, CatalogError> {
        self.me()?;
        let (items, next) = self.list_page("/me/library/all", after).await?;
        Ok(Page {
            items: vec![library_of(items)],
            next,
        })
    }

    /// One page of the artists the user follows.
    ///
    /// # Errors
    /// As [`SoundcloudCatalog::likes_page`].
    pub async fn followings_page(
        &self,
        after: Option<&NextHref>,
    ) -> Result<Page<ArtistSummary>, CatalogError> {
        let me = self.me()?;
        let (items, next) = self
            .list_page(&format!("/users/{me}/followings"), after)
            .await?;
        Ok(Page {
            items: items
                .into_iter()
                .filter_map(user_of)
                .filter_map(|u| map::artist(&u))
                .collect(),
            next,
        })
    }

    /// One page of the user's feed: the tracks of the artists they follow, reposts included.
    /// Playlists in the feed are left out.
    ///
    /// # Errors
    /// As [`SoundcloudCatalog::likes_page`].
    pub async fn feed_page(&self, after: Option<&NextHref>) -> Result<Page<Track>, CatalogError> {
        self.me()?;
        let (items, next) = self.list_page("/stream", after).await?;
        let raw = items.iter().filter_map(|i| object(i, "track")).collect();
        Ok(Page {
            items: self.tracks_filled(raw).await?,
            next,
        })
    }

    /// One page of the play history, most recent first.
    ///
    /// # Errors
    /// As [`SoundcloudCatalog::likes_page`].
    pub async fn history_page(
        &self,
        after: Option<&NextHref>,
    ) -> Result<Page<Played>, CatalogError> {
        self.me()?;
        let (items, next) = self.list_page("/me/play-history/tracks", after).await?;
        let times: Vec<i64> = items
            .iter()
            .filter(|i| object(i, "track").is_some())
            .map(|i| i.get("played_at").and_then(Value::as_i64).unwrap_or(0))
            .collect();
        let raw = items.iter().filter_map(|i| object(i, "track")).collect();
        let tracks = self.tracks_filled_keep_order(raw).await?;
        let items = times
            .into_iter()
            .zip(tracks)
            .filter_map(|(at_ms, track)| {
                Some(Played {
                    at_ms,
                    track: track?,
                })
            })
            .collect();
        Ok(Page { items, next })
    }

    /// The tracks of `raw` objects, in order. Stubs (an id and no title) are filled in batches.
    async fn tracks_filled(&self, raw: Vec<Value>) -> Result<Vec<Track>, CatalogError> {
        Ok(self
            .tracks_filled_keep_order(raw)
            .await?
            .into_iter()
            .flatten()
            .collect())
    }

    /// Like [`Self::tracks_filled`], with `None` where a track could not be read.
    async fn tracks_filled_keep_order(
        &self,
        raw: Vec<Value>,
    ) -> Result<Vec<Option<Track>>, CatalogError> {
        let mut slots = Vec::new();
        let mut order = Vec::new();
        for value in raw {
            match track_of(value) {
                Some(t) => match mapped(self.client.records(), &t) {
                    Some(full) => {
                        order.push(true);
                        slots.push(Slot::Full(Box::new(full)));
                    }
                    None if t.title.is_none() => {
                        order.push(true);
                        slots.push(Slot::Stub(t.id));
                    }
                    None => order.push(false),
                },
                None => order.push(false),
            }
        }
        let mut filled = self.fill_slots(slots).await?.into_iter();
        Ok(order
            .into_iter()
            .map(|has_slot| {
                if has_slot {
                    filled.next().flatten()
                } else {
                    None
                }
            })
            .collect())
    }

    /// The tracks the user liked, up to [`MAX_PAGES`] pages.
    pub(super) async fn all_likes(&self) -> Result<Vec<Track>, CatalogError> {
        all(|after| async move { self.likes_page(after.as_ref()).await }).await
    }

    /// The library, up to [`MAX_PAGES`] pages.
    pub(super) async fn all_library(&self) -> Result<Library, CatalogError> {
        let mut out = Library::default();
        for part in all(|after| async move { self.library_page(after.as_ref()).await }).await? {
            out.extend(part);
        }
        Ok(out)
    }

    /// The followed artists, up to [`MAX_PAGES`] pages.
    pub(super) async fn all_followings(&self) -> Result<Vec<ArtistSummary>, CatalogError> {
        all(|after| async move { self.followings_page(after.as_ref()).await }).await
    }

    /// The history of the first [`MAX_PAGES`] pages, in sections by day.
    pub(super) async fn all_history(&self) -> Result<Vec<HistorySection>, CatalogError> {
        let plays = all(|after| async move { self.history_page(after.as_ref()).await }).await?;
        Ok(sections(plays, now_ms()))
    }
}

#[cfg(test)]
mod tests;
