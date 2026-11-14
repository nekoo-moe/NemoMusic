//! The one codec registry every decoder in this crate is built from: symphonia's own decoders
//! plus the Opus decoder (`symphonia-adapter-libopus`, libopus built from source).
//!
//! Keeping the list here means [`StreamDecoder`](crate::stream_decoder::StreamDecoder) and
//! [`decode_file`](crate::decoder::decode_file) share one code path, and the Opus decoder can be
//! swapped by changing one line.

use std::sync::OnceLock;

use symphonia::core::codecs::audio::{AudioCodecParameters, AudioDecoder, AudioDecoderOptions};
use symphonia::core::codecs::registry::CodecRegistry;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia_adapter_libopus::OpusDecoder;

use crate::decoder::DecodeError;

/// The default symphonia decoders enabled in `Cargo.toml`, plus the Opus decoder.
pub(crate) fn build_codecs() -> CodecRegistry {
    let mut registry = CodecRegistry::new();
    symphonia::default::register_enabled_codecs(&mut registry);
    registry.register_audio_decoder::<OpusDecoder>();
    registry
}

/// The registry, built once on first use (on a prepare or decode thread, never in the callback).
fn codecs() -> &'static CodecRegistry {
    static CODECS: OnceLock<CodecRegistry> = OnceLock::new();
    CODECS.get_or_init(build_codecs)
}

/// Build the decoder for a track.
///
/// A failure is logged with symphonia's reason only (it names the codec limit, never a path or
/// a tag), and the track then plays nothing.
///
/// # Errors
/// [`DecodeError::UnsupportedCodec`] when no decoder takes the codec or the decoder refuses its
/// parameters (an Opus stream with more than two channels, for example);
/// [`DecodeError::Symphonia`] for any other failure.
pub(crate) fn make_decoder(
    params: &AudioCodecParameters,
) -> Result<Box<dyn AudioDecoder>, DecodeError> {
    codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|e| {
            tracing::warn!(error = %e, "could not build the audio decoder");
            match e {
                SymphoniaError::Unsupported(_) => DecodeError::UnsupportedCodec,
                other => DecodeError::Symphonia(other),
            }
        })
}

#[cfg(test)]
mod tests {
    use symphonia::core::audio::Channels;
    use symphonia::core::audio::layouts::{CHANNEL_LAYOUT_3P0, CHANNEL_LAYOUT_STEREO};
    use symphonia::core::codecs::audio::well_known::{CODEC_ID_FLAC, CODEC_ID_OPUS};

    use super::*;

    fn opus_params(channels: Channels) -> AudioCodecParameters {
        let mut params = AudioCodecParameters::new();
        params
            .for_codec(CODEC_ID_OPUS)
            .with_sample_rate(48_000)
            .with_channels(channels);
        params
    }

    #[test]
    fn the_registry_holds_opus_and_the_symphonia_decoders() {
        let registry = build_codecs();
        assert!(registry.get_audio_decoder(CODEC_ID_OPUS).is_some());
        assert!(registry.get_audio_decoder(CODEC_ID_FLAC).is_some());
    }

    #[test]
    fn a_stereo_opus_decoder_builds() {
        assert!(make_decoder(&opus_params(CHANNEL_LAYOUT_STEREO)).is_ok());
    }

    #[test]
    fn a_refused_opus_decoder_is_an_unsupported_codec() {
        let err = make_decoder(&opus_params(CHANNEL_LAYOUT_3P0))
            .err()
            .expect("libopus here takes one or two channels");
        assert!(matches!(err, DecodeError::UnsupportedCodec), "{err:?}");
        assert_eq!(err.to_string(), "unsupported codec");
    }
}
