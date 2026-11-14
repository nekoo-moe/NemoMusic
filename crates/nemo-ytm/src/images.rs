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
mod tests;
