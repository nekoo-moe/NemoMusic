//! Cover art loading with a disk cache.
//!
//! Remote art: [`pick_thumbnail`] chooses the size, the URL must be HTTPS on a host from
//! [`ALLOWED_IMAGE_HOSTS`], and the bytes are stored under a file name that is the SHA-256 hex of
//! the URL. No part of a server answer (header, redirect target, file name) is ever used in a file
//! name or path. Answers over [`MAX_IMAGE_BYTES`] and answers that do not look like an image are
//! dropped. Local art: only files with an image extension are read.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use nemo_core::BoxFuture;
use nemo_core::services::{ImageError, ImageSource};
use nemo_core::track::{Artwork, pick_thumbnail};

use crate::http::{InitError, https_redirects};
use crate::sponsorblock::sha256_hex;

/// Hosts that may serve cover art. `*.` matches any subdomain (at least one label), not the bare
/// domain. Adding a host needs owner approval (`tech.md`: every contacted host is listed).
pub const ALLOWED_IMAGE_HOSTS: [&str; 4] = [
    "i.ytimg.com",
    "*.ggpht.com",
    "*.googleusercontent.com",
    "music.youtube.com",
];

/// Largest image accepted, remote or local, in bytes.
pub const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;

/// Total time one download may take.
pub const IMAGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// File extensions read as local images (lower case).
const LOCAL_IMAGE_EXTENSIONS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "gif"];

/// True when `host` matches one entry of [`ALLOWED_IMAGE_HOSTS`].
fn host_allowed(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    ALLOWED_IMAGE_HOSTS.iter().any(|pattern| {
        pattern.strip_prefix("*.").map_or_else(
            || host == *pattern,
            |suffix| {
                host.strip_suffix(suffix)
                    .is_some_and(|label| label.ends_with('.') && label.len() > 1)
            },
        )
    })
}

/// True when `url` is HTTPS, has no user info and its host is on the allow-list.
#[must_use]
pub fn is_allowed_image_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.host_str().is_some_and(host_allowed)
    })
}

/// Cache file name for a URL: SHA-256 of the URL text as lower-case hex.
#[must_use]
pub fn cache_file_name(url: &str) -> String {
    sha256_hex(url.as_bytes())
}

/// JPEG, PNG, GIF or WebP by magic bytes.
fn looks_like_image(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])
        || bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || (bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP")
}

fn has_image_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| LOCAL_IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn read_capped(path: &Path) -> Result<Vec<u8>, ImageError> {
    let read_error = |text: &str| ImageError::Read(text.to_owned());
    let meta = std::fs::metadata(path).map_err(|_| read_error("could not read the file"))?;
    if !meta.is_file() {
        return Err(read_error("not a file"));
    }
    if meta.len() > MAX_IMAGE_BYTES {
        return Err(read_error("file too large"));
    }
    std::fs::read(path).map_err(|_| read_error("could not read the file"))
}

fn read_local(path: &Path) -> Result<Vec<u8>, ImageError> {
    if !has_image_extension(path) {
        return Err(ImageError::Read("not an image file".to_owned()));
    }
    read_capped(path)
}

#[derive(Debug)]
struct Inner {
    client: reqwest::Client,
    dir: PathBuf,
}

impl Inner {
    fn cache_path(&self, url: &str) -> PathBuf {
        self.dir.join(cache_file_name(url))
    }

    async fn download(&self, url: &str) -> Result<Vec<u8>, ImageError> {
        let net = |text: &str| ImageError::Network(text.to_owned());
        let mut response = self.client.get(url).send().await.map_err(|e| {
            if e.is_timeout() {
                net("request timed out")
            } else {
                net("request failed")
            }
        })?;
        if !response.status().is_success() {
            return Err(ImageError::Network(format!(
                "HTTP status {}",
                response.status().as_u16()
            )));
        }
        if response
            .content_length()
            .is_some_and(|n| n > MAX_IMAGE_BYTES)
        {
            return Err(net("image too large"));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| net("could not read the image"))?
        {
            let total = u64::try_from(body.len() + chunk.len()).unwrap_or(u64::MAX);
            if total > MAX_IMAGE_BYTES {
                return Err(net("image too large"));
            }
            body.extend_from_slice(&chunk);
        }
        if !looks_like_image(&body) {
            return Err(net("answer is not an image"));
        }
        Ok(body)
    }

    /// Best effort: write to a temporary name of our own, then rename. Failures are ignored.
    fn store(&self, url: &str, bytes: &[u8]) {
        let final_path = self.cache_path(url);
        let temp = self.dir.join(format!("{}.tmp", cache_file_name(url)));
        if std::fs::write(&temp, bytes).is_ok() && std::fs::rename(&temp, &final_path).is_err() {
            let _ = std::fs::remove_file(&temp);
        }
    }

    fn cached(&self, url: &str) -> Option<Vec<u8>> {
        let bytes = read_capped(&self.cache_path(url)).ok()?;
        looks_like_image(&bytes).then_some(bytes)
    }

    async fn remote(self: Arc<Self>, url: String) -> Result<Vec<u8>, ImageError> {
        let this = Arc::clone(&self);
        let key = url.clone();
        let hit = tokio::task::spawn_blocking(move || this.cached(&key))
            .await
            .ok()
            .flatten();
        if let Some(bytes) = hit {
            return Ok(bytes);
        }
        let bytes = self.download(&url).await?;
        let this = Arc::clone(&self);
        let to_store = bytes.clone();
        // Writing the cache must not delay the cover; ignore the outcome.
        let _ = tokio::task::spawn_blocking(move || this.store(&url, &to_store)).await;
        Ok(bytes)
    }
}

/// Loads cover art and keeps downloaded images in a cache directory.
#[derive(Debug, Clone)]
pub struct ImageCache {
    inner: Arc<Inner>,
    handle: tokio::runtime::Handle,
    /// Test switch: skip the host allow-list so the loopback server can be used.
    check_allow_list: bool,
}

impl ImageCache {
    /// Create the cache in `dir` (created when missing). `handle` is the runtime that does the work.
    ///
    /// # Errors
    /// [`InitError::CacheDir`] when the directory cannot be created, [`InitError::Client`] when the
    /// HTTP client cannot be built.
    pub fn new(dir: impl Into<PathBuf>, handle: tokio::runtime::Handle) -> Result<Self, InitError> {
        Self::build(dir.into(), handle, true)
    }

    fn build(
        dir: PathBuf,
        handle: tokio::runtime::Handle,
        production: bool,
    ) -> Result<Self, InitError> {
        std::fs::create_dir_all(&dir).map_err(|_| InitError::CacheDir)?;
        let client = reqwest::Client::builder()
            .timeout(IMAGE_TIMEOUT)
            .redirect(https_redirects())
            .https_only(production)
            .build()
            .map_err(|_| InitError::Client)?;
        Ok(Self {
            inner: Arc::new(Inner { client, dir }),
            handle,
            check_allow_list: production,
        })
    }

    async fn load(&self, artwork: &Artwork, target_px: u32) -> Result<Vec<u8>, ImageError> {
        match artwork {
            Artwork::None => Err(ImageError::Missing),
            Artwork::LocalFile(path) => {
                let path = path.clone();
                self.handle
                    .spawn_blocking(move || read_local(&path))
                    .await
                    .unwrap_or_else(|_| Err(ImageError::Read("read was cancelled".to_owned())))
            }
            Artwork::Remote(thumbnails) => {
                let url = pick_thumbnail(thumbnails, target_px)
                    .map(|t| t.url.clone())
                    .ok_or(ImageError::Missing)?;
                if self.check_allow_list && !is_allowed_image_url(&url) {
                    return Err(ImageError::Network("image host is not allowed".to_owned()));
                }
                let inner = Arc::clone(&self.inner);
                self.handle
                    .spawn(inner.remote(url))
                    .await
                    .unwrap_or_else(|_| Err(ImageError::Network("load was cancelled".to_owned())))
            }
        }
    }
}

impl ImageSource for ImageCache {
    fn image<'a>(
        &'a self,
        artwork: &'a Artwork,
        target_px: u32,
    ) -> BoxFuture<'a, Result<Vec<u8>, ImageError>> {
        // `load` only awaits tokio join handles, which any executor can poll.
        Box::pin(self.load(artwork, target_px))
    }
}

#[cfg(test)]
mod tests {
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
}
