//! Async time interfaces.

pub(crate) mod utils {
    use std::io;

    pub(crate) fn timeout_err(msg: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::TimedOut, msg)
    }
}

#[cfg(wstd_p2)]
use pin_project_lite::pin_project;
use std::future::{Future, IntoFuture};
use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::iter::AsyncIterator;

#[cfg(wstd_p2)]
use crate::runtime::AsyncPollable;

pub use crate::sys::time::SystemTime;

// ---- Duration ----

/// A Duration type to represent a span of time, typically used for system
/// timeouts.
///
/// This type wraps `std::time::Duration` so we can implement traits on it
/// without coherence issues, just like if we were implementing this in the
/// stdlib.
#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Clone, Copy)]
pub struct Duration(pub(crate) crate::sys::time::MonotonicDuration);
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

// ---- Instant ----

/// A measurement of a monotonically nondecreasing clock. Opaque and useful only
/// with Duration.
///
/// This type wraps `std::time::Duration` so we can implement traits on it
/// without coherence issues, just like if we were implementing this in the
/// stdlib.
#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Clone, Copy)]
pub struct Instant(pub(crate) crate::sys::time::MonotonicInstant);

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
        Instant(crate::sys::time::now())
    }

    /// Returns the amount of time elapsed from another instant to this one, or zero duration if
    /// that instant is later than this one.
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        Duration::from_nanos(self.0.saturating_sub(earlier.0))
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

// ---- Timer / Interval ----

/// An async iterator representing notifications at fixed interval.
pub fn interval(duration: Duration) -> Interval {
    Interval { duration }
}

/// An async iterator representing notifications at fixed interval.
///
/// See the [`interval`] function for more.
#[derive(Debug)]
pub struct Interval {
    duration: Duration,
}
impl AsyncIterator for Interval {
    type Item = Instant;

    async fn next(&mut self) -> Option<Self::Item> {
        Some(Timer::after(self.duration).wait().await)
    }
}

// ---- Timer (p2) ----

#[cfg(wstd_p2)]
#[derive(Debug)]
pub struct Timer(Option<AsyncPollable>);

#[cfg(wstd_p2)]
impl Timer {
    pub fn never() -> Timer {
        Timer(None)
    }
    pub fn at(deadline: Instant) -> Timer {
        let pollable = crate::sys::time::subscribe_at(deadline.0);
        Timer(Some(pollable))
    }
    pub fn after(duration: Duration) -> Timer {
        let pollable = crate::sys::time::subscribe_after(duration.0);
        Timer(Some(pollable))
    }
    pub fn set_after(&mut self, duration: Duration) {
        *self = Self::after(duration);
    }
    pub fn wait(&self) -> Wait {
        let wait_for = self.0.as_ref().map(AsyncPollable::wait_for);
        Wait { wait_for }
    }
}

#[cfg(wstd_p2)]
pin_project! {
    /// Future created by [`Timer::wait`]
    #[must_use = "futures do nothing unless polled or .awaited"]
    pub struct Wait {
        #[pin]
        wait_for: Option<crate::runtime::WaitFor>
    }
}

#[cfg(wstd_p2)]
impl Future for Wait {
    type Output = Instant;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.wait_for.as_pin_mut() {
            None => Poll::Pending,
            Some(f) => match f.poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(()) => Poll::Ready(Instant::now()),
            },
        }
    }
}

// ---- Timer (p3) ----

#[cfg(wstd_p3)]
pub struct Timer {
    kind: TimerKind,
}

#[cfg(wstd_p3)]
enum TimerKind {
    Never,
    After(Duration),
    At(Instant),
}

#[cfg(wstd_p3)]
impl std::fmt::Debug for Timer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timer").finish()
    }
}

#[cfg(wstd_p3)]
impl Timer {
    pub fn never() -> Timer {
        Timer {
            kind: TimerKind::Never,
        }
    }
    pub fn at(deadline: Instant) -> Timer {
        Timer {
            kind: TimerKind::At(deadline),
        }
    }
    pub fn after(duration: Duration) -> Timer {
        Timer {
            kind: TimerKind::After(duration),
        }
    }
    pub fn set_after(&mut self, duration: Duration) {
        *self = Self::after(duration);
    }
    pub fn wait(&self) -> Wait {
        let inner: Pin<Box<dyn Future<Output = ()>>> = match self.kind {
            TimerKind::Never => Box::pin(std::future::pending()),
            TimerKind::After(d) => Box::pin(crate::sys::time::timer_wait_for(d.0)),
            TimerKind::At(deadline) => Box::pin(crate::sys::time::timer_wait_until(deadline.0)),
        };
        Wait { inner }
    }
}

#[cfg(wstd_p3)]
#[must_use = "futures do nothing unless polled or .awaited"]
pub struct Wait {
    inner: Pin<Box<dyn Future<Output = ()>>>,
}

#[cfg(wstd_p3)]
impl Future for Wait {
    type Output = Instant;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.inner.as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => Poll::Ready(Instant::now()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    async fn debug_duration(what: &str, f: impl Future<Output = Instant>) {
        let start = Instant::now();
        let now = f.await;
        let d = now.duration_since(start);
        let d: std::time::Duration = d.into();
        println!("{what} awaited for {} s", d.as_secs_f32());
    }

    #[test]
    fn timer_now() {
        crate::runtime::block_on(debug_duration("timer_now", async {
            Timer::at(Instant::now()).wait().await
        }));
    }

    #[test]
    fn timer_after_100_milliseconds() {
        crate::runtime::block_on(debug_duration("timer_after_100_milliseconds", async {
            Timer::after(Duration::from_millis(100)).wait().await
        }));
    }

    #[test]
    fn test_duration_since() {
        let x = Instant::now();
        let d = Duration::new(456, 789);
        let y = x + d;
        assert_eq!(y.duration_since(x), d);
    }

    #[test]
    fn test_new_from_as() {
        assert_eq!(Duration::new(456, 864209753).as_secs(), 456);
        assert_eq!(Duration::new(456, 864209753).as_millis(), 456864);
        assert_eq!(Duration::new(456, 864209753).as_micros(), 456864209);
        assert_eq!(Duration::new(456, 864209753).as_nanos(), 456864209753);

        assert_eq!(Duration::from_secs(9876543210).as_secs(), 9876543210);
        assert_eq!(
            Duration::from_secs(9876543210).as_millis(),
            9_876_543_210_000
        );
        assert_eq!(
            Duration::from_secs(9876543210).as_micros(),
            9_876_543_210_000_000
        );
        assert_eq!(
            Duration::from_secs(9876543210).as_nanos(),
            9_876_543_210_000_000_000
        );

        assert_eq!(Duration::from_millis(9876543210).as_secs(), 9876543);
        assert_eq!(Duration::from_millis(9876543210).as_millis(), 9876543210);
        assert_eq!(
            Duration::from_millis(9876543210).as_micros(),
            9_876_543_210_000
        );
        assert_eq!(
            Duration::from_millis(9876543210).as_nanos(),
            9_876_543_210_000_000
        );

        assert_eq!(Duration::from_micros(9876543210).as_secs(), 9876);
        assert_eq!(Duration::from_micros(9876543210).as_millis(), 9876543);
        assert_eq!(Duration::from_micros(9876543210).as_micros(), 9876543210);
        assert_eq!(
            Duration::from_micros(9876543210).as_nanos(),
            9_876_543_210_000
        );

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
