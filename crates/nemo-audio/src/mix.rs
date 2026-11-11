//! Channel-count conversion. Exists only when the source and the device disagree, so the
//! default path does no arithmetic.

/// Weight of the centre and surround channels in the 5.1 to stereo fold-down (-3 dB).
const SURROUND_WEIGHT: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Converts interleaved frames from one channel count to another.
///
/// - 1 channel to N: the sample is duplicated into every output channel.
/// - 6 channels (5.1, order FL FR C LFE BL BR) to 2: ITU-style fold-down, LFE dropped,
///   normalised so full-scale input cannot clip.
/// - More channels to fewer: output channel `i` is the mean of the input channels `j` with
///   `j % to == i`.
/// - Fewer channels (but more than 1) to more: the extra output channels are silent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelMixer {
    from: usize,
    to: usize,
}

impl ChannelMixer {
    /// A mixer from `from` to `to` channels, or `None` when the counts are equal (no stage).
    #[must_use]
    pub fn new(from: u16, to: u16) -> Option<Self> {
        if from == to || from == 0 || to == 0 {
            None
        } else {
            Some(Self {
                from: usize::from(from),
                to: usize::from(to),
            })
        }
    }

    /// Convert whole frames from `input` into `out` (cleared first).
    pub fn mix(&self, input: &[f32], out: &mut Vec<f32>) {
        out.clear();
        out.reserve(input.len() / self.from * self.to);
        for frame in input.chunks_exact(self.from) {
            self.mix_frame(frame, out);
        }
    }

    fn mix_frame(&self, frame: &[f32], out: &mut Vec<f32>) {
        if self.from == 1 {
            out.extend(std::iter::repeat_n(frame[0], self.to));
        } else if self.from == 6 && self.to == 2 {
            let norm = 1.0 / (1.0 + 2.0 * SURROUND_WEIGHT);
            out.push((frame[0] + SURROUND_WEIGHT * (frame[2] + frame[4])) * norm);
            out.push((frame[1] + SURROUND_WEIGHT * (frame[2] + frame[5])) * norm);
        } else if self.from > self.to {
            for i in 0..self.to {
                let group = frame.iter().skip(i).step_by(self.to);
                let (sum, n) = group.fold((0.0_f32, 0_u16), |(s, n), v| (s + v, n + 1));
                out.push(sum / f32::from(n));
            }
        } else {
            out.extend_from_slice(frame);
            out.extend(std::iter::repeat_n(0.0, self.to - self.from));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_counts_need_no_mixer() {
        assert!(ChannelMixer::new(2, 2).is_none());
    }

    #[test]
    fn mono_is_duplicated() {
        let mut out = Vec::new();
        ChannelMixer::new(1, 2)
            .expect("mixer")
            .mix(&[0.25, -0.5], &mut out);
        assert_eq!(out, [0.25, 0.25, -0.5, -0.5]);
    }

    #[test]
    fn stereo_to_mono_is_the_mean() {
        let mut out = Vec::new();
        ChannelMixer::new(2, 1)
            .expect("mixer")
            .mix(&[0.5, 0.25, -1.0, 1.0], &mut out);
        assert_eq!(out, [0.375, 0.0]);
    }

    #[test]
    fn surround_folds_down_without_clipping() {
        let mut out = Vec::new();
        ChannelMixer::new(6, 2)
            .expect("mixer")
            .mix(&[1.0, 1.0, 1.0, 0.0, 1.0, 1.0], &mut out);
        assert!(out.iter().all(|s| (*s - 1.0).abs() < 1e-6), "{out:?}");
    }

    #[test]
    fn quad_to_stereo_groups_by_parity() {
        let mut out = Vec::new();
        ChannelMixer::new(4, 2)
            .expect("mixer")
            .mix(&[1.0, 0.0, 0.0, 1.0], &mut out);
        assert_eq!(out, [0.5, 0.5]);
    }
}
