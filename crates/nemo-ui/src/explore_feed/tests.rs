//! Tests of the Explore page model.

use super::*;
use nemo_core::catalog::{AlbumSummary, MoodChip, ShelfKind};

fn album(n: usize) -> AlbumSummary {
    AlbumSummary {
        id: BrowseId(format!("MPREb{n}")),
        title: format!("Album {n}"),
        artists: vec!["Artist".to_owned()],
        year: None,
        thumbnails: vec![],
    }
}

fn mood(title: &str) -> MoodChip {
    MoodChip {
        title: title.to_owned(),
        id: BrowseId(format!("FEmusic_moods_and_genres_category|{title}")),
    }
}

fn shelf(title: &str, items: Vec<FeedItem>) -> FeedShelf {
    FeedShelf {
        title: title.to_owned(),
        subtitle: None,
        kind: ShelfKind::Cards,
        items,
    }
}

fn answer() -> Arc<Explore> {
    Arc::new(Explore {
        new_releases: vec![album(1), album(2)],
        charts: vec![shelf("Video charts", vec![FeedItem::Album(album(3))])],
        shelves: vec![shelf(
            "New music videos",
            vec![
                FeedItem::Video(crate::mock::track(1)),
                FeedItem::Album(album(4)),
                FeedItem::Song(crate::mock::track(2)),
            ],
        )],
        moods: vec![mood("Chill"), mood("Focus")],
    })
}

fn me() -> Owner {
    Owner::SignedIn {
        name: "Mira".to_owned(),
        handle: None,
    }
}

#[test]
fn explore_is_asked_once_per_owner_and_again_after_a_sign_in() {
    let mut f = ExploreFeed::default();
    assert!(f.wants(&Owner::SignedOut));
    assert!(!f.wants(&Owner::Pending), "a pending sign-in waits");
    f.start(Owner::SignedOut);
    assert!(!f.wants(&Owner::SignedOut), "one request at a time");
    f.finish(Ok(answer()));
    assert!(!f.wants(&Owner::SignedOut));
    assert!(f.wants(&me()), "the signed-in session asks again");
}

#[test]
fn explore_shows_the_answer_with_charts_before_the_other_shelves() {
    let mut f = ExploreFeed::default();
    f.start(Owner::SignedOut);
    f.finish(Ok(answer()));
    assert_eq!(f.phase(), ExplorePhase::Ready);
    assert!(!f.shows_fallback());
    assert_eq!(f.notice(), None);
    let titles: Vec<&str> = f.shelves().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["Video charts", "New music videos"]);
    assert_eq!(
        f.shelf(1).map(|s| s.title.as_str()),
        Some("New music videos")
    );
    assert!(f.shelf(2).is_none());
}

#[test]
fn a_failed_reload_keeps_the_old_explore_with_one_short_notice() {
    let mut f = ExploreFeed::default();
    f.start(Owner::SignedOut);
    f.finish(Ok(answer()));
    f.reload();
    assert!(f.wants(&Owner::SignedOut));
    f.start(Owner::SignedOut);
    f.finish(Err("network error: offline".to_owned()));
    assert_eq!(f.phase(), ExplorePhase::Failed);
    assert_eq!(f.data().map(|e| e.new_releases.len()), Some(2));
    assert!(!f.shows_fallback(), "old data stays, no seed shelves");
    assert_eq!(f.notice(), Some(STALE_NOTICE));
    assert!(!f.wants(&Owner::SignedOut), "no retry loop");
}

#[test]
fn seed_shelves_show_only_when_explore_never_loaded() {
    let mut f = ExploreFeed::default();
    assert!(!f.shows_fallback(), "not while idle");
    f.start(Owner::SignedOut);
    assert!(!f.shows_fallback(), "not while loading");
    f.finish(Err("network error: offline".to_owned()));
    assert!(f.shows_fallback());
    assert_eq!(f.notice(), Some(FALLBACK_NOTICE));
    assert_eq!(f.error(), Some("network error: offline"));
}

#[test]
fn an_empty_explore_answer_counts_as_a_failure() {
    let mut f = ExploreFeed::default();
    f.start(Owner::SignedOut);
    let empty = Explore {
        charts: vec![shelf("Charts", vec![])],
        ..Explore::default()
    };
    f.finish(Ok(Arc::new(empty)));
    assert!(f.shows_fallback());
    assert_eq!(f.error(), Some(EMPTY_ANSWER));
}

#[test]
fn reload_waits_for_the_running_request() {
    let mut f = ExploreFeed::default();
    f.start(Owner::SignedOut);
    f.reload();
    assert!(!f.wants(&Owner::SignedOut));
}

#[test]
fn explore_fills_new_releases_and_moods_of_the_daily_inputs() {
    let mut daily = DailyData {
        moods: vec![mood("Home chip")],
        ..DailyData::default()
    };
    let mut f = ExploreFeed::default();
    f.fill_daily(&mut daily);
    assert_eq!(daily.moods.len(), 1, "nothing loaded: unchanged");
    f.start(Owner::SignedOut);
    f.finish(Ok(answer()));
    f.fill_daily(&mut daily);
    assert_eq!(daily.new_releases.len(), 2);
    assert_eq!(daily.moods, vec![mood("Chill"), mood("Focus")]);
}

#[test]
fn shelf_tracks_keep_songs_and_videos_in_order() {
    let a = answer();
    let tracks = shelf_tracks(&a.shelves[0]);
    assert_eq!(
        tracks.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
        vec![crate::mock::track(1).id, crate::mock::track(2).id]
    );
}

#[test]
fn mood_pages_load_once_and_retry_after_a_failure() {
    let id = mood("Chill").id;
    let mut m = MoodPages::default();
    assert!(m.wants(&id));
    m.start(id.clone());
    assert!(!m.wants(&id));
    assert!(matches!(m.get(&id), Some(MoodPage::Loading)));
    m.finish(&id, Err("network error".to_owned()));
    assert!(matches!(m.get(&id), Some(MoodPage::Failed(_))));
    m.forget(&id);
    assert!(m.wants(&id));
    m.start(id.clone());
    let shelves = Arc::new(vec![shelf("Playlists", vec![FeedItem::Album(album(1))])]);
    m.finish(&id, Ok(shelves));
    assert_eq!(m.shelf(&id, 0).map(|s| s.title.as_str()), Some("Playlists"));
}

#[test]
fn an_empty_mood_page_counts_as_a_failure() {
    let id = mood("Chill").id;
    let mut m = MoodPages::default();
    m.start(id.clone());
    m.finish(&id, Ok(Arc::new(vec![shelf("Empty", vec![])])));
    assert!(matches!(m.get(&id), Some(MoodPage::Failed(_))));
}

#[test]
fn mood_pages_keep_at_most_the_newest_sixteen() {
    let mut m = MoodPages::default();
    let ids: Vec<BrowseId> = (0..=MOOD_PAGES_MAX)
        .map(|i| BrowseId(format!("m{i}")))
        .collect();
    for id in &ids {
        m.start(id.clone());
    }
    assert!(m.wants(&ids[0]), "the oldest was dropped");
    assert!(!m.wants(&ids[MOOD_PAGES_MAX]));
    m.finish(&ids[0], Err("late".to_owned()));
    assert!(
        m.get(&ids[0]).is_none(),
        "a late answer for a dropped page is ignored"
    );
}
