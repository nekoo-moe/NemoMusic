//! A source that knows the length of its audio (an HLS stream whose init segment states none)
//! gives the decoder that length; a source that does not leaves it to the container.

mod common;

use std::io::{self, Read, Seek, SeekFrom};
use std::time::Duration;

use common::fixtures::level_wav;
use nemo_audio::source::MemorySource;
use nemo_audio::stream_decoder::StreamDecoder;
use nemo_core::services::ByteSource;

/// A [`MemorySource`] that states a length.
struct Hinted(MemorySource, Option<Duration>);

impl Read for Hinted {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Seek for Hinted {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.0.seek(to)
    }
}

impl ByteSource for Hinted {
    fn byte_len(&self) -> Option<u64> {
        self.0.byte_len()
    }

    fn duration_hint(&self) -> Option<Duration> {
        self.1
    }
}

fn wav() -> Vec<u8> {
    level_wav(1000, 48_000, 48_000)
}

#[test]
fn the_hint_replaces_the_containers_length() {
    let hint = Some(Duration::from_secs(183));
    let decoder = StreamDecoder::open(Box::new(Hinted(MemorySource::new(wav()), hint)), None)
        .expect("decoder");
    assert_eq!(decoder.info().duration, hint);
}

#[test]
fn no_hint_leaves_the_containers_length() {
    let decoder = StreamDecoder::open(Box::new(Hinted(MemorySource::new(wav()), None)), None)
        .expect("decoder");
    assert_eq!(decoder.info().duration, Some(Duration::from_secs(1)));
    let plain = StreamDecoder::open(Box::new(MemorySource::new(wav())), None).expect("decoder");
    assert_eq!(plain.info().duration, Some(Duration::from_secs(1)));
}
