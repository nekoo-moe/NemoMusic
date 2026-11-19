//! The start-up budget with a fake resolver: no network.

use std::sync::Mutex;
use std::time::SystemTime;

use nemo_core::BoxFuture;
use nemo_core::audio::Origin;
use nemo_core::catalog::{FeedShelf, ShelfKind};
use nemo_core::stream::{ResolvedStream, StreamError};
use nemo_core::track::{Artwork, Track, TrackId};

use super::*;

#[derive(Default)]
struct Hints(Mutex<Vec<String>>);

impl StreamResolver for Hints {
    fn resolve<'a>(
        &'a self,
        _video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        Box::pin(async { Err(StreamError::NotAvailable) })
    }
    fn warm(&self, video: &VideoId) {
        self.0.lock().expect("lock").push(video.as_str().to_owned());
    }
}

fn track(id: &str) -> Track {
    Track {
        id: TrackId(id.to_owned()),
        title: "t".to_owned(),
        artists: Vec::new(),
        album: None,
        duration: None,
        origin: Origin::YoutubeMusic,
        video_id: VideoId::parse(id).ok(),
        local_path: None,
        artwork: Artwork::None,
        source_url: None,
    }
}

fn feed(ids: &[&str]) -> HomeFeed {
    let (first, rest) = ids.split_at(ids.len().min(2));
    let shelf = |items: &[&str]| FeedShelf {
        title: "s".to_owned(),
        subtitle: None,
        kind: ShelfKind::default(),
        items: items.iter().map(|id| FeedItem::Song(track(id))).collect(),
    };
    HomeFeed {
        chips: Vec::new(),
        shelves: vec![shelf(first), shelf(rest)],
    }
}

#[test]
fn only_the_first_songs_in_the_order_shown_are_taken_and_never_more_than_the_limit() {
    let ids = [
        "aaaaaaaaaaa",
        "bbbbbbbbbbb",
        "ccccccccccc",
        "ddddddddddd",
        "eeeeeeeeeee",
    ];
    let found = home_videos(&feed(&ids), 10);
    assert_eq!(found.len(), 5);
    let resolver = Arc::new(Hints::default());
    let warm = StartupWarm::new(resolver.clone());
    warm.hint_home(&feed(&ids));
    assert_eq!(
        *resolver.0.lock().expect("lock"),
        ["aaaaaaaaaaa", "bbbbbbbbbbb", "ccccccccccc"]
    );
    warm.hint_home(&feed(&ids));
    warm.hint([VideoId::parse("fffffffffff").expect("valid")]);
    assert_eq!(
        resolver.0.lock().expect("lock").len(),
        LIMIT,
        "the budget is per run"
    );
}

#[test]
fn the_last_played_song_counts_against_the_same_budget() {
    let resolver = Arc::new(Hints::default());
    let warm = StartupWarm::new(resolver.clone());
    warm.hint([VideoId::parse("zzzzzzzzzzz").expect("valid")]);
    warm.hint_home(&feed(&["aaaaaaaaaaa", "bbbbbbbbbbb", "ccccccccccc"]));
    assert_eq!(
        *resolver.0.lock().expect("lock"),
        ["zzzzzzzzzzz", "aaaaaaaaaaa", "bbbbbbbbbbb"]
    );
}

#[test]
fn albums_and_other_cards_are_not_hinted() {
    let mut mixed = feed(&[]);
    mixed.shelves.push(FeedShelf {
        title: "s".to_owned(),
        subtitle: None,
        kind: ShelfKind::default(),
        items: Vec::new(),
    });
    assert!(home_videos(&mixed, 3).is_empty());
}

#[test]
fn a_played_youtube_song_is_recorded_and_a_local_file_is_not() {
    let dir = std::env::temp_dir().join(format!("nemo-last-played-{}", std::process::id()));
    let store = Arc::new(StreamStore::open(dir.join("streams.json")));
    let observer = LastPlayed(Arc::clone(&store));
    let started = |track| PlayHookEvent::TrackStarted {
        track,
        started_at: SystemTime::now(),
    };
    let mut local = track("lllllllllll");
    local.local_path = Some(std::path::PathBuf::from("song.flac"));
    observer.on_event(&started(local));
    assert_eq!(store.last_played(), None);
    observer.on_event(&started(track("abcdefghijk")));
    assert_eq!(store.last_played(), VideoId::parse("abcdefghijk").ok());
    let _ = std::fs::remove_dir_all(&dir); // Cleanup only.
}
