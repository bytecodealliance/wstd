//! Async time interfaces.

pub(crate) mod utils;

mod duration;
mod instant;
pub use duration::Duration;
pub use instant::Instant;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::iter::AsyncIterator;

#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
use wasip2::clocks::{
    monotonic_clock::{subscribe_duration, subscribe_instant},
    wall_clock,
};

#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
use crate::runtime::{AsyncPollable, Reactor};

#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
use pin_project_lite::pin_project;

#[cfg(feature = "wasip3")]
use wasip3::clocks::{monotonic_clock, system_clock};


/// A measurement of the system clock, useful for talking to external entities
/// like the file system or other processes. May be converted losslessly to a
/// more useful `std::time::SystemTime` to provide more methods.
#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct SystemTime(wall_clock::Datetime);

#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
impl SystemTime {
    pub fn now() -> Self {
        Self(wall_clock::now())
    }
}

#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
impl From<SystemTime> for std::time::SystemTime {
    fn from(st: SystemTime) -> Self {
        std::time::SystemTime::UNIX_EPOCH
            + std::time::Duration::from_secs(st.0.seconds)
            + std::time::Duration::from_nanos(st.0.nanoseconds.into())
    }
}

#[cfg(feature = "wasip3")]
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct SystemTime(system_clock::Instant);

#[cfg(feature = "wasip3")]
impl SystemTime {
    pub fn now() -> Self {
        Self(system_clock::now())
    }
}

#[cfg(feature = "wasip3")]
impl From<SystemTime> for std::time::SystemTime {
    fn from(st: SystemTime) -> Self {
        // p3 system_clock::Instant has i64 seconds
        if st.0.seconds >= 0 {
            std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(st.0.seconds as u64)
                + std::time::Duration::from_nanos(st.0.nanoseconds.into())
        } else {
            std::time::SystemTime::UNIX_EPOCH
                - std::time::Duration::from_secs((-st.0.seconds) as u64)
                + std::time::Duration::from_nanos(st.0.nanoseconds.into())
        }
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


#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
#[derive(Debug)]
pub struct Timer(Option<AsyncPollable>);

#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
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

#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
pin_project! {
    /// Future created by [`Timer::wait`]
    #[must_use = "futures do nothing unless polled or .awaited"]
    pub struct Wait {
        #[pin]
        wait_for: Option<crate::runtime::WaitFor>
    }
}

#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
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


#[cfg(feature = "wasip3")]
pub struct Timer {
    kind: TimerKind,
}

#[cfg(feature = "wasip3")]
enum TimerKind {
    Never,
    After(Duration),
    At(Instant),
}

#[cfg(feature = "wasip3")]
impl std::fmt::Debug for Timer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timer").finish()
    }
}

#[cfg(feature = "wasip3")]
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
            TimerKind::After(d) => Box::pin(monotonic_clock::wait_for(d.0)),
            TimerKind::At(deadline) => Box::pin(monotonic_clock::wait_until(deadline.0)),
        };
        Wait { inner }
    }
}

#[cfg(feature = "wasip3")]
#[must_use = "futures do nothing unless polled or .awaited"]
pub struct Wait {
    inner: Pin<Box<dyn Future<Output = ()>>>,
}

#[cfg(feature = "wasip3")]
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
}
