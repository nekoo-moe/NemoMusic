//! The Explore, charts and mood readers against recorded answers of the real pages
//! (`tests/fixtures/music/README.md`).

use nemo_core::catalog::FeedItem;

use crate::feeds::item::split_mood_id;
use crate::feeds::shelf::{parse_charts, parse_explore, parse_mood_page};

const EXPLORE: &str = include_str!("../../../tests/fixtures/music/browse-explore.json");
const CHARTS: &str = include_str!("../../../tests/fixtures/music/browse-charts.json");
const MOOD: &str = include_str!("../../../tests/fixtures/music/browse-mood.json");

#[test]
fn the_real_explore_page_gives_new_releases_moods_and_new_music_videos() {
    let e = parse_explore(EXPLORE);
    assert_eq!(e.new_releases.len(), 3, "New albums & singles");
    assert!(e.new_releases.iter().all(|a| a.id.0.starts_with("MPRE")));
    assert!(e.new_releases.iter().all(|a| !a.thumbnails.is_empty()));
    let moods: Vec<&str> = e.moods.iter().map(|m| m.title.as_str()).collect();
    assert_eq!(
        moods,
        [
            "Chill",
            "Commute",
            "Energize",
            "Feel good",
            "Focus",
            "Gaming"
        ]
    );
    for m in &e.moods {
        let (page, params) = split_mood_id(&m.id).expect("a mood id");
        assert_eq!(page, "FEmusic_moods_and_genres_category");
        assert!(params.is_some_and(|p| !p.is_empty()), "{}", m.title);
    }
    let titles: Vec<&str> = e.shelves.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["Trending", "New music videos"]);
    assert!(
        e.shelves[1]
            .items
            .iter()
            .all(|i| matches!(i, FeedItem::Video(_)))
    );
    assert!(!e.shelves[0].items.is_empty());
    assert!(e.charts.is_empty(), "charts come from their own page");
}

#[test]
fn the_real_charts_page_gives_video_charts_and_top_artists() {
    let charts = parse_charts(CHARTS);
    let titles: Vec<&str> = charts.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["Video charts", "Top artists"]);
    assert!(
        charts[0]
            .items
            .iter()
            .all(|i| matches!(i, FeedItem::Playlist(p) if p.id.0.starts_with("VL")))
    );
    assert!(
        charts[1]
            .items
            .iter()
            .all(|i| matches!(i, FeedItem::Artist(a) if a.id.0.starts_with("UC")))
    );
}

#[test]
fn the_real_mood_page_gives_its_playlist_shelves() {
    let shelves = parse_mood_page(MOOD);
    let titles: Vec<&str> = shelves.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["Unwind + explore", "Mindfulness & meditation"]);
    assert!(
        shelves
            .iter()
            .flat_map(|s| &s.items)
            .all(|i| matches!(i, FeedItem::Playlist(_)))
    );
}
