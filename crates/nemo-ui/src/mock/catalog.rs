//! The mock catalogs: [`MockCatalog`] and the [`SeededCatalog`] that behaves like the merged one.

use std::sync::Arc;

use super::{
    ALBUM_TITLES, ARTIST_NAMES, ARTISTS, LOCAL_ALBUMS, PER_ALBUM, PLAYLISTS, TRACKS, album_summary,
    artist_summary, browse, index_of, local_album_index, local_album_summary, local_tracks,
    mock_backend, playlist_summary, thumbs, track, tracks,
};
use nemo_core::BoxFuture;
use nemo_core::audio::{
    AudioFormat, Codec, Origin, OutputInfo, OutputMode, ProcessingStage, SignalPath, StreamInfo,
};
use nemo_core::catalog::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, BrowseId, Catalog, CatalogError,
    Explore, FeedItem, FeedShelf, HistorySection, HomeFeed, MoodChip, PlaylistDetail,
    PlaylistSummary, PodcastSummary, SearchResults, Shelf, ShelfItem, ShelfKind,
};
use nemo_core::services::Backend;
use nemo_core::track::Track;

/// The fake catalog: 40 tracks, 10 albums, 8 artists, 6 playlists.
#[derive(Debug, Default, Clone, Copy)]
pub struct MockCatalog;

impl Catalog for MockCatalog {
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        let q = query.trim().to_lowercase();
        let hit = move |text: &str| q.is_empty() || text.to_lowercase().contains(&q);
        let songs = tracks()
            .into_iter()
            .filter(|t| hit(&t.title) || t.artists.iter().any(|a| hit(a)))
            .collect();
        let albums = (0..LOCAL_ALBUMS)
            .map(local_album_summary)
            .chain((0..TRACKS / PER_ALBUM).map(album_summary))
            .filter(|a| hit(&a.title) || a.artists.iter().any(|x| hit(x)))
            .collect();
        let artists = (0..ARTISTS)
            .map(artist_summary)
            .filter(|a| hit(&a.name))
            .collect();
        let playlists = (0..PLAYLISTS)
            .map(playlist_summary)
            .filter(|p| hit(&p.title))
            .collect();
        Box::pin(async move {
            Ok(SearchResults {
                songs,
                albums,
                artists,
                playlists,
            })
        })
    }

    fn album<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        let local = local_album_index(id).map(|a| AlbumDetail {
            summary: local_album_summary(a),
            tracks: local_tracks()
                .into_iter()
                .skip(a * PER_ALBUM)
                .take(PER_ALBUM)
                .collect(),
        });
        let result = local
            .or_else(|| {
                index_of(id, "album")
                    .filter(|a| *a < TRACKS / PER_ALBUM)
                    .map(|a| AlbumDetail {
                        summary: album_summary(a),
                        tracks: (a * PER_ALBUM..(a + 1) * PER_ALBUM).map(track).collect(),
                    })
            })
            .ok_or(CatalogError::NotFound);
        Box::pin(async move { result })
    }

    fn artist<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        let result = index_of(id, "artist")
            .filter(|a| *a < ARTISTS)
            .map(|a| {
                let albums: Vec<usize> = (0..TRACKS / PER_ALBUM)
                    .filter(|x| x % ARTISTS == a)
                    .collect();
                ArtistDetail {
                    summary: artist_summary(a),
                    top_songs: albums
                        .iter()
                        .flat_map(|al| (al * PER_ALBUM..(al + 1) * PER_ALBUM).map(track))
                        .take(5)
                        .collect(),
                    albums: albums.into_iter().map(album_summary).collect(),
                    description: Some("An invented artist for the mock catalog.".to_owned()),
                }
            })
            .ok_or(CatalogError::NotFound);
        Box::pin(async move { result })
    }

    fn playlist<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        let result = index_of(id, "playlist")
            .filter(|p| *p < PLAYLISTS)
            .map(|p| PlaylistDetail {
                summary: playlist_summary(p),
                tracks: (0..12).map(|i| track(p * 5 + i * 3)).collect(),
                edit: None,
            })
            .ok_or(CatalogError::NotFound);
        Box::pin(async move { result })
    }

    fn home(&self) -> BoxFuture<'_, Result<Vec<Shelf>, CatalogError>> {
        Box::pin(async {
            Ok(vec![
                Shelf {
                    title: "Albums for you".to_owned(),
                    items: (0..TRACKS / PER_ALBUM)
                        .map(|a| ShelfItem::Album(album_summary(a)))
                        .collect(),
                },
                Shelf {
                    title: "Artists".to_owned(),
                    items: (0..ARTISTS)
                        .map(|a| ShelfItem::Artist(artist_summary(a)))
                        .collect(),
                },
                Shelf {
                    title: "Playlists".to_owned(),
                    items: (0..PLAYLISTS)
                        .map(|p| ShelfItem::Playlist(playlist_summary(p)))
                        .collect(),
                },
                Shelf {
                    title: "Songs".to_owned(),
                    items: (0..12).map(|n| ShelfItem::Song(track(n))).collect(),
                },
            ])
        })
    }

    /// The richer home page of the mock: mood chips and every kind of shelf.
    fn home_feed(&self) -> BoxFuture<'_, Result<HomeFeed, CatalogError>> {
        Box::pin(async {
            let chips = ["Relax", "Focus", "Energize", "Sleep", "Party"]
                .iter()
                .enumerate()
                .map(|(i, t)| MoodChip {
                    title: (*t).to_owned(),
                    id: browse("mood", i),
                })
                .collect();
            let mk_shelf = |title: &str, kind: ShelfKind, items: Vec<FeedItem>| FeedShelf {
                title: title.to_owned(),
                subtitle: None,
                kind,
                items,
            };
            Ok(HomeFeed {
                chips,
                shelves: vec![
                    mk_shelf(
                        "Listen again",
                        ShelfKind::Cards,
                        (4..10).map(|n| FeedItem::Song(track(n))).collect(),
                    ),
                    mk_shelf(
                        "Mixes",
                        ShelfKind::Cards,
                        (0..PLAYLISTS)
                            .map(|p| FeedItem::Playlist(playlist_summary(p)))
                            .collect(),
                    ),
                    mk_shelf(
                        "Videos",
                        ShelfKind::Cards,
                        (10..16).map(|n| FeedItem::Video(track(n))).collect(),
                    ),
                    mk_shelf(
                        "Podcasts",
                        ShelfKind::Cards,
                        (0..3)
                            .map(|n| {
                                FeedItem::Podcast(PodcastSummary {
                                    id: browse("podcast", n),
                                    title: format!("{} Hour", ALBUM_TITLES[n]),
                                    author: Some(ARTIST_NAMES[n].to_owned()),
                                    thumbnails: thumbs(n + 5),
                                })
                            })
                            .collect(),
                    ),
                ],
            })
        })
    }

    fn library_playlists(&self) -> BoxFuture<'_, Result<Vec<PlaylistSummary>, CatalogError>> {
        Box::pin(async { Ok((0..PLAYLISTS).map(playlist_summary).collect()) })
    }

    fn liked_songs(&self) -> BoxFuture<'_, Result<Vec<Track>, CatalogError>> {
        Box::pin(async { Ok((0..40).filter(|n| n % 3 != 1).map(track).collect()) })
    }

    fn library_albums(&self) -> BoxFuture<'_, Result<Vec<AlbumSummary>, CatalogError>> {
        Box::pin(async { Ok((0..5).map(album_summary).collect()) })
    }

    fn library_artists(&self) -> BoxFuture<'_, Result<Vec<ArtistSummary>, CatalogError>> {
        Box::pin(async { Ok((0..4).map(artist_summary).collect()) })
    }

    fn history(&self) -> BoxFuture<'_, Result<Vec<HistorySection>, CatalogError>> {
        Box::pin(async {
            Ok(vec![
                HistorySection {
                    title: "Today".to_owned(),
                    tracks: (2..7).map(track).collect(),
                },
                HistorySection {
                    title: "Yesterday".to_owned(),
                    tracks: (7..13).map(track).collect(),
                },
            ])
        })
    }

    fn explore(&self) -> BoxFuture<'_, Result<Explore, CatalogError>> {
        Box::pin(async {
            Ok(Explore {
                new_releases: (0..8).map(album_summary).collect(),
                charts: Vec::new(),
                shelves: Vec::new(),
                moods: ["Chill", "Gym", "Commute", "Romance"]
                    .iter()
                    .enumerate()
                    .map(|(i, t)| MoodChip {
                        title: (*t).to_owned(),
                        id: browse("mood", i + 10),
                    })
                    .collect(),
            })
        })
    }

    fn mood_playlists<'a>(
        &'a self,
        _id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<Vec<FeedShelf>, CatalogError>> {
        Box::pin(async {
            Ok(vec![FeedShelf {
                title: "Playlists".to_owned(),
                subtitle: None,
                kind: ShelfKind::Cards,
                items: (0..PLAYLISTS)
                    .map(|p| FeedItem::Playlist(playlist_summary(p)))
                    .collect(),
            }])
        })
    }

    /// The mock accepts every like, so the UI flow can be driven without an account.
    fn rate_song<'a>(
        &'a self,
        _video_id: &'a nemo_core::track::VideoId,
        _liked: bool,
    ) -> BoxFuture<'a, Result<(), CatalogError>> {
        Box::pin(async { Ok(()) })
    }
}

/// Signal path of a mock track: YouTube AAC 129 kbps resampled to a 48 kHz device in shared mode
/// (what the dev machine reports), or local FLAC 96 kHz / 24-bit to an exclusive device.
#[must_use]
pub fn signal_path(track: &Track) -> SignalPath {
    match track.origin {
        Origin::YoutubeMusic => {
            let out = AudioFormat {
                sample_rate: 48_000,
                channels: 2,
                bit_depth: Some(32),
            };
            SignalPath {
                source: StreamInfo {
                    origin: Origin::YoutubeMusic,
                    codec: Codec::Aac,
                    bitrate_kbps: Some(129),
                    format: Some(AudioFormat {
                        sample_rate: 44_100,
                        channels: 2,
                        bit_depth: None,
                    }),
                },
                processing: vec![ProcessingStage::Resample {
                    from_hz: 44_100,
                    to_hz: 48_000,
                }],
                output: OutputInfo {
                    device_name: "Speakers (Realtek Audio)".to_owned(),
                    mode: OutputMode::Shared,
                    format: out,
                },
            }
        }
        Origin::Local => {
            let format = AudioFormat {
                sample_rate: 96_000,
                channels: 2,
                bit_depth: Some(24),
            };
            SignalPath {
                source: StreamInfo {
                    origin: Origin::Local,
                    codec: Codec::Flac,
                    bitrate_kbps: None,
                    format: Some(format),
                },
                processing: Vec::new(),
                output: OutputInfo {
                    device_name: "Mock DAC".to_owned(),
                    mode: OutputMode::Exclusive,
                    format,
                },
            }
        }
    }
}

/// A catalog that behaves like the real merged one: no home feed, an empty query lists only the
/// local library, and any other query answers with a slice of the mock catalog (a different slice
/// per query), so the seed shelves on Home fill up.
#[derive(Debug, Default, Clone, Copy)]
pub struct SeededCatalog;

impl Catalog for SeededCatalog {
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchResults, CatalogError>> {
        let q = query.trim();
        let all = tracks();
        let result = if q.is_empty() {
            SearchResults {
                songs: all
                    .into_iter()
                    .filter(|t| t.origin == Origin::Local)
                    .collect(),
                albums: (0..LOCAL_ALBUMS).map(local_album_summary).collect(),
                ..SearchResults::default()
            }
        } else {
            let k = q.bytes().map(usize::from).sum::<usize>() % 4;
            SearchResults {
                songs: all.into_iter().skip(k * 7).take(8).collect(),
                albums: (0..TRACKS / PER_ALBUM)
                    .skip(k)
                    .take(5)
                    .map(album_summary)
                    .collect(),
                artists: (0..ARTISTS).skip(k).take(4).map(artist_summary).collect(),
                playlists: (0..PLAYLISTS)
                    .skip(k)
                    .take(3)
                    .map(playlist_summary)
                    .collect(),
            }
        };
        Box::pin(async move { Ok(result) })
    }

    fn album<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<AlbumDetail, CatalogError>> {
        MockCatalog.album(id)
    }

    fn artist<'a>(&'a self, id: &'a BrowseId) -> BoxFuture<'a, Result<ArtistDetail, CatalogError>> {
        MockCatalog.artist(id)
    }

    fn playlist<'a>(
        &'a self,
        id: &'a BrowseId,
    ) -> BoxFuture<'a, Result<PlaylistDetail, CatalogError>> {
        MockCatalog.playlist(id)
    }
}

/// Like [`mock_backend`], but the catalog behaves like the real one (see [`SeededCatalog`]).
#[must_use]
pub fn mock_backend_seeded() -> Backend {
    Backend {
        catalog: Arc::new(SeededCatalog),
        ..mock_backend()
    }
}
