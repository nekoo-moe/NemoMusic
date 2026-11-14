//! Incremental decoder: pulls packets from any [`ByteSource`] and hands out interleaved `f32`
//! blocks, so a stream is never decoded as a whole. Supports seeking by time.

use std::io;
use std::time::Duration;

use nemo_core::audio::{AudioFormat, Codec};
use nemo_core::services::ByteSource;
use symphonia::core::codecs::audio::AudioDecoder;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Time, TimeBase};

use crate::decoder::{DecodeError, codec_from_id, stated_bit_depth};
use crate::source::MediaAdapter;

/// Facts about an opened stream.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceInfo {
    /// The codec, as far as the container states it.
    pub codec: Codec,
    /// PCM format of the decoded blocks (the bit depth is `None` for lossy codecs).
    pub format: AudioFormat,
    /// Track duration, when the container states one.
    pub duration: Option<Duration>,
}

/// A stream opened for incremental decoding.
pub struct StreamDecoder {
    reader: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    info: SourceInfo,
    /// Frames still to drop after an accurate seek landed before the requested time.
    skip_frames: u64,
}

impl StreamDecoder {
    /// Probe the container and prepare the decoder. `extension` is a hint only (`"m4a"`).
    ///
    /// Reads the container header, so this may block on the network.
    ///
    /// # Errors
    /// [`DecodeError`] when the container is unknown, has no audio track, states no sample
    /// rate or channel count, or uses a codec this build cannot decode.
    pub fn open(source: Box<dyn ByteSource>, extension: Option<&str>) -> Result<Self, DecodeError> {
        let mss = MediaSourceStream::new(
            Box::new(MediaAdapter::new(source)),
            MediaSourceStreamOptions::default(),
        );
        let mut hint = Hint::new();
        if let Some(ext) = extension {
            hint.with_extension(ext);
        }
        let reader = symphonia::default::get_probe().probe(
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
        let format = AudioFormat {
            sample_rate,
            channels,
            bit_depth: stated_bit_depth(params),
        };
        let info = SourceInfo {
            codec: codec_from_id(params.codec),
            format,
            duration: track_duration(track, sample_rate),
        };
        let decoder = crate::codecs::make_decoder(params)?;
        Ok(Self {
            track_id: track.id,
            time_base: track.time_base,
            reader,
            decoder,
            info,
            skip_frames: 0,
        })
    }

    /// Facts about the stream.
    #[must_use]
    pub fn info(&self) -> &SourceInfo {
        &self.info
    }

    /// Decode the next block into `out` (cleared first), interleaved. Returns `false` at the
    /// end of the stream. A packet that fails to decode is skipped.
    ///
    /// # Errors
    /// [`DecodeError`] for unrecoverable container or I/O failures.
    pub fn next_block(&mut self, out: &mut Vec<f32>) -> Result<bool, DecodeError> {
        out.clear();
        loop {
            let packet = match self.reader.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => return Ok(false),
                Err(SymphoniaError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    return Ok(false);
                }
                Err(e) => return Err(e.into()),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(buffer) => {
                    out.resize(buffer.samples_interleaved(), 0.0);
                    buffer.copy_to_slice_interleaved(&mut out[..]);
                }
                Err(SymphoniaError::DecodeError(why)) => {
                    tracing::warn!(reason = why, "skipped an undecodable packet");
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
            self.drop_seek_lead_in(out);
            if !out.is_empty() {
                return Ok(true);
            }
        }
    }

    /// Remove the frames before the requested seek time from the front of `out`.
    fn drop_seek_lead_in(&mut self, out: &mut Vec<f32>) {
        if self.skip_frames == 0 {
            return;
        }
        let channels = usize::from(self.info.format.channels);
        let frames = out.len() / channels;
        let drop = usize::try_from(self.skip_frames).map_or(frames, |s| s.min(frames));
        out.drain(..drop * channels);
        self.skip_frames -= drop as u64;
    }

    /// Seek to `to` and land on the exact frame: the container seeks to a point at or before it
    /// and the decoder drops the lead-in.
    ///
    /// # Errors
    /// [`DecodeError`] when the container cannot seek there.
    pub fn seek(&mut self, to: Duration) -> Result<(), DecodeError> {
        let time = Time::try_from_nanos_u128(to.as_nanos()).unwrap_or(Time::MAX);
        let seeked = self.reader.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time,
                track_id: Some(self.track_id),
            },
        )?;
        self.decoder.reset();
        self.skip_frames = self.lead_in_frames(seeked.actual_ts, seeked.required_ts);
        Ok(())
    }

    /// Source frames between two timestamps of the track timebase.
    fn lead_in_frames(
        &self,
        actual: symphonia::core::units::Timestamp,
        required: symphonia::core::units::Timestamp,
    ) -> u64 {
        let Some(gap) = actual.duration_to(required) else {
            return 0;
        };
        match self.time_base.and_then(|tb| tb.calc_duration(gap)) {
            Some(t) => {
                let rate = f64::from(self.info.format.sample_rate);
                // Rounding to the nearest frame is the intent; the value is small and positive.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let frames = (t.as_secs_f64() * rate).round() as u64;
                frames
            }
            None => gap.get(),
        }
    }
}

/// Duration from the track's frame count or timebase, if the container states one.
fn track_duration(track: &symphonia::core::formats::Track, rate: u32) -> Option<Duration> {
    if let (Some(dur), Some(tb)) = (track.duration, track.time_base) {
        let secs = tb.calc_duration(dur)?.as_secs_f64();
        return Duration::try_from_secs_f64(secs).ok();
    }
    let frames = track.num_frames?;
    Duration::try_from_secs_f64(crate::units::f64_of(frames) / f64::from(rate.max(1))).ok()
}
