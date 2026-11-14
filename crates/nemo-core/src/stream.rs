//! Resolving a YouTube Music track to a playable stream.

use std::time::SystemTime;

use crate::BoxFuture;
use crate::audio::{Codec, StreamInfo};
use crate::secret::Secret;
use crate::track::VideoId;

/// What the listener asked for when a stream is resolved. The default (automatic language, no
/// Premium, no codec hint) resolves exactly as [`StreamResolver::resolve`] does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamRequest {
    /// Preferred audio language as a BCP 47 tag (`"en"`, `"pt-BR"`). `None` means automatic:
    /// the video's original track.
    pub audio_language: Option<String>,
    /// Use the signed-in account's session to reach Premium formats (opt-in).
    pub prefer_premium: bool,
    /// A hint for the format ranker, never a codec the decoder cannot play (Cast asks for AAC
    /// for DLNA). `None` means automatic.
    pub prefer_codec: Option<Codec>,
}

/// One audio track of a video that has several (dubbed or described audio).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioTrackInfo {
    /// BCP 47 language tag, if the source gives one.
    pub language: Option<String>,
    /// Label as the source writes it, for example `English (dubbed)`.
    pub label: String,
    /// Whether this is the video's default (original) track.
    pub is_default: bool,
}

/// A resolved stream. The URL is a secret: it can carry the listener's IP and a signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStream {
    /// Direct audio URL. Never log it.
    pub url: Secret<String>,
    /// Measured facts about the stream, for the quality badge.
    pub info: StreamInfo,
    /// When the URL stops working, if known.
    pub expires_at: Option<SystemTime>,
}

/// Resolving a stream failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StreamError {
    /// The track cannot be played with this account (region, age, Premium only).
    #[error("track not available")]
    NotAvailable,
    /// The resolver program or library is missing.
    #[error("stream resolver is not installed")]
    ResolverMissing,
    /// The network request failed.
    #[error("network error: {0}")]
    Network(String),
    /// The resolver's answer could not be parsed.
    #[error("could not parse resolver answer: {0}")]
    Parse(String),
}

/// Turns a video id into a playable stream.
pub trait StreamResolver: Send + Sync {
    /// Resolve one track. Human-paced: callers must not resolve a whole queue at once.
    fn resolve<'a>(
        &'a self,
        video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>>;

    /// Resolve one track as `request` asks (language, Premium, codec hint). Human-paced, like
    /// [`StreamResolver::resolve`]. Default: ignores the request and calls `resolve`.
    fn resolve_with<'a>(
        &'a self,
        video: &'a VideoId,
        request: &'a StreamRequest,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        let _ = request;
        self.resolve(video)
    }

    /// Hint: `video` will probably be played soon (the next queue entries). A caching resolver
    /// may resolve it in the background. Must return at once. Default: does nothing.
    fn warm(&self, _video: &VideoId) {}

    /// The stream last returned for `video` could not be opened (for example HTTP 403). A
    /// resolver may drop a cached answer or switch to another method for this video. Must
    /// return at once. Default: does nothing.
    fn stream_failed(&self, _video: &VideoId) {}
}

#[cfg(test)]
mod tests {
    use std::task::{Context, Poll, Waker};

    use super::*;

    /// A resolver that implements only `resolve` and answers with a fixed URL.
    struct Fixed;

    impl StreamResolver for Fixed {
        fn resolve<'a>(
            &'a self,
            video: &'a VideoId,
        ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
            let url = Secret::new(format!("https://example.invalid/{}", video.as_str()));
            Box::pin(async move {
                Ok(ResolvedStream {
                    url,
                    info: StreamInfo {
                        origin: crate::audio::Origin::YoutubeMusic,
                        codec: Codec::Aac,
                        bitrate_kbps: Some(128),
                        format: None,
                    },
                    expires_at: None,
                })
            })
        }
    }

    fn ready<T>(mut future: BoxFuture<'_, T>) -> T {
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("the fixed resolver answers at once"),
        }
    }

    #[test]
    fn default_resolve_with_returns_what_resolve_returns() {
        let video = VideoId::parse("dQw4w9WgXcQ").expect("valid id");
        let request = StreamRequest {
            audio_language: Some("ja".to_owned()),
            prefer_premium: true,
            prefer_codec: Some(Codec::Aac),
        };
        assert_eq!(
            ready(Fixed.resolve_with(&video, &request)),
            ready(Fixed.resolve(&video))
        );
        assert_eq!(
            ready(Fixed.resolve_with(&video, &StreamRequest::default())),
            ready(Fixed.resolve(&video))
        );
    }

    #[test]
    fn the_default_request_is_automatic() {
        let request = StreamRequest::default();
        assert_eq!(request.audio_language, None);
        assert!(!request.prefer_premium);
        assert_eq!(request.prefer_codec, None);
    }
}
