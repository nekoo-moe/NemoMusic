//! Unit tests for the image cache.

use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use nemo_core::track::Thumbnail;

use super::*;
use crate::test_server::{Response, TestServer};

const PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

fn png_body(extra: usize) -> Vec<u8> {
    let mut body = PNG.to_vec();
    body.extend(std::iter::repeat_n(7_u8, extra));
    body
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("runtime builds")
}

/// A minimal executor that is not tokio, to prove the future is runtime-agnostic.
fn block_on_plain<F: Future>(future: F) -> F::Output {
    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        std::thread::park();
    }
}

fn remote(url: String) -> Artwork {
    Artwork::Remote(vec![Thumbnail {
        url,
        width: 100,
        height: 100,
    }])
}

/// A unique, empty directory under the OS temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "nemo-ytm-test-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn allow_list_accepts_only_listed_https_hosts() {
    for ok in [
        "https://i.ytimg.com/vi/x/hq.jpg",
        "https://lh3.googleusercontent.com/a=w544",
        "https://yt3.ggpht.com/a",
        "https://music.youtube.com/img",
        "https://I.YTIMG.COM/x",
        "https://a.b.googleusercontent.com/x",
    ] {
        assert!(is_allowed_image_url(ok), "{ok}");
    }
    for bad in [
        "http://i.ytimg.com/x",
        "https://evil.example/x",
        "https://i.ytimg.com.evil.example/x",
        "https://evili.ytimg.com/x",
        "https://googleusercontent.com/x",
        "https://notggpht.com/x",
        "https://.ggpht.com/x",
        "https://user:pw@i.ytimg.com/x",
        "https://www.youtube.com/x",
        "file:///C:/a.png",
        "i.ytimg.com/x",
        "",
    ] {
        assert!(!is_allowed_image_url(bad), "{bad}");
    }
}

#[test]
fn cache_name_is_the_sha256_hex_of_the_url() {
    // NIST vector: SHA-256("abc").
    assert_eq!(
        cache_file_name("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let name = cache_file_name("https://i.ytimg.com/a/../b?x=y/z");
    assert_eq!(name.len(), 64);
    assert!(name.bytes().all(|b| b.is_ascii_hexdigit()));
}

#[test]
fn magic_bytes() {
    assert!(looks_like_image(&[0xFF, 0xD8, 0xFF, 0xE0]));
    assert!(looks_like_image(&png_body(3)));
    assert!(looks_like_image(b"GIF89a.."));
    assert!(looks_like_image(b"RIFF\x00\x00\x00\x00WEBPVP8 "));
    assert!(!looks_like_image(b"<html>"));
    assert!(!looks_like_image(b"RIFF\x00\x00\x00\x00WAVEfmt "));
    assert!(!looks_like_image(b""));
}

#[test]
fn missing_art_and_empty_thumbnail_lists() {
    let dir = TempDir::new("missing");
    let rt = runtime();
    let cache = ImageCache::new(&dir.0, rt.handle().clone()).expect("cache");
    assert_eq!(
        block_on_plain(cache.image(&Artwork::None, 100)),
        Err(ImageError::Missing)
    );
    assert_eq!(
        block_on_plain(cache.image(&Artwork::Remote(Vec::new()), 100)),
        Err(ImageError::Missing)
    );
}

#[test]
fn hosts_off_the_list_are_refused_without_a_request() {
    let dir = TempDir::new("host");
    let server = TestServer::start(|_, _| Response::new(200, png_body(10)));
    let rt = runtime();
    let cache = ImageCache::new(&dir.0, rt.handle().clone()).expect("cache");
    // The loopback server is plain HTTP on an unlisted host, so both rules refuse it.
    let result = block_on_plain(cache.image(&remote(server.url("/a.png")), 100));
    assert!(matches!(result, Err(ImageError::Network(_))), "{result:?}");
    assert_eq!(server.requests().len(), 0);
}

#[test]
fn downloads_once_then_serves_from_disk() {
    let dir = TempDir::new("cache");
    let body = png_body(1000);
    let payload = body.clone();
    let server = TestServer::start(move |_, _| {
        Response::new(200, payload.clone()).header("Content-Type", "image/png")
    });
    let rt = runtime();
    let cache = ImageCache::build(dir.0.clone(), rt.handle().clone(), false).expect("cache");
    let url = server.url("/cover?evil=../../x");
    let art = remote(url.clone());
    assert_eq!(block_on_plain(cache.image(&art, 100)), Ok(body.clone()));
    assert_eq!(server.requests().len(), 1);

    // The file name is the hash of the URL, and nothing else is in the directory.
    let files: Vec<String> = std::fs::read_dir(&dir.0)
        .expect("read dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(files, vec![cache_file_name(&url)]);

    // A second call, even from a fresh cache object, does not touch the network.
    let again = ImageCache::build(dir.0.clone(), rt.handle().clone(), false).expect("cache");
    assert_eq!(block_on_plain(again.image(&art, 100)), Ok(body));
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn oversized_and_non_image_answers_are_dropped_and_not_cached() {
    let dir = TempDir::new("bad");
    let server = TestServer::start(|request, _| match request.path.as_str() {
        "/big" => Response::new(200, png_body(9 * 1024 * 1024)),
        "/html" => Response::new(200, b"<html>not an image</html>".to_vec()),
        "/gone" => Response::new(404, Vec::new()),
        _ => Response::new(500, Vec::new()),
    });
    let rt = runtime();
    let cache = ImageCache::build(dir.0.clone(), rt.handle().clone(), false).expect("cache");
    for path in ["/big", "/html", "/gone"] {
        let result = block_on_plain(cache.image(&remote(server.url(path)), 100));
        assert!(
            matches!(result, Err(ImageError::Network(_))),
            "{path}: {result:?}"
        );
    }
    assert_eq!(std::fs::read_dir(&dir.0).expect("read dir").count(), 0);
}

#[test]
fn local_files_need_an_image_extension() {
    let dir = TempDir::new("local");
    let rt = runtime();
    let cache = ImageCache::new(dir.0.join("cache"), rt.handle().clone()).expect("cache");
    let png = dir.0.join("cover.PNG");
    std::fs::write(&png, png_body(5)).expect("write");
    assert_eq!(
        block_on_plain(cache.image(&Artwork::LocalFile(png), 100)),
        Ok(png_body(5))
    );
    let audio = dir.0.join("song.flac");
    std::fs::write(&audio, b"fLaC").expect("write");
    let text = dir.0.join("notes.txt");
    std::fs::write(&text, b"x").expect("write");
    for path in [audio, text, dir.0.join("none.jpg"), dir.0.clone()] {
        let result = block_on_plain(cache.image(&Artwork::LocalFile(path.clone()), 100));
        assert!(
            matches!(result, Err(ImageError::Read(_))),
            "{path:?}: {result:?}"
        );
    }
    let big = dir.0.join("big.jpg");
    std::fs::write(&big, vec![0_u8; 9 * 1024 * 1024]).expect("write");
    assert!(matches!(
        block_on_plain(cache.image(&Artwork::LocalFile(big), 100)),
        Err(ImageError::Read(_))
    ));
}

#[test]
fn the_cache_directory_is_created() {
    let dir = TempDir::new("mkdir");
    let rt = runtime();
    let nested = dir.0.join("a").join("b");
    ImageCache::new(&nested, rt.handle().clone()).expect("cache");
    assert!(nested.is_dir());
}
