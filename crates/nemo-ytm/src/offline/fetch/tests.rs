//! Ranged download tests against the loopback test server. No request leaves 127.0.0.1.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use super::*;
use crate::test_server::{Response as Answer, TestServer, range_response};

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl Dir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("nemo-offline-fetch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("dir");
        Self(path)
    }

    fn part(&self) -> PathBuf {
        self.0.join("song.part")
    }

    fn dest(&self) -> PathBuf {
        self.0.join("song")
    }
}

/// 2.5 MiB of a repeating pattern, so offsets are visible in the bytes.
fn song() -> Vec<u8> {
    (0..READ_BYTES * 5 / 2)
        .map(|i| u8::try_from(i % 251).expect("below 256"))
        .collect()
}

fn fetch(dir: &Dir, url: &str, pause: &AtomicBool) -> Result<Fetched, FetchError> {
    let part = dir.part();
    let dest = dir.dest();
    let target = Target {
        part: &part,
        dest: &dest,
    };
    Fetcher::new(false).expect("client").fetch(
        &Secret::new(url.to_owned()),
        target,
        pause,
        &mut |_, _| {},
    )
}

#[test]
fn a_download_saves_exactly_the_served_bytes() {
    let data = song();
    let body = data.clone();
    let server = TestServer::start(move |req, _| range_response(&body, req));
    let dir = Dir::new("whole");
    let done = fetch(&dir, &server.url("/a"), &AtomicBool::new(false));
    let len = u64::try_from(data.len()).expect("len");
    assert_eq!(done, Ok(Fetched::Complete { bytes: len }));
    assert_eq!(std::fs::read(dir.dest()).expect("saved"), data);
    assert!(!dir.part().exists());
    assert_eq!(server.requests().len(), 1);
    assert_eq!(server.requests()[0].range, Some((0, None)));
}

#[test]
fn resume_continues_from_the_part_length() {
    let data = song();
    let body = data.clone();
    let server = TestServer::start(move |req, _| range_response(&body, req));
    let dir = Dir::new("resume");
    std::fs::write(dir.part(), &data[..1000]).expect("part");
    let done = fetch(&dir, &server.url("/a"), &AtomicBool::new(false));
    assert!(matches!(done, Ok(Fetched::Complete { .. })), "{done:?}");
    assert_eq!(server.requests()[0].range, Some((1000, None)));
    assert_eq!(std::fs::read(dir.dest()).expect("saved"), data);
}

#[test]
fn a_200_answer_to_a_range_restarts_from_zero() {
    let data = song();
    let body = data.clone();
    let server = TestServer::start(move |_, _| Answer::new(200, body.clone()));
    let dir = Dir::new("restart");
    std::fs::write(dir.part(), b"stale bytes from another url").expect("part");
    let done = fetch(&dir, &server.url("/a"), &AtomicBool::new(false));
    assert!(matches!(done, Ok(Fetched::Complete { .. })), "{done:?}");
    assert_eq!(std::fs::read(dir.dest()).expect("saved"), data);
}

#[test]
fn pause_stops_within_one_read_and_keeps_the_part() {
    let data = song();
    let body = data.clone();
    let server = TestServer::start(move |req, _| range_response(&body, req));
    let dir = Dir::new("pause");
    let part = dir.part();
    let dest = dir.dest();
    let pause = AtomicBool::new(false);
    let mut reads = 0;
    let done = Fetcher::new(false).expect("client").fetch(
        &Secret::new(server.url("/a")),
        Target {
            part: &part,
            dest: &dest,
        },
        &pause,
        &mut |_, _| {
            reads += 1;
            pause.store(true, Ordering::SeqCst);
        },
    );
    assert_eq!(reads, 1, "no read after the pause");
    let Ok(Fetched::Paused { done }) = done else {
        panic!("{done:?}");
    };
    let read_bytes = u64::try_from(READ_BYTES).expect("fits");
    assert!(done > 0 && done <= read_bytes, "{done}");
    assert_eq!(std::fs::metadata(&part).expect("part").len(), done);
    assert!(!dest.exists());
    let resumed = fetch(&dir, &server.url("/a"), &AtomicBool::new(false));
    assert!(
        matches!(resumed, Ok(Fetched::Complete { .. })),
        "{resumed:?}"
    );
    assert_eq!(server.requests()[1].range, Some((done, None)));
    assert_eq!(std::fs::read(&dest).expect("saved"), data);
}

#[test]
fn a_short_body_fails_and_keeps_what_arrived() {
    let server = TestServer::start(|_, _| {
        Answer::new(206, vec![9; 100]).header("Content-Range", "bytes 0-99/200")
    });
    let dir = Dir::new("short");
    let done = fetch(&dir, &server.url("/a"), &AtomicBool::new(false));
    assert_eq!(done, Err(FetchError::Length));
    assert!(!dir.dest().exists());
    assert_eq!(std::fs::metadata(dir.part()).expect("part").len(), 100);
}

#[test]
fn a_different_range_is_refused() {
    let server = TestServer::start(|_, _| {
        Answer::new(206, vec![9; 10]).header("Content-Range", "bytes 50-59/60")
    });
    let dir = Dir::new("other-range");
    std::fs::write(dir.part(), [0_u8; 10]).expect("part");
    let done = fetch(&dir, &server.url("/a"), &AtomicBool::new(false));
    assert_eq!(done, Err(FetchError::Length));
    assert_eq!(std::fs::metadata(dir.part()).expect("part").len(), 10);
}

#[test]
fn a_complete_part_file_is_moved_on_416() {
    let data = vec![3_u8; 64];
    let body = data.clone();
    let server = TestServer::start(move |req, _| range_response(&body, req));
    let dir = Dir::new("complete");
    std::fs::write(dir.part(), &data).expect("part");
    let done = fetch(&dir, &server.url("/a"), &AtomicBool::new(false));
    assert_eq!(done, Ok(Fetched::Complete { bytes: 64 }));
    assert_eq!(std::fs::read(dir.dest()).expect("saved"), data);
}

#[test]
fn rate_limits_and_errors_carry_no_url() {
    let server = TestServer::start(|req, _| {
        if req.path == "/slow" {
            Answer::new(429, Vec::new()).header("Retry-After", "30")
        } else {
            Answer::new(403, Vec::new())
        }
    });
    let dir = Dir::new("status");
    let slow = fetch(&dir, &server.url("/slow"), &AtomicBool::new(false));
    assert_eq!(
        slow,
        Err(FetchError::RateLimited(Some(Duration::from_secs(30))))
    );
    let denied = fetch(&dir, &server.url("/denied"), &AtomicBool::new(false));
    assert_eq!(denied, Err(FetchError::Status(403)));
    assert!(!format!("{denied:?}").contains("127.0.0.1"));
}

#[test]
fn an_http_url_is_refused_when_https_is_required() {
    let dir = Dir::new("https");
    let part = dir.part();
    let dest = dir.dest();
    let done = Fetcher::new(true).expect("client").fetch(
        &Secret::new("http://127.0.0.1:9/a".to_owned()),
        Target {
            part: &part,
            dest: &dest,
        },
        &AtomicBool::new(false),
        &mut |_, _| {},
    );
    assert_eq!(done, Err(FetchError::Network));
}
