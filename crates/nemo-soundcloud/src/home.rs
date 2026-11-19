//! Home, mixes, charts and radio: the reads of soundcloud-full task 10 (no screens).
//!
//! What the web app calls, read from its script and checked against the real service on
//! 2026-11-19 (anonymous):
//!
//! - `GET /mixed-selections`: the home page, rows of playlists and system playlists (the trending
//!   mixes by genre; signed in it also holds the account's own mixes).
//! - `GET /charts?kind=trending&genre=soundcloud:genres:all-music`: the trending chart, entries
//!   `{score, track}`. `kind=top`, other genres and regions answered 404 or 400 without a
//!   session, so only the trending chart of all music is read.
//! - `GET /charts/selections`: rows of chart playlists (Music Charts US, UK).
//! - `GET /system-playlists/soundcloud:system-playlists:<tail>`: one mix, its tracks as ids.
//! - `GET /stations/soundcloud:track-stations:<track id>/tracks`: the radio of a track.
//!
//! Everything is a page the UI already knows: a mix, a chart and a radio open through
//! [`Catalog::playlist`](nemo_core::catalog::Catalog::playlist) with the page ids below, so no new
//! screen type is needed.
//!
//! | Page | `BrowseId` | Opened by |
//! |---|---|---|
//! | a system playlist (a mix) | `sc:system:<tail>` | `playlist` |
//! | a chart | `sc:chart:trending:all-music` | `playlist` |
//! | the radio of a track | `sc:station:<track id>` | `playlist`, or [`SoundcloudCatalog::radio`] for the tracks only |

use nemo_core::catalog::{
    BrowseId, CatalogError, CollectionMeta, Explore, FeedItem, FeedShelf, HomeFeed, PlaylistDetail,
    PlaylistSummary, Shelf, ShelfItem, ShelfKind,
};
use nemo_core::track::{Thumbnail, Track, TrackId};
use serde_json::Value;

use crate::catalog::{Slot, SoundcloudCatalog, mapped, to_catalog, tracks_of};
use crate::map;
use crate::model::{Item, ScSelection, ScSystemPlaylist, parse_item, parse_list, parse_one};

/// The urn prefix of a system playlist.
const SYSTEM_URN: &str = "soundcloud:system-playlists:";
/// The urn of the chart genre that answers without a session.
const ALL_MUSIC: &str = "soundcloud:genres:all-music";
/// Entries asked of the trending chart.
const CHART_LIMIT: &str = "30";
/// Longest tail of a system playlist urn.
const MAX_TAIL: usize = 100;
/// Titles of the pages that have none of their own. The UI may replace them.
const CHART_TITLE: &str = "Trending on SoundCloud";
const STATION_TITLE: &str = "Track radio";

/// A page that is not a playlist, album or user of SoundCloud's own numbering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page {
    /// A system playlist; the text is the urn after `soundcloud:system-playlists:`.
    System(String),
    /// The trending chart of all music.
    Chart,
    /// The radio of a track.
    Station(u64),
}

impl Page {
    /// Read a `sc:system:…`, `sc:chart:…` or `sc:station:…` page id. The text is checked, so it
    /// is safe inside a request path.
    #[must_use]
    pub fn parse(id: &BrowseId) -> Option<Self> {
        let rest = id.0.strip_prefix("sc:")?;
        if let Some(tail) = rest.strip_prefix("system:") {
            let ok = !tail.is_empty()
                && tail.len() <= MAX_TAIL
                && tail.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b':' | b'-' | b'_')
                });
            return ok.then(|| Self::System(tail.to_owned()));
        }
        if rest == "chart:trending:all-music" {
            return Some(Self::Chart);
        }
        let digits = rest.strip_prefix("station:")?;
        let numeric = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
        numeric
            .then(|| digits.parse().ok().map(Self::Station))
            .flatten()
    }

    /// The page id of this page.
    #[must_use]
    pub fn id(&self) -> BrowseId {
        BrowseId(match self {
            Self::System(tail) => format!("sc:system:{tail}"),
            Self::Chart => "sc:chart:trending:all-music".to_owned(),
            Self::Station(track) => format!("sc:station:{track}"),
        })
    }
}

/// The cover sizes of a system playlist: its own picture, else the one SoundCloud computes.
fn system_cover(p: &ScSystemPlaylist) -> Vec<Thumbnail> {
    let art = p
        .artwork_url
        .as_deref()
        .or(p.calculated_artwork_url.as_deref());
    map::thumbnails(art)
}

/// The tail of a system playlist urn, if it is one.
fn system_tail(urn: &str) -> Option<&str> {
    urn.strip_prefix(SYSTEM_URN).filter(|t| !t.is_empty())
}

fn system_summary(p: &ScSystemPlaylist) -> Option<PlaylistSummary> {
    let tail = system_tail(&p.urn)?;
    let id = Page::System(tail.to_owned()).id();
    Page::parse(&id)?;
    Some(PlaylistSummary {
        id,
        title: p.title.clone().filter(|t| !t.is_empty())?,
        author: p.user.as_ref().and_then(|u| u.username.clone()),
        thumbnails: system_cover(p),
    })
}

/// One entry of a selection as a shelf item. Playlists, system playlists, tracks and users are
/// known; any other kind is skipped.
fn shelf_item(raw: Value, catalog: &SoundcloudCatalog) -> Option<FeedItem> {
    let kind = raw.get("kind")?.as_str()?.to_owned();
    if kind == "system-playlist" {
        let system: ScSystemPlaylist = serde_json::from_value(raw).ok()?;
        return system_summary(&system).map(FeedItem::Playlist);
    }
    match parse_item(raw)? {
        Item::Track(t) => mapped(catalog.client.records(), &t).map(FeedItem::Song),
        Item::User(u) => map::artist(&u).map(FeedItem::Artist),
        Item::Playlist(p) if p.is_album => map::album(&p, None).map(FeedItem::Album),
        Item::Playlist(p) => map::playlist(&p, None).map(FeedItem::Playlist),
    }
}

/// The shelves of a `{collection: [selection]}` answer. A selection without a usable item is
/// dropped.
fn shelves(body: &[u8], catalog: &SoundcloudCatalog) -> Result<Vec<FeedShelf>, CatalogError> {
    let mut out = Vec::new();
    for raw in parse_list(body).map_err(to_catalog)? {
        let Ok(selection) = serde_json::from_value::<ScSelection>(raw) else {
            continue;
        };
        let items: Vec<FeedItem> = selection
            .items
            .collection
            .into_iter()
            .filter_map(|item| shelf_item(item, catalog))
            .collect();
        let title = selection.title.filter(|t| !t.is_empty());
        if let (Some(title), false) = (title, items.is_empty()) {
            out.push(FeedShelf {
                title,
                subtitle: None,
                kind: ShelfKind::Cards,
                items,
            });
        }
    }
    Ok(out)
}

/// The old shelf type holds songs, albums, artists and playlists; the others are left out.
fn plain_shelf(shelf: FeedShelf) -> Shelf {
    let items = shelf
        .items
        .into_iter()
        .filter_map(|item| match item {
            FeedItem::Song(t) | FeedItem::Video(t) => Some(ShelfItem::Song(t)),
            FeedItem::Album(a) => Some(ShelfItem::Album(a)),
            FeedItem::Artist(a) => Some(ShelfItem::Artist(a)),
            FeedItem::Playlist(p) => Some(ShelfItem::Playlist(p)),
            _ => None,
        })
        .collect();
    Shelf {
        title: shelf.title,
        items,
    }
}

/// The tracks of a chart answer, in the chart's order.
fn chart_tracks(body: &[u8], catalog: &SoundcloudCatalog) -> Result<Vec<Track>, CatalogError> {
    let entries = parse_list(body).map_err(to_catalog)?;
    let tracks = entries
        .into_iter()
        .filter_map(|mut entry| entry.get_mut("track").map(Value::take))
        .collect();
    Ok(tracks_of(tracks, catalog.client.records()))
}

impl SoundcloudCatalog {
    /// The home page as rows. One request.
    pub(crate) async fn home_shelves(&self) -> Result<Vec<Shelf>, CatalogError> {
        let rows = self.home_rows().await?;
        Ok(rows.into_iter().map(plain_shelf).collect())
    }

    /// The home page as a feed (no mood chips). One request.
    pub(crate) async fn home_page(&self) -> Result<HomeFeed, CatalogError> {
        Ok(HomeFeed {
            chips: Vec::new(),
            shelves: self.home_rows().await?,
        })
    }

    async fn home_rows(&self) -> Result<Vec<FeedShelf>, CatalogError> {
        let body = self
            .client
            .get("/mixed-selections", &[])
            .await
            .map_err(to_catalog)?;
        shelves(&body, self)
    }

    /// The charts page: the trending chart as a song list, and the chart playlists of the
    /// countries SoundCloud lists. Two requests; one failing leaves the other.
    pub(crate) async fn explore_page(&self) -> Result<Explore, CatalogError> {
        let trending = self.trending_tracks().await;
        let selections = self.client.get("/charts/selections", &[]).await;
        let lists = match &selections {
            Ok(body) => shelves(body, self),
            Err(e) => Err(to_catalog(*e)),
        };
        match (trending, lists) {
            (Err(e), Err(_)) => Err(e),
            (trending, lists) => {
                let mut out = Explore::default();
                if let Ok(tracks) = trending {
                    out.charts.push(FeedShelf {
                        title: CHART_TITLE.to_owned(),
                        subtitle: None,
                        kind: ShelfKind::QuickPicks,
                        items: tracks.into_iter().map(FeedItem::Song).collect(),
                    });
                }
                out.shelves = lists.unwrap_or_default();
                Ok(out)
            }
        }
    }

    async fn trending_tracks(&self) -> Result<Vec<Track>, CatalogError> {
        let body = self
            .client
            .get(
                "/charts",
                &[
                    ("kind", "trending"),
                    ("genre", ALL_MUSIC),
                    ("limit", CHART_LIMIT),
                ],
            )
            .await
            .map_err(to_catalog)?;
        chart_tracks(&body, self)
    }

    /// The tracks SoundCloud plays after `track` in its radio (about 50, a plain list, no
    /// paging). One request.
    ///
    /// # Errors
    /// [`CatalogError::NotFound`] for an id that is not a SoundCloud track id.
    pub async fn radio(&self, track: &TrackId) -> Result<Vec<Track>, CatalogError> {
        let id = map::parse_track_id(track).ok_or(CatalogError::NotFound)?;
        self.station_tracks(id).await
    }

    async fn station_tracks(&self, track: u64) -> Result<Vec<Track>, CatalogError> {
        let path = format!("/stations/soundcloud:track-stations:{track}/tracks");
        let body = self.client.get(&path, &[]).await.map_err(to_catalog)?;
        Ok(tracks_of(
            parse_list(&body).map_err(to_catalog)?,
            self.client.records(),
        ))
    }

    /// A mix, the chart or a radio as a playlist page.
    pub(crate) async fn extra_page(&self, page: Page) -> Result<PlaylistDetail, CatalogError> {
        let (title, author, thumbnails, tracks) = match &page {
            Page::System(tail) => self.system_page(tail).await?,
            Page::Chart => (
                CHART_TITLE.to_owned(),
                None,
                Vec::new(),
                self.trending_tracks().await?,
            ),
            Page::Station(track) => (
                STATION_TITLE.to_owned(),
                None,
                Vec::new(),
                self.station_tracks(*track).await?,
            ),
        };
        let thumbnails = if thumbnails.is_empty() {
            tracks
                .first()
                .map(|t| match &t.artwork {
                    nemo_core::track::Artwork::Remote(thumbs) => thumbs.clone(),
                    _ => Vec::new(),
                })
                .unwrap_or_default()
        } else {
            thumbnails
        };
        Ok(PlaylistDetail {
            meta: CollectionMeta {
                owner: author.clone(),
                track_count: u32::try_from(tracks.len()).ok(),
                ..CollectionMeta::default()
            },
            summary: PlaylistSummary {
                id: page.id(),
                title,
                author,
                thumbnails,
            },
            tracks,
            edit: None,
        })
    }

    async fn system_page(
        &self,
        tail: &str,
    ) -> Result<(String, Option<String>, Vec<Thumbnail>, Vec<Track>), CatalogError> {
        let path = format!("/system-playlists/{SYSTEM_URN}{tail}");
        let body = self.client.get(&path, &[]).await.map_err(to_catalog)?;
        let system: ScSystemPlaylist = parse_one(&body).map_err(to_catalog)?;
        let title = system
            .title
            .clone()
            .filter(|t| !t.is_empty())
            .ok_or(CatalogError::NotFound)?;
        let mut slots = Vec::new();
        for raw in system
            .tracks
            .iter()
            .take(crate::catalog::MAX_PLAYLIST_TRACKS)
        {
            let Some(Item::Track(t)) = parse_item(raw.clone()) else {
                continue;
            };
            match mapped(self.client.records(), &t) {
                Some(full) => slots.push(Slot::Full(Box::new(full))),
                None if t.title.is_none() => slots.push(Slot::Stub(t.id)),
                None => {}
            }
        }
        let tracks = self.fill_stubs(slots).await?;
        let author = system.user.as_ref().and_then(|u| u.username.clone());
        Ok((title, author, system_cover(&system), tracks))
    }
}

#[cfg(test)]
mod tests;
