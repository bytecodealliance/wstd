//! Async time interfaces.

#[cfg(target_env = "p2")]
pub(crate) mod utils;

mod duration;
mod instant;
pub use duration::Duration;
pub use instant::Instant;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
#[cfg(target_env = "p2")]
use wasip2::clocks::wall_clock::{self, Datetime};
#[cfg(target_env = "p3")]
use wasip3::clocks::system_clock::{self as wall_clock, Instant as Datetime};

use crate::iter::AsyncIterator;
#[cfg(target_env = "p2")]
use crate::runtime::{AsyncPollable, Reactor};

/// A measurement of the system clock, useful for talking to external entities
/// like the file system or other processes. May be converted losslessly to a
/// more useful `std::time::SystemTime` to provide more methods.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct SystemTime(Datetime);

impl SystemTime {
    pub fn now() -> Self {
        Self(wall_clock::now())
    }
}

impl From<SystemTime> for std::time::SystemTime {
    #[cfg(target_env = "p2")]
    fn from(st: SystemTime) -> Self {
        std::time::SystemTime::UNIX_EPOCH
            + std::time::Duration::from_secs(st.0.seconds)
            + std::time::Duration::from_nanos(st.0.nanoseconds.into())
    }

    #[cfg(target_env = "p3")]
    fn from(st: SystemTime) -> Self {
        let mut result = std::time::SystemTime::UNIX_EPOCH;
        if st.0.seconds < 0 {
            result -= std::time::Duration::from_secs(st.0.seconds.unsigned_abs());
        } else {
            result += std::time::Duration::from_secs(st.0.seconds.unsigned_abs());
        }
        result + std::time::Duration::from_nanos(st.0.nanoseconds.into())
    }
}

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

#[cfg(target_env = "p2")]
mod timer {
    use super::*;
    use pin_project_lite::pin_project;
    use wasip2::clocks::monotonic_clock::{subscribe_duration, subscribe_instant};

    #[derive(Debug)]
    pub struct Timer(Option<AsyncPollable>);

    impl Timer {
        pub fn never() -> Timer {
            Timer(None)
        }
        pub fn at(deadline: Instant) -> Timer {
            let pollable = Reactor::current().schedule(subscribe_instant(deadline.0));
            Timer(Some(pollable))
        }
        pub fn after(duration: Duration) -> Timer {
            let pollable = Reactor::current().schedule(subscribe_duration(duration.0));
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

    pin_project! {
        /// Future created by [`Timer::wait`]
        #[must_use = "futures do nothing unless polled or .awaited"]
        pub struct Wait {
            #[pin]
            wait_for: Option<crate::runtime::WaitFor>
        }
    }

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
}

#[cfg(target_env = "p3")]
mod timer {
    use super::*;
    use wasip3::clocks::monotonic_clock::{wait_for, wait_until};

    #[derive(Debug)]
    pub struct Timer(TimerInner);

    enum TimerInner {
        Never,
        At(Instant),
        After(Duration),
    }

    impl std::fmt::Debug for TimerInner {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("Timer")
        }
    }

    impl Timer {
        pub fn never() -> Timer {
            Timer(TimerInner::Never)
        }
        pub fn at(deadline: Instant) -> Timer {
            Timer(TimerInner::At(deadline))
        }
        pub fn after(duration: Duration) -> Timer {
            Timer(TimerInner::After(duration))
        }
        pub fn set_after(&mut self, duration: Duration) {
            *self = Self::after(duration);
        }
        pub fn wait(&self) -> Wait {
            match &self.0 {
                TimerInner::Never => Wait(Box::pin(std::future::pending())),
                TimerInner::At(instant) => Wait(Box::pin(wait_until(instant.0))),
                TimerInner::After(duration) => Wait(Box::pin(wait_for(duration.0))),
            }
        }
    }

    /// Future created by [`Timer::wait`].
    #[must_use = "futures do nothing unless polled or .awaited"]
    pub struct Wait(Pin<Box<dyn Future<Output = ()>>>);

    impl Future for Wait {
        type Output = Instant;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            match self.0.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(()) => Poll::Ready(Instant::now()),
            }
        }
    }
}

pub use timer::*;

#[cfg(test)]
mod test {
    use super::*;

    #[cfg(target_env = "p2")]
    fn system_time(seconds: u64, nanoseconds: u32) -> SystemTime {
        SystemTime(Datetime {
            seconds,
            nanoseconds,
        })
    }

    #[cfg(target_env = "p3")]
    fn system_time(seconds: i64, nanoseconds: u32) -> SystemTime {
        SystemTime(Datetime {
            seconds,
            nanoseconds,
        })
    }

    #[test]
    fn system_time_conversion_at_epoch() {
        let actual: std::time::SystemTime = system_time(0, 0).into();

        assert_eq!(actual, std::time::SystemTime::UNIX_EPOCH);
    }

    #[test]
    fn system_time_conversion_after_epoch() {
        let actual: std::time::SystemTime = system_time(1, 999_999_999).into();
        let elapsed = actual
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap();

        assert_eq!(elapsed, std::time::Duration::new(1, 999_999_999));
    }

    #[cfg(target_env = "p3")]
    #[test]
    fn system_time_conversion_just_before_epoch() {
        let actual: std::time::SystemTime = system_time(-1, 999_999_999).into();
        let before_epoch = std::time::SystemTime::UNIX_EPOCH
            .duration_since(actual)
            .unwrap();

        assert_eq!(before_epoch, std::time::Duration::from_nanos(1));
    }

    #[cfg(target_env = "p3")]
    #[test]
    fn system_time_conversion_at_seconds_limits() {
        for seconds in [i64::MIN, i64::MAX] {
            let actual: std::time::SystemTime = system_time(seconds, 0).into();
            let elapsed = if seconds < 0 {
                std::time::SystemTime::UNIX_EPOCH
                    .duration_since(actual)
                    .unwrap()
            } else {
                actual
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap()
            };

            assert_eq!(
                elapsed,
                std::time::Duration::from_secs(seconds.unsigned_abs())
            );
        }
    }

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
}
