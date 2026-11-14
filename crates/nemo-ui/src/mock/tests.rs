//! Unit tests of the mock backend.

use super::*;
use nemo_core::account::{AccountError, AccountService, AccountState};
use nemo_core::catalog::{Catalog, CatalogError};
use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent};
use nemo_core::secret::Secret;
use nemo_core::services::{EventReceiver, ImageError, ImageSource, PlayerControl};
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

/// Run a future that is already ready (every mock future is).
fn ready<T>(fut: impl Future<Output = T>) -> T {
    let mut fut = pin!(fut);
    let mut cx = Context::from_waker(Waker::noop());
    match fut.as_mut().poll(&mut cx) {
        Poll::Ready(v) => v,
        Poll::Pending => unreachable!("mock futures complete at once"),
    }
}

fn drain(rx: &mut dyn EventReceiver) -> Vec<PlayerEvent> {
    std::iter::from_fn(|| rx.try_next()).collect()
}

#[test]
fn catalog_has_forty_tracks_ten_albums_eight_artists() {
    let r = ready(MockCatalog.search("")).expect("ok");
    assert_eq!(r.songs.len(), 40);
    assert_eq!(r.albums.len(), 10 + LOCAL_ALBUMS);
    assert_eq!(r.artists.len(), 8);
    assert_eq!(r.playlists.len(), 6);
    let ids: std::collections::HashSet<_> = r.songs.iter().map(|t| t.id.0.clone()).collect();
    assert_eq!(ids.len(), 40, "unique ids");
}

#[test]
fn search_filters_by_title_and_artist() {
    let r = ready(MockCatalog.search("velvet harbor")).expect("ok");
    assert!(!r.songs.is_empty() && r.songs.iter().all(|t| t.artists[0] == "Velvet Harbor"));
    assert_eq!(r.artists.len(), 1);
    assert_eq!(
        ready(MockCatalog.search("zzzz-nothing")).expect("ok").songs,
        vec![]
    );
}

#[test]
fn details_resolve_and_unknown_ids_are_not_found() {
    let album = ready(MockCatalog.album(&browse("album", 2))).expect("album");
    assert_eq!(album.tracks.len(), 4);
    let artist = ready(MockCatalog.artist(&browse("artist", 1))).expect("artist");
    assert!(!artist.albums.is_empty() && !artist.top_songs.is_empty());
    assert_eq!(
        ready(MockCatalog.playlist(&browse("playlist", 0)))
            .expect("pl")
            .tracks
            .len(),
        12
    );
    assert_eq!(
        ready(MockCatalog.album(&BrowseId("nope".into()))),
        Err(CatalogError::NotFound)
    );
    assert_eq!(
        ready(MockCatalog.album(&browse("album", 99))),
        Err(CatalogError::NotFound)
    );
    assert!(ready(MockCatalog.home()).expect("home").len() >= 2);
}

#[test]
fn covers_decode_and_differ() {
    let a = cover_png(0, 128);
    let b = cover_png(1, 128);
    assert_ne!(a, b);
    let img = image::load_from_memory(&a).expect("png");
    assert_eq!((img.width(), img.height()), (128, 128));
}

#[test]
fn image_source_returns_png_and_reports_missing_art() {
    let images = MockImages::default();
    let t = track(5);
    let bytes = ready(images.image(&t.artwork, 200)).expect("cover");
    assert!(image::load_from_memory(&bytes).is_ok());
    assert_eq!(
        ready(images.image(&Artwork::None, 200)),
        Err(ImageError::Missing)
    );
    assert!(matches!(
        ready(images.image(&Artwork::LocalFile("x".into()), 200)),
        Err(ImageError::Read(_))
    ));
}

#[test]
fn tracks_show_honest_signal_paths() {
    let yt = track(0);
    assert_eq!(yt.origin, Origin::YoutubeMusic);
    assert_eq!(signal_path(&yt).source.badge(), "AAC 129\u{2009}kbps");
    let local = track(3);
    assert_eq!(local.origin, Origin::Local);
    let p = signal_path(&local);
    assert_eq!(p.source.badge(), "FLAC 96\u{2009}kHz / 24-bit");
    assert!(p.is_bit_perfect());
    assert!(!signal_path(&yt).is_bit_perfect());
}

#[test]
fn player_reacts_to_every_command() {
    let p = MockPlayer::new();
    assert_eq!(p.snapshot().state, PlayState::Paused);
    assert_eq!(p.position(), Duration::ZERO);

    p.send(PlayerCommand::Resume);
    assert_eq!(p.snapshot().state, PlayState::Playing);
    p.send(PlayerCommand::Pause);
    assert_eq!(p.snapshot().state, PlayState::Paused);

    p.send(PlayerCommand::Seek(Duration::from_secs(42)));
    assert_eq!(p.position(), Duration::from_secs(42));
    p.send(PlayerCommand::Seek(Duration::from_secs(99_999)));
    assert_eq!(
        Some(p.position()),
        p.snapshot().duration,
        "seek clamps to the end"
    );

    p.send(PlayerCommand::Next);
    assert_eq!(p.snapshot().queue_index, Some(1));
    assert_eq!(p.snapshot().state, PlayState::Playing);
    p.send(PlayerCommand::Previous);
    assert_eq!(
        p.snapshot().queue_index,
        Some(0),
        "early in a track, previous goes back"
    );
}

#[test]
fn previous_goes_back_early_in_a_track_and_restarts_late() {
    let p = MockPlayer::new();
    p.send(PlayerCommand::Next);
    p.send(PlayerCommand::Pause);
    assert_eq!(p.snapshot().queue_index, Some(1));
    p.send(PlayerCommand::Previous);
    assert_eq!(p.snapshot().queue_index, Some(0));
    p.send(PlayerCommand::Seek(Duration::from_secs(30)));
    p.send(PlayerCommand::Previous);
    assert_eq!(p.snapshot().queue_index, Some(0));
    assert!(p.position() < Duration::from_secs(1));
}

#[test]
fn queue_commands_volume_and_stop() {
    let p = MockPlayer::new();
    let before = p.snapshot().queue.len();
    p.send(PlayerCommand::Enqueue(track(30)));
    p.send(PlayerCommand::PlayNext(track(31)));
    let s = p.snapshot();
    assert_eq!(s.queue.len(), before + 2);
    assert_eq!(s.queue[1].id, track(31).id);
    p.send(PlayerCommand::RemoveAt(1));
    assert_eq!(p.snapshot().queue.len(), before + 1);
    p.send(PlayerCommand::SetVolume(250));
    assert_eq!(p.snapshot().volume_percent, 100);
    p.send(PlayerCommand::SetSkipSegments(true));
    assert!(p.snapshot().skip_segments);
    p.send(PlayerCommand::SetQueue {
        tracks: vec![track(3), track(4)],
        start: 1,
    });
    assert_eq!(
        p.snapshot().current.as_ref().map(|t| t.id.clone()),
        Some(track(4).id)
    );
    p.send(PlayerCommand::Play(track(3)));
    let s = p.snapshot();
    assert_eq!(s.queue.len(), 1);
    assert!(s.signal_path.expect("path").is_bit_perfect());
    p.send(PlayerCommand::Stop);
    let s = p.snapshot();
    assert_eq!(
        (s.state, s.current, s.queue_index),
        (PlayState::Stopped, None, None)
    );
    assert_eq!(p.position(), Duration::ZERO);
    p.send(PlayerCommand::UndoSkip);
    p.send(PlayerCommand::Resume);
    assert_eq!(
        p.snapshot().state,
        PlayState::Playing,
        "resume restarts a stopped queue"
    );
}

#[test]
fn removing_the_current_track_plays_the_next_one() {
    let p = MockPlayer::new();
    p.send(PlayerCommand::RemoveAt(0));
    assert_eq!(p.snapshot().current.map(|t| t.id), Some(track(1).id));
    assert_eq!(p.snapshot().queue_index, Some(0));
}

#[test]
fn clock_advances_only_while_playing() {
    let p = MockPlayer::new();
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(p.position(), Duration::ZERO, "paused clock stands still");
    p.send(PlayerCommand::Resume);
    std::thread::sleep(Duration::from_millis(60));
    let a = p.position();
    assert!(a >= Duration::from_millis(50), "{a:?}");
    p.send(PlayerCommand::Pause);
    let frozen = p.position();
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(p.position(), frozen);
}

#[test]
fn a_finished_track_moves_to_the_next() {
    let p = MockPlayer::new();
    p.send(PlayerCommand::Resume);
    let d = p.snapshot().duration.expect("duration");
    p.send(PlayerCommand::Seek(
        d.saturating_sub(Duration::from_millis(20)),
    ));
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(p.snapshot().queue_index, Some(1));
}

#[test]
fn events_follow_commands_and_each_receiver_gets_its_own_copy() {
    let p = MockPlayer::new();
    let mut a = p.subscribe();
    let mut b = p.subscribe();
    p.send(PlayerCommand::Resume);
    p.send(PlayerCommand::SetVolume(30));
    let ea = drain(a.as_mut());
    assert!(ea.contains(&PlayerEvent::StateChanged(PlayState::Playing)));
    assert!(ea.contains(&PlayerEvent::VolumeChanged(30)));
    assert_eq!(ea, drain(b.as_mut()));
    assert!(drain(a.as_mut()).is_empty(), "drained");
    p.send(PlayerCommand::Next);
    let e = drain(a.as_mut());
    assert!(e.iter().any(|x| matches!(x, PlayerEvent::TrackChanged(_))));
    assert!(
        e.iter()
            .any(|x| matches!(x, PlayerEvent::SignalPathChanged(_)))
    );
    assert!(e.iter().any(|x| matches!(x, PlayerEvent::DurationKnown(_))));
}

#[test]
fn mock_backend_is_wired() {
    let b = mock_backend();
    assert_eq!(b.player.snapshot().queue.len(), 12);
    assert!(ready(b.catalog.search("")).is_ok());
}

/// Poll a future until it is ready, sleeping between polls (the mock account uses a timer).
fn wait<T>(fut: impl Future<Output = T>) -> T {
    let mut fut = pin!(fut);
    let mut cx = Context::from_waker(Waker::noop());
    for _ in 0..500 {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the mock account never answered");
}

#[test]
fn the_mock_account_signs_in_and_out() {
    let a = MockAccount::with_delay(Duration::from_millis(20));
    assert_eq!(a.state(), AccountState::SignedOut);
    let info = wait(a.sign_in()).expect("signs in");
    assert_eq!(info.name, "Mira Okonkwo");
    assert!(matches!(a.state(), AccountState::SignedIn(_)));
    let report = wait(a.sign_out()).expect("signs out");
    assert_eq!(report.credentials_removed, 1);
    assert_eq!(a.state(), AccountState::SignedOut);
}

#[test]
fn the_mock_account_rejects_an_empty_cookie_and_never_prints_one() {
    let a = MockAccount::with_delay(Duration::from_millis(10));
    let empty = Secret::new(String::new());
    assert_eq!(
        wait(a.sign_in_with_cookie(empty)),
        Err(AccountError::Rejected)
    );
    assert_eq!(a.state(), AccountState::SignedOut);
    let cookie = Secret::new("SID=abc".to_owned());
    assert_eq!(format!("{cookie:?}"), "[redacted]");
    assert!(wait(a.sign_in_with_cookie(cookie)).is_ok());
}

#[test]
fn two_tracks_carry_vietnamese_text() {
    assert_eq!(
        track(1).title,
        "M\u{1edb}i Quen Nhau \u{110}\u{e3} R\u{1ee7}"
    );
    assert_eq!(
        track(1).artists.join(", "),
        "HIEUTHUHAI, Ph\u{1b0}\u{1a1}ng Ly"
    );
}
