use wasip2::clocks::{
    monotonic_clock::{self, subscribe_duration, subscribe_instant},
    wall_clock,
};

use crate::runtime::{AsyncPollable, Reactor};

/// A measurement of a monotonically nondecreasing clock. Opaque and useful only
/// with Duration.
pub type MonotonicInstant = monotonic_clock::Instant;

/// A duration from the monotonic clock, in nanoseconds.
pub type MonotonicDuration = monotonic_clock::Duration;

/// Return the current monotonic clock instant.
pub fn now() -> MonotonicInstant {
    monotonic_clock::now()
}

/// A measurement of the system clock, useful for talking to external entities
/// like the file system or other processes. May be converted losslessly to a
/// more useful `std::time::SystemTime` to provide more methods.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct SystemTime(wall_clock::Datetime);

impl SystemTime {
    pub fn now() -> Self {
        Self(wall_clock::now())
    }
}

impl From<SystemTime> for std::time::SystemTime {
    fn from(st: SystemTime) -> Self {
        std::time::SystemTime::UNIX_EPOCH
            + std::time::Duration::from_secs(st.0.seconds)
            + std::time::Duration::from_nanos(st.0.nanoseconds.into())
    }
}

/// Create a timer that fires at a specific monotonic clock instant.
pub fn subscribe_at(instant: MonotonicInstant) -> AsyncPollable {
    Reactor::current().schedule(subscribe_instant(instant))
}

/// Create a timer that fires after a monotonic clock duration.
pub fn subscribe_after(duration: MonotonicDuration) -> AsyncPollable {
    Reactor::current().schedule(subscribe_duration(duration))
}
