//! `FileSource` is the one reader for local and cached files: the probe finds the container with
//! no hint, and length, reads and seeks match the file on disk.

mod common;

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use common::fixtures::{flac_bytes, tone_i16};
use nemo_audio::source::{FileOpenError, FileSource};
use nemo_audio::stream_decoder::StreamDecoder;
use nemo_core::audio::Codec;
use nemo_core::services::ByteSource;

const RATE: u32 = 44_100;
const FRAMES: usize = 4096 * 4;

fn aac_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone-aac.m4a")
}

/// A temp file with a name that says nothing about its format, so no hint can help the probe.
fn temp_file(tag: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nemo-audio-file-source-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(format!("{tag}.bin"));
    std::fs::write(&path, bytes).expect("write fixture");
    path
}

fn flac_fixture() -> (Vec<u8>, Vec<i16>) {
    let samples: Vec<i16> = tone_i16(RATE, 2, FRAMES, 440.0)
        .into_iter()
        .map(|s| i16::try_from(s).expect("invariant: 16-bit tone"))
        .collect();
    (flac_bytes(&samples, RATE, 2), samples)
}

fn decode_all(decoder: &mut StreamDecoder) -> Vec<f32> {
    let mut all = Vec::new();
    let mut block = Vec::new();
    while decoder.next_block(&mut block).expect("decode") {
        all.extend_from_slice(&block);
    }
    all
}

fn to_i16(samples: &[f32]) -> Vec<i16> {
    #[allow(clippy::cast_possible_truncation)] // a 16-bit source scaled back is within range
    samples
        .iter()
        .map(|s| (f64::from(*s) * 32_768.0).round() as i16)
        .collect()
}

#[test]
fn aac_in_mp4_probes_with_no_hint() {
    let path = aac_fixture();
    let source = FileSource::open(&path).expect("open");
    let mut decoder = StreamDecoder::open(Box::new(source), None).expect("probe");
    let info = *decoder.info();
    assert_eq!(info.codec, Codec::Aac);
    assert_eq!(info.format.sample_rate, RATE);
    assert_eq!(info.format.channels, 2);
    assert_eq!(info.format.bit_depth, None, "AAC states no bit depth");
    let samples = decode_all(&mut decoder);
    let frames = samples.len() / 2;
    // One second, give or take the encoder's priming and padding frames.
    assert!((43_000..=47_000).contains(&frames), "{frames} frames");
    assert!(samples.iter().any(|s| s.abs() > 0.1), "the tone is audible");
}

#[test]
fn aac_in_mp4_seeks_to_the_middle() {
    let open = || {
        let source = FileSource::open(&aac_fixture()).expect("open");
        StreamDecoder::open(Box::new(source), None).expect("probe")
    };
    let whole = decode_all(&mut open()).len() / 2;
    let mut decoder = open();
    let mut first = Vec::new();
    assert!(decoder.next_block(&mut first).expect("decode"));
    decoder.seek(Duration::from_millis(500)).expect("seek");
    let rest = decode_all(&mut decoder).len() / 2;
    let expected = whole - 22_050;
    assert!(
        rest.abs_diff(expected) <= 2048,
        "{rest} frames after the seek, {expected} expected"
    );
}

#[test]
fn flac_probes_with_no_hint_and_decodes_exactly() {
    let (bytes, samples) = flac_fixture();
    let path = temp_file("flac", &bytes);
    let source = FileSource::open(&path).expect("open");
    let mut decoder = StreamDecoder::open(Box::new(source), None).expect("probe");
    let info = *decoder.info();
    assert_eq!(info.codec, Codec::Flac);
    assert_eq!(info.format.sample_rate, RATE);
    assert_eq!(info.format.channels, 2);
    assert_eq!(info.format.bit_depth, Some(16));
    assert_eq!(to_i16(&decode_all(&mut decoder)), samples);
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn flac_seek_lands_on_the_exact_frame() {
    let (bytes, samples) = flac_fixture();
    let path = temp_file("flac-seek", &bytes);
    let source = FileSource::open(&path).expect("open");
    let mut decoder = StreamDecoder::open(Box::new(source), None).expect("probe");
    // Frame 6000 is inside the second FLAC block, so the decoder drops a lead-in.
    let frame = 6000_usize;
    let at = Duration::from_secs_f64(6000.0 / f64::from(RATE));
    decoder.seek(at).expect("seek");
    let rest = to_i16(&decode_all(&mut decoder));
    assert_eq!(rest, samples[frame * 2..]);
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn length_reads_and_seeks_match_the_file() {
    let (bytes, _) = flac_fixture();
    let path = temp_file("bytes", &bytes);
    let mut source = FileSource::open(&path).expect("open");
    let len = u64::try_from(bytes.len()).expect("invariant: small file");
    assert_eq!(source.byte_len(), Some(len));
    assert_eq!(source.seek(SeekFrom::End(0)).expect("seek end"), len);
    assert_eq!(source.seek(SeekFrom::Start(1000)).expect("seek"), 1000);
    let mut chunk = [0_u8; 64];
    source.read_exact(&mut chunk).expect("read");
    assert_eq!(chunk[..], bytes[1000..1064]);
    assert_eq!(
        source.seek(SeekFrom::Current(-64)).expect("seek back"),
        1000
    );
    let mut all = Vec::new();
    source.seek(SeekFrom::Start(0)).expect("rewind");
    source.read_to_end(&mut all).expect("read all");
    assert_eq!(all, bytes);
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn open_errors_are_typed_and_hold_no_path() {
    let missing = std::env::temp_dir().join("nemo-missing-7f3a9c-Secret Song Title.flac");
    let err = FileSource::open(&missing).err().expect("missing file");
    assert_eq!(err, FileOpenError::NotFound);
    let text = err.to_string();
    for part in ["7f3a9c", "Secret Song Title", ".flac", "nemo-missing"] {
        assert!(!text.contains(part), "{text}");
    }
    let dir_err = FileSource::open(&std::env::temp_dir()).err();
    // Opening a folder fails on Windows; on Linux it opens and reads fail later.
    if let Some(e) = dir_err {
        assert!(
            !e.to_string()
                .contains(&*std::env::temp_dir().to_string_lossy())
        );
    }
}
