use super::{Duration, Wait};
use std::future::IntoFuture;
use std::ops::{Add, AddAssign, Sub, SubAssign};
use wasip2::clocks::monotonic_clock;

/// A measurement of a monotonically nondecreasing clock. Opaque and useful only
/// with Duration.
///
/// This type wraps `std::time::Duration` so we can implement traits on it
/// without coherence issues, just like if we were implementing this in the
/// stdlib.
#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Clone, Copy)]
pub struct Instant(pub(crate) monotonic_clock::Instant);

impl Instant {
    /// Returns an instant corresponding to "now".
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wstd::time::Instant;
    ///
    /// let now = Instant::now();
    /// ```
    #[must_use]
    pub fn now() -> Self {
        Instant(wasip2::clocks::monotonic_clock::now())
    }

    /// Returns the amount of time elapsed from another instant to this one, or zero duration if
    /// that instant is later than this one.
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        Duration::from_nanos(self.0.checked_sub(earlier.0).unwrap_or_default())
    }

    /// Returns the amount of time elapsed since this instant.
    pub fn elapsed(&self) -> Duration {
        Instant::now().duration_since(*self)
    }
}

impl Add<Duration> for Instant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign<Duration> for Instant {
    fn add_assign(&mut self, rhs: Duration) {
        *self = Self(self.0 + rhs.0)
    }
}

impl Sub<Duration> for Instant {
    type Output = Self;

    fn sub(self, rhs: Duration) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign<Duration> for Instant {
    fn sub_assign(&mut self, rhs: Duration) {
        *self = Self(self.0 - rhs.0)
    }
}

impl IntoFuture for Instant {
    type Output = Instant;

    type IntoFuture = Wait;

    fn into_future(self) -> Self::IntoFuture {
        crate::task::sleep_until(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duration_since() {
        let x = Instant::now();
        let d = Duration::new(456, 789);
        let y = x + d;
        assert_eq!(y.duration_since(x), d);
    }
}
