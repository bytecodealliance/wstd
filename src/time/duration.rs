use super::{Instant, Wait};
use std::future::IntoFuture;
use std::ops::{Add, AddAssign, Sub, SubAssign};
use wasip2::clocks::monotonic_clock;

/// A Duration type to represent a span of time, typically used for system
/// timeouts.
///
/// This type wraps `std::time::Duration` so we can implement traits on it
/// without coherence issues, just like if we were implementing this in the
/// stdlib.
#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Clone, Copy)]
pub struct Duration(pub(crate) monotonic_clock::Duration);
impl Duration {
    /// Creates a new `Duration` from the specified number of whole seconds and
    /// additional nanoseconds.
    #[must_use]
    #[inline]
    pub fn new(secs: u64, nanos: u32) -> Duration {
        std::time::Duration::new(secs, nanos).into()
    }

    /// Creates a new `Duration` from the specified number of whole seconds.
    #[must_use]
    #[inline]
    pub fn from_secs(secs: u64) -> Duration {
        std::time::Duration::from_secs(secs).into()
    }

    /// Creates a new `Duration` from the specified number of milliseconds.
    #[must_use]
    #[inline]
    pub fn from_millis(millis: u64) -> Self {
        std::time::Duration::from_millis(millis).into()
    }

    /// Creates a new `Duration` from the specified number of microseconds.
    #[must_use]
    #[inline]
    pub fn from_micros(micros: u64) -> Self {
        std::time::Duration::from_micros(micros).into()
    }

    /// Creates a new `Duration` from the specified number of nanoseconds.
    #[must_use]
    #[inline]
    pub fn from_nanos(nanos: u64) -> Self {
        std::time::Duration::from_nanos(nanos).into()
    }

    /// Creates a new `Duration` from the specified number of seconds represented
    /// as `f64`.
    ///
    /// # Panics
    /// This constructor will panic if `secs` is not finite, negative or overflows `Duration`.
    ///
    /// # Examples
    /// ```no_run
    /// use wstd::time::Duration;
    ///
    /// let dur = Duration::from_secs_f64(2.7);
    /// assert_eq!(dur, Duration::new(2, 700_000_000));
    /// ```
    #[must_use]
    #[inline]
    pub fn from_secs_f64(secs: f64) -> Duration {
        std::time::Duration::from_secs_f64(secs).into()
    }

    /// Creates a new `Duration` from the specified number of seconds represented
    /// as `f32`.
    ///
    /// # Panics
    /// This constructor will panic if `secs` is not finite, negative or overflows `Duration`.
    #[must_use]
    #[inline]
    pub fn from_secs_f32(secs: f32) -> Duration {
        std::time::Duration::from_secs_f32(secs).into()
    }

    /// Returns the number of whole seconds contained by this `Duration`.
    #[must_use]
    #[inline]
    pub const fn as_secs(&self) -> u64 {
        self.0 / 1_000_000_000
    }

    /// Returns the number of whole milliseconds contained by this `Duration`.
    #[must_use]
    #[inline]
    pub const fn as_millis(&self) -> u128 {
        (self.0 / 1_000_000) as u128
    }

    /// Returns the number of whole microseconds contained by this `Duration`.
    #[must_use]
    #[inline]
    pub const fn as_micros(&self) -> u128 {
        (self.0 / 1_000) as u128
    }

    /// Returns the total number of nanoseconds contained by this `Duration`.
    #[must_use]
    #[inline]
    pub const fn as_nanos(&self) -> u128 {
        self.0 as u128
    }
}

impl From<std::time::Duration> for Duration {
    fn from(inner: std::time::Duration) -> Self {
        Self(
            inner
                .as_nanos()
                .try_into()
                .expect("only dealing with durations that can fit in u64"),
        )
    }
}

impl From<Duration> for std::time::Duration {
    fn from(duration: Duration) -> Self {
        Self::from_nanos(duration.0)
    }
}

impl Add<Duration> for Duration {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign<Duration> for Duration {
    fn add_assign(&mut self, rhs: Duration) {
        *self = Self(self.0 + rhs.0)
    }
}

impl Sub<Duration> for Duration {
    type Output = Self;

    fn sub(self, rhs: Duration) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign<Duration> for Duration {
    fn sub_assign(&mut self, rhs: Duration) {
        *self = Self(self.0 - rhs.0)
    }
}

impl IntoFuture for Duration {
    type Output = Instant;

    type IntoFuture = Wait;

    fn into_future(self) -> Self::IntoFuture {
        crate::task::sleep(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_from_as() {
        assert_eq!(Duration::new(456, 864209753).as_secs(), 456);
        assert_eq!(Duration::new(456, 864209753).as_millis(), 456864);
        assert_eq!(Duration::new(456, 864209753).as_micros(), 456864209);
        assert_eq!(Duration::new(456, 864209753).as_nanos(), 456864209753);

        assert_eq!(Duration::from_secs(9876543210).as_secs(), 9876543210);
        assert_eq!(Duration::from_secs(9876543210).as_millis(), 9876543210_000);
        assert_eq!(
            Duration::from_secs(9876543210).as_micros(),
            9876543210_000000
        );
        assert_eq!(
            Duration::from_secs(9876543210).as_nanos(),
            9876543210_000000000
        );

        assert_eq!(Duration::from_millis(9876543210).as_secs(), 9876543);
        assert_eq!(Duration::from_millis(9876543210).as_millis(), 9876543210);
        assert_eq!(
            Duration::from_millis(9876543210).as_micros(),
            9876543210_000
        );
        assert_eq!(
            Duration::from_millis(9876543210).as_nanos(),
            9876543210_000000
        );

        assert_eq!(Duration::from_micros(9876543210).as_secs(), 9876);
        assert_eq!(Duration::from_micros(9876543210).as_millis(), 9876543);
        assert_eq!(Duration::from_micros(9876543210).as_micros(), 9876543210);
        assert_eq!(Duration::from_micros(9876543210).as_nanos(), 9876543210_000);

        assert_eq!(Duration::from_nanos(9876543210).as_secs(), 9);
        assert_eq!(Duration::from_nanos(9876543210).as_millis(), 9876);
        assert_eq!(Duration::from_nanos(9876543210).as_micros(), 9876543);
        assert_eq!(Duration::from_nanos(9876543210).as_nanos(), 9876543210);
    }

    #[test]
    fn test_from_secs_float() {
        assert_eq!(Duration::from_secs_f64(158.9).as_secs(), 158);
        assert_eq!(Duration::from_secs_f32(158.9).as_secs(), 158);
        assert_eq!(Duration::from_secs_f64(159.1).as_secs(), 159);
        assert_eq!(Duration::from_secs_f32(159.1).as_secs(), 159);
    }
}
