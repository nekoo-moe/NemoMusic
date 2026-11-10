//! Small numeric helpers.

/// A frame count as `f64`. Frame counts stay far below 2^52, so the conversion is exact.
#[allow(clippy::cast_precision_loss)] // reason: counts are far below 2^52
#[must_use]
pub(crate) fn f64_of(n: u64) -> f64 {
    n as f64
}
