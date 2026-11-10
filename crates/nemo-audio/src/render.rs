//! The real-time render step: ring consumer -> output buffer.

use std::sync::atomic::{AtomicU64, Ordering};

use rtrb::Consumer;

/// Fill `out` from the ring. Returns how many samples came from the ring.
///
/// Any shortfall is filled with silence (`0.0`) and counts as **one** underrun in `underruns`,
/// however many samples are missing. The samples are copied unchanged: no gain, no resample, no
/// dither.
///
/// RT-SAFE: no allocation, no lock, no logging, no blocking. Only a copy and one relaxed
/// atomic add on the underrun path.
pub fn render(consumer: &mut Consumer<f32>, out: &mut [f32], underruns: &AtomicU64) -> usize {
    let want = consumer.slots().min(out.len());
    let mut copied = 0;
    if want > 0
        && let Ok(chunk) = consumer.read_chunk(want)
    {
        let (first, second) = chunk.as_slices();
        let split = first.len();
        out[..split].copy_from_slice(first);
        out[split..want].copy_from_slice(second);
        chunk.commit_all();
        copied = want;
    }
    if copied < out.len() {
        out[copied..].fill(0.0);
        underruns.fetch_add(1, Ordering::Relaxed);
    }
    copied
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ring::{new_ring, push_slice};

    #[test]
    fn full_ring_copies_unchanged_and_counts_nothing() {
        let (mut p, mut c) = new_ring(8);
        assert_eq!(push_slice(&mut p, &[0.25, -0.5, 0.75, -1.0]), 4);
        let mut out = [9.0; 4];
        let underruns = AtomicU64::new(0);
        assert_eq!(render(&mut c, &mut out, &underruns), 4);
        assert_eq!(out, [0.25, -0.5, 0.75, -1.0]);
        assert_eq!(underruns.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn underrun_writes_silence_and_counts_once_per_callback() {
        let (mut p, mut c) = new_ring(8);
        push_slice(&mut p, &[0.5, 0.5]);
        let underruns = AtomicU64::new(0);
        let mut out = [9.0; 6];
        assert_eq!(render(&mut c, &mut out, &underruns), 2);
        assert_eq!(out, [0.5, 0.5, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(underruns.load(Ordering::Relaxed), 1);
        let mut out = [9.0; 3];
        assert_eq!(render(&mut c, &mut out, &underruns), 0);
        assert_eq!(out, [0.0; 3]);
        assert_eq!(underruns.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn wrapped_ring_keeps_sample_order() {
        let (mut p, mut c) = new_ring(4);
        let underruns = AtomicU64::new(0);
        let mut out = [0.0; 3];
        push_slice(&mut p, &[1.0, 2.0, 3.0]);
        render(&mut c, &mut out, &underruns);
        push_slice(&mut p, &[4.0, 5.0, 6.0]);
        render(&mut c, &mut out, &underruns);
        assert_eq!(out, [4.0, 5.0, 6.0]);
        assert_eq!(underruns.load(Ordering::Relaxed), 0);
    }
}
