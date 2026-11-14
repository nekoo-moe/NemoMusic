//! Tests for offline mode: the offline catalog and the request gates (criteria 4, 5 and 6).

use std::io::{Cursor, Read, Seek, SeekFrom, Write as _};
use std::net::TcpStream;
use std::sync::atomic::AtomicUsize;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::audio::{Codec, Origin, StreamInfo};
use nemo_core::catalog::{BrowseId, Catalog, CatalogError};
use nemo_core::downloads::NetworkMode;
use nemo_core::segment::{Segment, SegmentError, SegmentSource};
use nemo_core::services::{ByteSource, ImageError, ImageSource, StreamOpener};
use nemo_core::stream::{ResolvedStream, StreamError, StreamRequest, StreamResolver};
use nemo_core::track::{Artwork, Thumbnail, Track, TrackId};

use super::*;
use crate::offline::catalog::SongMeta;
use crate::test_server::{Response, TestServer};

fn ready<T>(mut future: BoxFuture<'_, T>) -> T {
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("every fake here answers at once"),
    }
}

/// Send one plain GET to the loopback fake host and read the answer.
fn knock(port: u16) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .write_all(b"GET /fake HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .expect("write");
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).expect("read");
}

fn stream_info() -> StreamInfo {
    StreamInfo {
        origin: Origin::YoutubeMusic,
        codec: Codec::Opus,
        bitrate_kbps: Some(128),
        format: None,
    }
}

fn resolved() -> ResolvedStream {
    ResolvedStream {
        url: nemo_core::secret::Secret::new("http://127.0.0.1/fake".to_owned()),
        info: stream_info(),
        expires_at: None,
    }
}

/// Every service of the fake host: each call sends the host one request.
struct Host(u16);

impl StreamResolver for Host {
    fn resolve<'a>(&'a self, _: &'a VideoId) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        knock(self.0);
        Box::pin(async { Ok(resolved()) })
    }
}

struct Bytes(Cursor<Vec<u8>>);

impl Read for Bytes {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl Seek for Bytes {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

impl ByteSource for Bytes {
    fn byte_len(&self) -> Option<u64> {
        Some(self.0.get_ref().len() as u64)
    }
}

impl StreamOpener for Host {
    fn open(&self, _: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        knock(self.0);
        Ok(Box::new(Bytes(Cursor::new(vec![0; 4]))))
    }
}

impl SegmentSource for Host {
    fn segments<'a>(
        &'a self,
        _: &'a VideoId,
        _: Duration,
    ) -> BoxFuture<'a, Result<Vec<Segment>, SegmentError>> {
        knock(self.0);
        Box::pin(async { Ok(Vec::new()) })
    }
}

impl ImageSource for Host {
    fn image<'a>(&'a self, _: &'a Artwork, _: u32) -> BoxFuture<'a, Result<Vec<u8>, ImageError>> {
        knock(self.0);
        Box::pin(async { Ok(vec![1]) })
    }
}

/// Services whose network is down: every call fails with a network error and is counted.
#[derive(Default)]
struct Down(AtomicUsize);

impl Down {
    fn calls(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }

    fn hit(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl StreamResolver for Down {
    fn resolve<'a>(&'a self, _: &'a VideoId) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        self.hit();
        Box::pin(async { Err(StreamError::Network("down".to_owned())) })
    }
}

impl StreamOpener for Down {
    fn open(&self, _: &ResolvedStream) -> Result<Box<dyn ByteSource>, StreamError> {
        self.hit();
        Err(StreamError::Network("down".to_owned()))
    }
}

impl SegmentSource for Down {
    fn segments<'a>(
        &'a self,
        _: &'a VideoId,
        _: Duration,
    ) -> BoxFuture<'a, Result<Vec<Segment>, SegmentError>> {
        self.hit();
        Box::pin(async { Err(SegmentError::Network("down".to_owned())) })
    }
}

impl ImageSource for Down {
    fn image<'a>(&'a self, _: &'a Artwork, _: u32) -> BoxFuture<'a, Result<Vec<u8>, ImageError>> {
        self.hit();
        Box::pin(async { Err(ImageError::Network("down".to_owned())) })
    }
}

fn track(video_id: &str, title: &str, album: &str) -> Track {
    Track {
        id: TrackId(video_id.to_owned()),
        title: title.to_owned(),
        artists: vec!["Synthetic Artist".to_owned()],
        album: Some(album.to_owned()),
        duration: Some(Duration::from_millis(181_000)),
        origin: Origin::YoutubeMusic,
        video_id: Some(VideoId::parse(video_id).expect("id")),
        local_path: None,
        artwork: Artwork::Remote(vec![Thumbnail {
            url: "http://127.0.0.1/cover".to_owned(),
            width: 120,
            height: 120,
        }]),
    }
}

fn save_song(d: &Dir, track: &Track, saved_at: i64) {
    let meta = serde_json::to_string(&SongMeta::from(track)).expect("meta");
    raw(d)
        .execute(
            "INSERT INTO song (video_id, file, bytes, codec, reason, meta, saved_at)
             VALUES (?1, ?2, 1, 'opus', 'user', ?3, ?4)",
            rusqlite::params![track.id.0, opaque_name().expect("name"), meta, saved_at],
        )
        .expect("insert song");
}

fn save_collection(d: &Dir, kind: &str, browse_id: &str, title: &str, videos: &[&str]) {
    let conn = raw(d);
    conn.execute(
        "INSERT INTO collection (kind, browse_id, title, saved_at) VALUES (?1, ?2, ?3, 1)",
        [kind, browse_id, title],
    )
    .expect("insert collection");
    let id = conn.last_insert_rowid();
    for (position, video_id) in (0_i64..).zip(videos) {
        conn.execute(
            "INSERT INTO collection_song (collection_id, video_id, position) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, video_id, position],
        )
        .expect("insert collection song");
    }
}

/// A cache with two saved songs, one album holding both (second first) and one playlist.
fn saved_cache(d: &Dir) -> (Arc<OfflineCache>, Track, Track) {
    let cache = Arc::new(OfflineCache::open(&d.0, clock(1)).expect("open"));
    let first = track("aaaaaaaaaa1", "First Light", "Synthetic Album");
    let second = track("bbbbbbbbbb2", "Second Wind", "Synthetic Album");
    save_song(d, &first, 1);
    save_song(d, &second, 2);
    save_collection(
        d,
        "album",
        "MPREb_synth",
        "Synthetic Album",
        &["bbbbbbbbbb2", "aaaaaaaaaa1"],
    );
    save_collection(d, "playlist", "PLsynth", "Evening Mix", &["aaaaaaaaaa1"]);
    (cache, first, second)
}

#[test]
fn a_new_switch_starts_online() {
    let switch = OfflineSwitch::new(clock(0));
    assert!(!switch.is_offline());
    assert!(!switch.take_notice());
}

#[test]
fn the_offline_catalog_lists_only_saved_songs_and_collections() {
    let d = dir("mode-catalog");
    let (cache, first, second) = saved_cache(&d);
    let catalog = OfflineCatalog::new(cache);

    let found = ready(catalog.search("wind")).expect("search");
    assert_eq!(found.songs, vec![second.clone()]);
    let by_album = ready(catalog.search("synthetic album")).expect("search");
    assert_eq!(by_album.songs, vec![first.clone(), second.clone()]);
    assert_eq!(by_album.albums.len(), 1);
    assert!(by_album.playlists.is_empty());

    let album = ready(catalog.album(&BrowseId("MPREb_synth".to_owned()))).expect("album");
    assert_eq!(album.summary.title, "Synthetic Album");
    assert_eq!(album.tracks, vec![second, first.clone()]);
    let playlist = ready(catalog.playlist(&BrowseId("PLsynth".to_owned()))).expect("playlist");
    assert_eq!(playlist.tracks, vec![first]);
    assert_eq!(playlist.edit, None);

    let albums = ready(catalog.library_albums()).expect("albums");
    assert_eq!(albums.len(), 1);
    let playlists = ready(catalog.library_playlists()).expect("playlists");
    assert_eq!(playlists[0].title, "Evening Mix");

    let missing = BrowseId("MPREb_other".to_owned());
    assert_eq!(ready(catalog.album(&missing)), Err(CatalogError::NotFound));
    assert!(matches!(
        ready(catalog.artist(&missing)),
        Err(CatalogError::Unavailable(_))
    ));
    assert!(matches!(
        ready(catalog.home_feed()),
        Err(CatalogError::Unavailable(_))
    ));
}

#[test]
fn a_song_row_that_does_not_parse_is_left_out() {
    let d = dir("mode-bad-row");
    let (cache, ..) = saved_cache(&d);
    raw(&d)
        .execute(
            "INSERT INTO song (video_id, file, bytes, codec, reason, meta, saved_at)
             VALUES ('cccccccccc3', ?1, 1, 'opus', 'user', 'not json', 3)",
            [opaque_name().expect("name")],
        )
        .expect("insert");
    let found = ready(OfflineCatalog::new(cache).search("")).expect("search");
    assert_eq!(found.songs.len(), 2);
}

#[test]
fn offline_mode_sends_no_request_while_saved_songs_browse_and_play() {
    let d = dir("mode-no-request");
    let (cache, first, _) = saved_cache(&d);
    let server = TestServer::start(|_, _| Response::new(200, b"ok".to_vec()));
    let port = server
        .url("")
        .rsplit(':')
        .next()
        .expect("port")
        .parse()
        .expect("port");
    let host = Arc::new(Host(port));
    let switch = Arc::new(OfflineSwitch::new(clock(1)));
    let resolver = GatedResolver::new(host.clone(), switch.clone());
    let opener = GatedOpener::new(host.clone(), switch.clone());
    let segments = GatedSegments::new(host.clone(), switch.clone());
    let images = GatedImages::new(host, switch.clone());
    switch.set_offline(true);

    let catalog = OfflineCatalog::new(cache);
    let album = ready(catalog.album(&BrowseId("MPREb_synth".to_owned()))).expect("album");
    assert_eq!(album.tracks.len(), 2);
    let video = first.video_id.clone().expect("id");
    resolver.warm(&video);
    assert!(matches!(
        ready(resolver.resolve(&video)),
        Err(StreamError::Network(_))
    ));
    let request = StreamRequest::default();
    assert!(ready(resolver.resolve_with(&video, &request)).is_err());
    assert!(opener.open(&resolved()).is_err());
    let length = Duration::from_secs(181);
    assert!(matches!(
        ready(segments.segments(&video, length)),
        Err(SegmentError::Network(_))
    ));
    assert!(matches!(
        ready(images.image(&first.artwork, 120)),
        Err(ImageError::Network(_))
    ));
    assert!(
        server.requests().is_empty(),
        "offline mode reached the host"
    );

    // The same gates pass requests through once the user switches offline mode off.
    switch.set_offline(false);
    assert!(ready(resolver.resolve(&video)).is_ok());
    assert!(opener.open(&resolved()).is_ok());
    assert!(ready(segments.segments(&video, length)).is_ok());
    assert!(ready(images.image(&first.artwork, 120)).is_ok());
    assert_eq!(server.requests().len(), 4);
}

#[test]
fn two_failures_within_the_window_switch_once_and_a_third_says_nothing() {
    let time = clock(0);
    let switch = Arc::new(OfflineSwitch::new(time.clone()));
    let down = Arc::new(Down::default());
    let resolver = GatedResolver::new(down.clone(), switch.clone());
    let video = VideoId::parse("aaaaaaaaaa1").expect("id");

    assert!(ready(resolver.resolve(&video)).is_err());
    time.0.store(31, Ordering::SeqCst);
    assert!(ready(resolver.resolve(&video)).is_err());
    assert!(!switch.is_offline(), "31 s apart is outside the window");
    time.0.store(61, Ordering::SeqCst);
    assert!(ready(resolver.resolve(&video)).is_err());
    assert!(switch.is_offline());
    assert!(switch.take_notice(), "told once");
    assert!(!switch.take_notice(), "not told twice");

    time.0.store(62, Ordering::SeqCst);
    assert!(ready(resolver.resolve(&video)).is_err());
    switch.record_failure();
    assert_eq!(down.calls(), 3, "the gate refused the fourth resolve");
    assert!(!switch.take_notice(), "a later failure repeats no notice");
}

#[test]
fn only_the_user_switches_offline_mode_off() {
    let time = clock(0);
    let switch = Arc::new(OfflineSwitch::new(time.clone()));
    let opener = GatedOpener::new(Arc::new(Down::default()), switch.clone());
    assert!(opener.open(&resolved()).is_err());
    assert!(opener.open(&resolved()).is_err());
    assert!(switch.is_offline(), "opener failures count");
    time.0.store(100_000, Ordering::SeqCst);
    assert!(switch.is_offline(), "time alone does not switch it off");

    switch.set_offline(false);
    assert!(!switch.is_offline());
    assert!(opener.open(&resolved()).is_err());
    assert!(
        !switch.is_offline(),
        "one failure after switching back is not enough"
    );
}

#[test]
fn background_lookups_do_not_switch_offline_mode_on() {
    let switch = Arc::new(OfflineSwitch::new(clock(0)));
    let down = Arc::new(Down::default());
    let segments = GatedSegments::new(down.clone(), switch.clone());
    let images = GatedImages::new(down.clone(), switch.clone());
    let video = VideoId::parse("aaaaaaaaaa1").expect("id");
    for _ in 0..3 {
        assert!(ready(segments.segments(&video, Duration::from_secs(1))).is_err());
        assert!(ready(images.image(&Artwork::None, 64)).is_err());
    }
    assert_eq!(down.calls(), 6);
    assert!(!switch.is_offline());
}
