//! Single-producer single-consumer ring of `f32` samples (lock-free, `rtrb`).
//!
//! A decode thread owns the [`Producer`]; the audio callback owns the [`Consumer`].

pub use rtrb::{Consumer, Producer};

/// Create a ring that holds `capacity_samples` interleaved samples.
#[must_use]
pub fn new_ring(capacity_samples: usize) -> (Producer<f32>, Consumer<f32>) {
    rtrb::RingBuffer::new(capacity_samples)
}

/// Copy as many samples as fit into the ring without blocking. Returns the count written.
pub fn push_slice(producer: &mut Producer<f32>, samples: &[f32]) -> usize {
    let n = producer.slots().min(samples.len());
    if n == 0 {
        return 0;
    }
    match producer.write_chunk(n) {
        Ok(mut chunk) => {
            let (first, second) = chunk.as_mut_slices();
            let split = first.len();
            first.copy_from_slice(&samples[..split]);
            second.copy_from_slice(&samples[split..n]);
            chunk.commit_all();
            n
        }
        Err(_) => 0,
    }
}
