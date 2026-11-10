//! Decode a local FLAC or WAV file into interleaved `f32` frames.
//!
//! # Exactness
//!
//! An `f32` has a 24-bit significand. Every 16-bit and 24-bit PCM sample is therefore stored
//! exactly, as `integer / 2^(bits - 1)` (a power-of-two division is exact). 32-bit integer PCM
//! is **not** exact in `f32`: low bits are rounded. [`DecodedAudio::is_exact`] says which case
//! applies, and the badge never claims more than the source states.

use std::fs::File;
use std::io;
use std::path::Path;

use nemo_core::audio::{AudioFormat, Codec, Origin, StreamInfo};
use symphonia::core::codecs::audio::AudioCodecId;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::codecs::audio::well_known::{
    CODEC_ID_AAC, CODEC_ID_FLAC, CODEC_ID_MP3, CODEC_ID_PCM_MULAW, CODEC_ID_PCM_S32LE,
};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

/// Why a file could not be decoded.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// The file could not be opened.
    #[error("cannot open audio file: {0}")]
    Open(#[source] io::Error),
    /// The container or codec reader failed.
    #[error("audio decode failed: {0}")]
    Symphonia(#[source] SymphoniaError),
    /// The file has no audio track.
    #[error("the file has no audio track")]
    NoAudioTrack,
    /// The stream does not state a fact the engine needs. Nothing is guessed.
    #[error("the stream does not state its {0}")]
    MissingParameter(&'static str),
    /// The stream uses a codec this build cannot decode (for example Opus).
    #[error("unsupported codec")]
    UnsupportedCodec,
    /// The channel count or sample rate is outside what this crate can carry.
    #[error("unsupported stream layout: {0}")]
    UnsupportedLayout(&'static str),
}

impl From<SymphoniaError> for DecodeError {
    fn from(err: SymphoniaError) -> Self {
        Self::Symphonia(err)
    }
}

/// A whole file decoded to interleaved `f32` samples.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAudio {
    /// Measured facts about the source (origin [`Origin::Local`]).
    pub info: StreamInfo,
    /// PCM format of `samples`.
    pub format: AudioFormat,
    /// Interleaved samples, `frames * channels` long.
    pub samples: Vec<f32>,
}

impl DecodedAudio {
    /// Number of frames (one sample per channel).
    #[must_use]
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.format.channels.max(1))
    }

    /// `true` when `f32` holds every source sample exactly (known bit depth of 24 or less).
    #[must_use]
    pub fn is_exact(&self) -> bool {
        self.format.bit_depth.is_some_and(|bits| bits <= 24)
    }
}

/// Decode a local FLAC or WAV file completely into memory.
///
/// The whole file is held in memory. Streaming decode is out of scope for this slice.
///
/// # Errors
///
/// Returns [`DecodeError`] when the file cannot be opened, is not FLAC or PCM WAV, has no
/// audio track, omits its sample rate or channel count, or a packet fails to decode.
pub fn decode_file(path: &Path) -> Result<DecodedAudio, DecodeError> {
    let file = File::open(path).map_err(DecodeError::Open)?;
    let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut reader = symphonia::default::get_probe().probe(
        &hint,
        mss,
        FormatOptions::default(),
        MetadataOptions::default(),
    )?;
    let track = reader
        .default_track(TrackType::Audio)
        .ok_or(DecodeError::NoAudioTrack)?;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or(DecodeError::NoAudioTrack)?;
    let track_id = track.id;

    let sample_rate = params
        .sample_rate
        .ok_or(DecodeError::MissingParameter("sample rate"))?;
    let channels = params
        .channels
        .as_ref()
        .ok_or(DecodeError::MissingParameter("channel count"))?
        .count();
    let channels = u16::try_from(channels)
        .ok()
        .filter(|c| *c > 0)
        .ok_or(DecodeError::UnsupportedLayout("channel count"))?;
    let bit_depth = params.bits_per_sample.and_then(|b| u8::try_from(b).ok());
    let codec = codec_from_id(params.codec);
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())?;

    let mut samples: Vec<f32> = Vec::new();
    let mut scratch: Vec<f32> = Vec::new();
    while let Some(packet) = reader.next_packet()? {
        if packet.track_id != track_id {
            continue;
        }
        let buffer = decoder.decode(&packet)?;
        scratch.resize(buffer.samples_interleaved(), 0.0);
        buffer.copy_to_slice_interleaved(&mut scratch[..]);
        samples.extend_from_slice(&scratch);
    }

    let format = AudioFormat {
        sample_rate,
        channels,
        bit_depth,
    };
    Ok(DecodedAudio {
        info: StreamInfo {
            origin: Origin::Local,
            codec,
            bitrate_kbps: None,
            format: Some(format),
        },
        format,
        samples,
    })
}

/// Codecs the streaming decoder can really decode in this build, for stream selection.
///
/// Opus is absent on purpose: symphonia 0.6.1 has an Opus codec id (it can demux Opus in
/// Ogg or Matroska) but ships no Opus decoder, so offering it would only fail at playback.
#[must_use]
pub fn supported_stream_codecs() -> &'static [Codec] {
    &[Codec::Aac, Codec::Flac, Codec::Mp3, Codec::Pcm]
}

/// Map a symphonia codec id to the shared [`Codec`]. Every PCM id maps to [`Codec::Pcm`];
/// anything unlisted is [`Codec::Unknown`].
pub(crate) fn codec_from_id(id: AudioCodecId) -> Codec {
    if id == CODEC_ID_FLAC {
        Codec::Flac
    } else if id == CODEC_ID_AAC {
        Codec::Aac
    } else if id == CODEC_ID_MP3 {
        Codec::Mp3
    } else if (CODEC_ID_PCM_S32LE..=CODEC_ID_PCM_MULAW).contains(&id) {
        Codec::Pcm
    } else {
        Codec::Unknown
    }
}
