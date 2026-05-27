//! Saturating depth counter used by readers and writers to track nesting.

/// A saturating `u32` counter for nesting-depth tracking.
///
/// All arithmetic uses saturating semantics: `inc()` saturates at `u32::MAX`
/// (effectively unreachable for real documents), and `dec()` saturates at `0`.
/// This prevents underflow when malformed input would otherwise cause it,
/// in line with the project's fail-fast policy: a stuck-at-zero counter
/// surfaces as an invalid-sequence error downstream rather than a panic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Depth(u32);

impl Depth {
    /// Saturating decrement by one. Calling on a zero value is a no-op.
    #[inline]
    pub fn dec(&mut self) {
        self.0 = self.0.saturating_sub(1);
    }

    /// Returns the inner `u32` value.
    #[inline]
    #[must_use]
    pub fn get(self) -> u32 {
        self.0
    }

    /// Saturating increment by one. Calling at `u32::MAX` is a no-op.
    #[inline]
    pub fn inc(&mut self) {
        self.0 = self.0.saturating_add(1);
    }

    /// Returns `true` when the depth is greater than zero.
    #[inline]
    #[must_use]
    pub fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// Returns `true` when the depth is zero.
    #[inline]
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Resets the depth to zero. Used when a closing scope guarantees
    /// the current nesting level returns to the outermost state.
    #[inline]
    pub fn reset(&mut self) {
        self.0 = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        let d = Depth::default();
        assert_eq!(d.get(), 0);
        assert!(d.is_zero());
        assert!(!d.is_positive());
    }

    #[test]
    fn inc_increases_by_one() {
        let mut d = Depth::default();
        d.inc();
        assert_eq!(d.get(), 1);
        assert!(d.is_positive());
        assert!(!d.is_zero());
    }

    #[test]
    fn dec_decreases_by_one() {
        let mut d = Depth::default();
        d.inc();
        d.inc();
        d.dec();
        assert_eq!(d.get(), 1);
    }

    #[test]
    fn dec_at_zero_does_not_panic_or_underflow() {
        let mut d = Depth::default();
        d.dec();
        assert_eq!(d.get(), 0);
        d.dec();
        d.dec();
        assert_eq!(d.get(), 0);
    }

    #[test]
    fn inc_at_max_does_not_panic_or_overflow() {
        let mut d = Depth(u32::MAX);
        d.inc();
        assert_eq!(d.get(), u32::MAX);
    }

    #[test]
    fn reset_returns_to_zero() {
        let mut d = Depth::default();
        d.inc();
        d.inc();
        d.inc();
        d.reset();
        assert_eq!(d.get(), 0);
        assert!(d.is_zero());
    }
}
