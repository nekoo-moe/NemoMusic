//! The AI provider contract (ai-features): song suggestions, a taste reading and lyrics
//! translation through one endpoint the user sets.
//!
//! No I/O here. Every request is two steps the UI cannot skip: [`AiService::prepare`] builds the
//! exact messages for the user to see, and only after the user confirms does
//! [`AiService::send`] transmit that same preview. Nothing is called in the background.

use crate::BoxFuture;
use crate::secret::Secret;

/// One AI feature. Each has its own switch, off until the user turns it on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiFeature {
    /// Song suggestions from seed songs.
    Suggestions,
    /// A short reading of the user's taste.
    TasteReading,
    /// Translation of the lyrics on screen (lyrics-plus).
    LyricsTranslation,
}

/// Where the endpoint is, as far as the address tells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointScope {
    /// This computer (`localhost` or a loopback address).
    Loopback,
    /// A private address on the local network, written as an IP number.
    PrivateLan,
    /// Anywhere else (HTTPS only).
    Remote,
}

/// What the UI may show about the saved provider. Holds no key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProviderInfo {
    /// Host name data goes to.
    pub host: String,
    /// Model name the user typed.
    pub model: String,
    /// Where the host is.
    pub scope: EndpointScope,
    /// Whether requests go over plain HTTP (loopback or a private LAN address only).
    pub plain_http: bool,
}

/// A provider as the user enters it. `Debug` prints the key as `[redacted]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProviderConfig {
    /// Endpoint base URL, for example `http://localhost:11434`.
    pub endpoint: String,
    /// Model name.
    pub model: String,
    /// API key, if the endpoint needs one. Kept only in the OS credential store.
    pub api_key: Option<Secret<String>>,
    /// The user saw and accepted the warning for plain HTTP on the local network.
    pub confirm_plain_http: bool,
}

/// A song as sent to the provider: title and artist, and only what the feature needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongRef {
    /// Song title.
    pub title: String,
    /// Artist names as one line.
    pub artist: String,
    /// Album title, if the feature uses it.
    pub album: Option<String>,
    /// Play count, if the feature uses it.
    pub plays: Option<u32>,
}

/// Where the data of a taste reading comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TasteSource {
    /// Top artists and play counts from the local listening stats.
    Stats,
    /// The account's liked songs.
    LikedSongs,
    /// A playlist; `account` is true for a YouTube Music playlist, false for a local one.
    Playlist {
        /// Whether the playlist belongs to the signed-in account.
        account: bool,
    },
}

impl TasteSource {
    /// Whether the data came from the YouTube Music account. Readings from account data are
    /// deleted on sign-out; the others stay.
    #[must_use]
    pub const fn from_account(&self) -> bool {
        match self {
            Self::Stats => false,
            Self::LikedSongs => true,
            Self::Playlist { account } => *account,
        }
    }
}

/// What the user asked the AI to do, with the data the UI collected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiRequest {
    /// Suggest songs like these (at most 50 seeds).
    Suggest {
        /// The seed songs.
        seeds: Vec<SongRef>,
    },
    /// Read the user's taste.
    Taste {
        /// Where the data comes from.
        source: TasteSource,
        /// Top artists with their play counts.
        artists: Vec<(String, u32)>,
        /// Songs, when the source is a list of songs.
        songs: Vec<SongRef>,
        /// Language of the reading, as a BCP 47 tag.
        reply_language: String,
    },
    /// Translate lyric lines (at most 200).
    Translate {
        /// The lines, in order.
        lines: Vec<String>,
        /// Target language, as a BCP 47 tag.
        target_language: String,
    },
}

/// One message of a request, exactly as it will be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewMessage {
    /// `"system"` or `"user"`.
    pub role: &'static str,
    /// The text.
    pub content: String,
}

/// The exact request the user sees before anything is sent. [`AiService::send`] sends these
/// messages and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadPreview {
    /// Which feature asks.
    pub feature: AiFeature,
    /// The messages, in order.
    pub messages: Vec<PreviewMessage>,
    /// Host the request goes to.
    pub host: String,
    /// Model name.
    pub model: String,
    /// Where the host is.
    pub scope: EndpointScope,
    /// Whether the request goes over plain HTTP.
    pub plain_http: bool,
    /// How many songs the request holds.
    pub song_count: usize,
}

/// One song the model suggested. Only data: it is looked up in the catalog, never acted on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestedSong {
    /// Song title.
    pub title: String,
    /// Artist name.
    pub artist: String,
}

/// A parsed answer. The shape is fixed per feature; anything else is [`AiError::BadAnswer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiAnswer {
    /// Suggested songs.
    Songs(Vec<SuggestedSong>),
    /// A taste reading, plain text.
    Reading(String),
    /// Translated lines, one per sent line.
    Translated(Vec<String>),
}

/// An answer with the model and host that gave it, for the label next to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiResult {
    /// The parsed answer.
    pub answer: AiAnswer,
    /// Model name.
    pub model: String,
    /// Host.
    pub host: String,
}

/// A taste reading kept on this computer until the user deletes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedReading {
    /// Row id.
    pub id: i64,
    /// When it was saved, in seconds since the Unix epoch.
    pub created_at: i64,
    /// Where its data came from.
    pub source: TasteSource,
    /// Model name.
    pub model: String,
    /// Host.
    pub host: String,
    /// The reading.
    pub text: String,
}

/// An AI request failed. Texts are short and hold no URL, key or answer text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AiError {
    /// No provider is saved.
    #[error("no AI provider is set up")]
    NoProvider,
    /// The feature's switch is off.
    #[error("this AI feature is off")]
    FeatureOff,
    /// The endpoint URL breaks a rule; the text says which.
    #[error("this endpoint is not allowed: {0}")]
    InvalidEndpoint(&'static str),
    /// Too many songs or lines, or too much text, for one request.
    #[error("too much data for one request")]
    TooLarge,
    /// The app is offline and the endpoint is not on this computer.
    #[error("offline: the AI provider cannot be reached")]
    Offline,
    /// The provider did not answer in time.
    #[error("the AI provider did not answer in time")]
    Timeout,
    /// The provider could not be reached.
    #[error("cannot reach the AI provider")]
    Unreachable,
    /// The provider refused the key (HTTP 401 or 403).
    #[error("the AI provider rejected the API key")]
    KeyRejected,
    /// The provider does not know the model (HTTP 404).
    #[error("the AI provider does not have this model")]
    ModelMissing,
    /// The provider asked to slow down (HTTP 429).
    #[error("the AI provider asked to slow down; try again later")]
    RateLimited,
    /// The provider failed (HTTP 5xx).
    #[error("the AI provider had an error")]
    ServerError,
    /// The answer was not in the expected form; nothing from it is shown.
    #[error("the answer could not be used")]
    BadAnswer,
    /// The answer was empty.
    #[error("the AI provider sent an empty answer")]
    Empty,
    /// The local settings or readings could not be read or written.
    #[error("cannot save AI data on this computer")]
    Storage,
    /// The OS credential store failed for the API key.
    #[error("cannot use the credential store for the API key")]
    SecretStore,
}

/// The AI provider service the UI uses. With no provider ([`AiService::provider`] is `None`)
/// the UI builds no AI control at all.
pub trait AiService: Send + Sync {
    /// The saved provider, or `None` when none is set up. Cheap.
    fn provider(&self) -> Option<AiProviderInfo>;

    /// Check and save a provider; the key goes to the OS credential store only.
    ///
    /// # Errors
    /// [`AiError::InvalidEndpoint`] for a URL that breaks the endpoint rules or plain HTTP on the
    /// local network without `confirm_plain_http`; [`AiError::Storage`] or
    /// [`AiError::SecretStore`] when saving fails.
    fn save_provider(&self, cfg: AiProviderConfig) -> Result<AiProviderInfo, AiError>;

    /// Delete the endpoint, the model and the key, and the saved readings when
    /// `delete_readings` is true.
    ///
    /// # Errors
    /// [`AiError::SecretStore`] when the key could not be deleted, [`AiError::Storage`] when
    /// the settings could not be.
    fn remove_provider(&self, delete_readings: bool) -> Result<(), AiError>;

    /// Send a fixed test message that holds no user data.
    ///
    /// # Errors
    /// Any request error, such as [`AiError::Unreachable`] or [`AiError::KeyRejected`].
    fn test_connection(&self) -> BoxFuture<'_, Result<(), AiError>>;

    /// The models the endpoint lists. Optional on the server side.
    ///
    /// # Errors
    /// Any request error; servers without a model list answer [`AiError::ModelMissing`].
    fn list_models(&self) -> BoxFuture<'_, Result<Vec<String>, AiError>>;

    /// Whether the user turned `feature` on.
    fn feature_enabled(&self, feature: AiFeature) -> bool;

    /// Turn `feature` on or off.
    fn set_feature_enabled(&self, feature: AiFeature, on: bool);

    /// Whether the preview step is skipped for `feature`. Always false for
    /// [`AiFeature::TasteReading`].
    fn skip_preview(&self, feature: AiFeature) -> bool;

    /// Set "Do not ask again" for `feature`. Ignored for [`AiFeature::TasteReading`].
    fn set_skip_preview(&self, feature: AiFeature, on: bool);

    /// Whether a request to a remote endpoint failed on the network in the last 60 seconds.
    fn offline_hint(&self) -> bool;

    /// Build the exact request for the user to see. No I/O.
    ///
    /// # Errors
    /// [`AiError::NoProvider`], [`AiError::FeatureOff`], or [`AiError::TooLarge`] past the
    /// limits (50 songs, 200 lines, 64 KiB).
    fn prepare(&self, req: &AiRequest) -> Result<PayloadPreview, AiError>;

    /// Send a confirmed preview, exactly as built, and parse the answer strictly.
    ///
    /// # Errors
    /// Any request error, or [`AiError::BadAnswer`] / [`AiError::Empty`] for an unusable answer.
    fn send<'a>(&'a self, preview: &'a PayloadPreview) -> BoxFuture<'a, Result<AiResult, AiError>>;

    /// The saved taste readings, newest first.
    fn readings(&self) -> Vec<SavedReading>;

    /// Save a taste reading on this computer. Returns its id.
    ///
    /// # Errors
    /// [`AiError::Storage`] when it cannot be written, [`AiError::BadAnswer`] when `result` is
    /// not a reading.
    fn save_reading(&self, source: TasteSource, result: &AiResult) -> Result<i64, AiError>;

    /// Delete one saved reading.
    fn delete_reading(&self, id: i64);
}

/// An AI service with no provider. Every AI control stays hidden.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoAi;

impl AiService for NoAi {
    fn provider(&self) -> Option<AiProviderInfo> {
        None
    }

    fn save_provider(&self, _cfg: AiProviderConfig) -> Result<AiProviderInfo, AiError> {
        Err(AiError::NoProvider)
    }

    fn remove_provider(&self, _delete_readings: bool) -> Result<(), AiError> {
        Ok(())
    }

    fn test_connection(&self) -> BoxFuture<'_, Result<(), AiError>> {
        Box::pin(async { Err(AiError::NoProvider) })
    }

    fn list_models(&self) -> BoxFuture<'_, Result<Vec<String>, AiError>> {
        Box::pin(async { Err(AiError::NoProvider) })
    }

    fn feature_enabled(&self, _feature: AiFeature) -> bool {
        false
    }

    fn set_feature_enabled(&self, _feature: AiFeature, _on: bool) {}

    fn skip_preview(&self, _feature: AiFeature) -> bool {
        false
    }

    fn set_skip_preview(&self, _feature: AiFeature, _on: bool) {}

    fn offline_hint(&self) -> bool {
        false
    }

    fn prepare(&self, _req: &AiRequest) -> Result<PayloadPreview, AiError> {
        Err(AiError::NoProvider)
    }

    fn send<'a>(
        &'a self,
        _preview: &'a PayloadPreview,
    ) -> BoxFuture<'a, Result<AiResult, AiError>> {
        Box::pin(async { Err(AiError::NoProvider) })
    }

    fn readings(&self) -> Vec<SavedReading> {
        Vec::new()
    }

    fn save_reading(&self, _source: TasteSource, _result: &AiResult) -> Result<i64, AiError> {
        Err(AiError::NoProvider)
    }

    fn delete_reading(&self, _id: i64) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_config_debug_hides_the_key() {
        let cfg = AiProviderConfig {
            endpoint: "http://localhost:11434".to_owned(),
            model: "llama3".to_owned(),
            api_key: Some(Secret::new("sk-synthetic-key".to_owned())),
            confirm_plain_http: false,
        };
        let text = format!("{cfg:?}");
        assert!(!text.contains("sk-synthetic-key"), "{text}");
        assert!(text.contains("[redacted]"), "{text}");
    }

    #[test]
    fn only_account_sources_count_as_account_data() {
        let table = [
            (TasteSource::Stats, false),
            (TasteSource::LikedSongs, true),
            (TasteSource::Playlist { account: true }, true),
            (TasteSource::Playlist { account: false }, false),
        ];
        for (source, expected) in table {
            assert_eq!(source.from_account(), expected, "{source:?}");
        }
    }

    #[test]
    fn no_ai_has_no_provider_and_nothing_on() {
        assert_eq!(NoAi.provider(), None);
        assert!(!NoAi.feature_enabled(AiFeature::Suggestions));
        assert!(!NoAi.skip_preview(AiFeature::LyricsTranslation));
        assert!(NoAi.readings().is_empty());
        let req = AiRequest::Suggest { seeds: Vec::new() };
        assert_eq!(NoAi.prepare(&req), Err(AiError::NoProvider));
    }
}
