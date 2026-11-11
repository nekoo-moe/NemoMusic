//! In-memory audio fixtures: WAV and FLAC files generated from code.

/// A little-endian PCM WAV file with `bits` bits (16 or 24) per sample.
pub fn wav_bytes(samples: &[i32], bits: u32, rate: u32, channels: u16) -> Vec<u8> {
    let bytes_per = bits / 8;
    let data_len = u32::try_from(samples.len()).expect("invariant: small file") * bytes_per;
    let block = u16::try_from(bytes_per).expect("invariant: 2 or 3") * channels;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
    wav.extend_from_slice(&block.to_le_bytes());
    wav.extend_from_slice(
        &u16::try_from(bits)
            .expect("invariant: 16 or 24")
            .to_le_bytes(),
    );
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        wav.extend_from_slice(&s.to_le_bytes()[..usize::try_from(bytes_per).expect("invariant")]);
    }
    wav
}

/// A sine tone as 16-bit integers, the same in every channel, with a phase offset so no
/// sample is exactly zero.
pub fn tone_i16(rate: u32, channels: u16, frames: usize, freq: f64) -> Vec<i32> {
    let mut out = Vec::with_capacity(frames * usize::from(channels));
    for i in 0..frames {
        let t = f64::from(u32::try_from(i).expect("invariant: small index")) / f64::from(rate);
        let v = (0.5 * (2.0 * std::f64::consts::PI * freq * t + 0.3).sin() * 32767.0).round();
        #[allow(clippy::cast_possible_truncation)] // within the 16-bit range
        let v = v as i32;
        out.extend(std::iter::repeat_n(v, usize::from(channels)));
    }
    out
}

/// A track whose every sample is `level` (16-bit, stereo), so it can be told apart by value.
pub fn level_wav(level: i32, rate: u32, frames: usize) -> Vec<u8> {
    wav_bytes(&vec![level; frames * 2], 16, rate, 2)
}

/// Stereo 24-bit counter: left holds `frame + 1`, right holds `-(frame + 1)`. The frame number
/// can be read back from the decoded `f32` exactly.
pub fn counter_wav(rate: u32, frames: usize) -> Vec<u8> {
    let mut s = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let v = i32::try_from(i + 1).expect("invariant: small counter");
        s.push(v);
        s.push(-v);
    }
    wav_bytes(&s, 24, rate, 2)
}

/// Frame number stored by [`counter_wav`], read from a decoded left-channel sample.
pub fn counter_index(sample: f32) -> i64 {
    #[allow(clippy::cast_possible_truncation)] // the value is a small integer
    let v = (f64::from(sample) * 8_388_608.0).round() as i64;
    v - 1
}

fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0_u8;
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 {
                (crc << 1) ^ 0x07
            } else {
                crc << 1
            };
        }
    }
    crc
}

fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0_u16;
    for &b in data {
        crc ^= u16::from(b) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x8005
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// A 16-bit FLAC file with verbatim subframes, 4096-frame blocks. `frames` must be a multiple
/// of 4096. `samples` is interleaved. Verbatim frames are valid FLAC and exercise the real
/// FLAC reader, seek table-free seeking included.
pub fn flac_bytes(samples: &[i16], rate: u32, channels: u16) -> Vec<u8> {
    const BLOCK: usize = 4096;
    let ch = usize::from(channels);
    let frames = samples.len() / ch;
    assert_eq!(frames % BLOCK, 0, "fixture frames must fill whole blocks");
    let mut out = b"fLaC".to_vec();
    out.extend_from_slice(&[0x80, 0, 0, 34]); // last metadata block: STREAMINFO, 34 bytes
    out.extend_from_slice(&4096_u16.to_be_bytes());
    out.extend_from_slice(&4096_u16.to_be_bytes());
    out.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // frame sizes unknown
    let packed: u64 = (u64::from(rate) << 44)
        | (u64::from(channels - 1) << 41)
        | (15_u64 << 36)
        | u64::try_from(frames).expect("invariant: small");
    out.extend_from_slice(&packed.to_be_bytes());
    out.extend_from_slice(&[0_u8; 16]); // MD5 unknown
    for (n, block) in samples.chunks(BLOCK * ch).enumerate() {
        let mut frame = vec![
            0xFF,
            0xF8,
            0xC0,
            (u8::try_from(channels - 1).expect("ch") << 4) | 0x08,
        ];
        let number = char::from_u32(u32::try_from(n).expect("invariant: small")).expect("scalar");
        let mut utf8 = [0_u8; 4];
        frame.extend_from_slice(number.encode_utf8(&mut utf8).as_bytes());
        frame.push(crc8(&frame));
        for c in 0..ch {
            frame.push(0x02); // verbatim subframe
            for f in 0..BLOCK {
                frame.extend_from_slice(&block[f * ch + c].to_be_bytes());
            }
        }
        let crc = crc16(&frame);
        frame.extend_from_slice(&crc.to_be_bytes());
        out.extend_from_slice(&frame);
    }
    out
}
