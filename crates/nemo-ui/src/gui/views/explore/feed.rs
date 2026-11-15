//! Cards for the shelves of Explore and of mood pages.

use nemo_core::catalog::{AlbumSummary, FeedItem, FeedShelf, Shelf, ShelfItem};
use nemo_core::track::Artwork;

use crate::format;
use crate::gui::app::{ArtShape, DetailKey, DetailKind, ListSource, Message};
use crate::gui::views::kit::Card;
use crate::route::Route;
use crate::state::Event;

fn album_card(a: &AlbumSummary) -> Card {
    Card {
        artwork: Artwork::Remote(a.thumbnails.clone()),
        title: a.title.clone(),
        subtitle: format::artists(&a.artists),
        shape: ArtShape::Tile,
        on_press: Message::Ui(Event::Go(Route::Album(a.id.clone()))),
        play: Some(Message::PlayCollection(DetailKey {
            kind: DetailKind::Album,
            id: a.id.clone(),
        })),
    }
}

/// Cards for new releases.
pub(super) fn album_cards(albums: &[AlbumSummary]) -> Vec<Card> {
    albums.iter().map(album_card).collect()
}

/// Cards for a shelf of the catalog. Songs and music videos play from `source` (the songs and
/// videos of this shelf, in order); podcasts, episodes and mood tiles are left out.
pub(super) fn feed_cards(shelf: &FeedShelf, source: &ListSource) -> Vec<Card> {
    let mut out = Vec::new();
    let mut track_index = 0;
    for item in &shelf.items {
        match item {
            FeedItem::Song(t) | FeedItem::Video(t) => {
                let play = Message::PlayAt {
                    source: source.clone(),
                    index: track_index,
                };
                track_index += 1;
                out.push(Card {
                    artwork: t.artwork.clone(),
                    title: t.title.clone(),
                    subtitle: format::artists(&t.artists),
                    shape: ArtShape::Tile,
                    on_press: play.clone(),
                    play: Some(play),
                });
            }
            FeedItem::Album(a) => out.push(album_card(a)),
            FeedItem::Playlist(p) => out.push(Card {
                artwork: Artwork::Remote(p.thumbnails.clone()),
                title: p.title.clone(),
                subtitle: p.author.clone().unwrap_or_else(|| "Playlist".to_owned()),
                shape: ArtShape::Tile,
                on_press: Message::Ui(Event::Go(Route::Playlist(p.id.clone()))),
                play: Some(Message::PlayCollection(DetailKey {
                    kind: DetailKind::Playlist,
                    id: p.id.clone(),
                })),
            }),
            FeedItem::Artist(a) => out.push(Card {
                artwork: Artwork::Remote(a.thumbnails.clone()),
                title: a.name.clone(),
                subtitle: "Artist".to_owned(),
                shape: ArtShape::Circle,
                on_press: Message::Ui(Event::Go(Route::Artist(a.id.clone()))),
                play: None,
            }),
            FeedItem::Podcast(_) | FeedItem::Episode(_) | FeedItem::Mood(_) => {}
        }
    }
    out
}

/// Cards for the albums, playlists and artists of a Home shelf (the fallback; songs are skipped:
/// a song list needs a source to play from).
pub(super) fn collection_cards(shelf: &Shelf) -> Vec<Card> {
    let mut out = Vec::new();
    for item in &shelf.items {
        match item {
            ShelfItem::Song(_) => {}
            ShelfItem::Album(a) => out.push(album_card(a)),
            ShelfItem::Playlist(p) => out.push(Card {
                artwork: Artwork::Remote(p.thumbnails.clone()),
                title: p.title.clone(),
                subtitle: p.author.clone().unwrap_or_else(|| "Playlist".to_owned()),
                shape: ArtShape::Tile,
                on_press: Message::Ui(Event::Go(Route::Playlist(p.id.clone()))),
                play: Some(Message::PlayCollection(DetailKey {
                    kind: DetailKind::Playlist,
                    id: p.id.clone(),
                })),
            }),
            ShelfItem::Artist(a) => out.push(Card {
                artwork: Artwork::Remote(a.thumbnails.clone()),
                title: a.name.clone(),
                subtitle: "Artist".to_owned(),
                shape: ArtShape::Circle,
                on_press: Message::Ui(Event::Go(Route::Artist(a.id.clone()))),
                play: None,
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemo_core::catalog::{BrowseId, MoodChip, PlaylistSummary, ShelfKind};

    #[test]
    fn explore_cards_play_songs_and_videos_by_their_place_in_the_shelf() {
        let shelf = FeedShelf {
            title: "New music videos".to_owned(),
            subtitle: None,
            kind: ShelfKind::Cards,
            items: vec![
                FeedItem::Video(crate::mock::track(1)),
                FeedItem::Playlist(PlaylistSummary {
                    id: BrowseId("VLPL1".to_owned()),
                    title: "Chart".to_owned(),
                    author: None,
                    thumbnails: vec![],
                }),
                FeedItem::Mood(MoodChip {
                    title: "Chill".to_owned(),
                    id: BrowseId("m".to_owned()),
                }),
                FeedItem::Song(crate::mock::track(2)),
            ],
        };
        let cards = feed_cards(&shelf, &ListSource::Explore(3));
        assert_eq!(cards.len(), 3, "the mood tile is not a card");
        let indexes: Vec<Option<usize>> = cards
            .iter()
            .map(|c| match &c.play {
                Some(Message::PlayAt {
                    source: ListSource::Explore(3),
                    index,
                }) => Some(*index),
                _ => None,
            })
            .collect();
        assert_eq!(indexes, [Some(0), None, Some(1)]);
        assert!(matches!(
            &cards[1].on_press,
            Message::Ui(Event::Go(Route::Playlist(_)))
        ));
    }
}
