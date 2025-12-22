//! Async time interfaces.

pub(crate) mod utils;

mod duration;
mod instant;
pub use duration::Duration;
pub use instant::Instant;

use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use wasip2::clocks::{
    monotonic_clock::{subscribe_duration, subscribe_instant},
    wall_clock::{self, Datetime},
};

use crate::{
    iter::AsyncIterator,
    runtime::{AsyncPollable, Reactor},
};

/// A measurement of the system clock, useful for talking to external entities
/// like the file system or other processes.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct SystemTime(wall_clock::Datetime);

impl SystemTime {
    pub const UNIX_EPOCH: SystemTime = SystemTime(Datetime {
        seconds: 0,
        nanoseconds: 0,
    });

    pub fn now() -> Self {
        Self(wall_clock::now())
    }

    pub fn duration_since(&self, earlier: &SystemTime) -> Result<Duration, SystemTimeError> {
        if self.0.seconds >= earlier.0.seconds && self.0.nanoseconds >= earlier.0.nanoseconds {
            return Ok(Duration::new(
                self.0.seconds - earlier.0.seconds,
                self.0.nanoseconds - earlier.0.nanoseconds,
            ));
        }

        Err(SystemTimeError)
    }
}

pub struct SystemTimeError;

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
    fn systemtime_duration_since() {
        crate::runtime::block_on(async {
            let earlier = SystemTime::UNIX_EPOCH;
            let now = SystemTime::now();

            assert!(now.duration_since(&earlier).is_ok());
            assert!(now.duration_since(&now).is_ok_and(|x| x.as_secs() == 0));
            assert!(earlier.duration_since(&now).is_err());
        });
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
