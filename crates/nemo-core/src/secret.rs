//! A wrapper that keeps secrets out of logs (code.md, Logging).

use std::fmt;

/// A value that must never appear in logs, error messages or test fixtures.
///
/// `Debug` prints `[redacted]`. There is no `Display`. Read the value with [`Secret::expose`]
/// only where it is used, and never pass it to `tracing`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    /// Wrap a secret value.
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Borrow the inner value. The caller must not log it.
    #[must_use]
    pub const fn expose(&self) -> &T {
        &self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

#[cfg(test)]
mod tests {
    use super::Secret;

    #[test]
    fn debug_never_prints_the_value() {
        let s = Secret::new(String::from("SAPISID=abc"));
        assert_eq!(format!("{s:?}"), "[redacted]");
        assert_eq!(format!("{:?}", Some(&s)), "Some([redacted])");
    }

    #[test]
    fn expose_returns_the_value() {
        assert_eq!(Secret::new(7_u8).expose(), &7);
    }
}
