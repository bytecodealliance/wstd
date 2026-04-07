//! Async event loop support.
//!
//! The way to use this is to call [`block_on()`]. Inside the future, [`Reactor::current`]
//! will give an instance of the [`Reactor`] running the event loop, which can be
//! used to spawn tasks and (on p2) to schedule pollables.

#![deny(missing_debug_implementations, nonstandard_style)]
#![warn(missing_docs, unreachable_pub)]

mod block_on;
mod reactor;

pub use ::async_task::Task;
pub use block_on::block_on;
pub use reactor::Reactor;
use std::cell::RefCell;

#[cfg(wstd_p2)]
pub use reactor::{AsyncPollable, WaitFor};

// There are no threads in WASI, so this is just a safe way to thread a single reactor to all
// use sites in the background.
std::thread_local! {
pub(crate) static REACTOR: RefCell<Option<Reactor>> = const { RefCell::new(None) };
}

/// Spawn a `Future` as a `Task` on the current `Reactor`.
///
/// Panics if called from outside `block_on`.
pub fn spawn<F, T>(fut: F) -> Task<T>
where
    F: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    Reactor::current().spawn(fut)
}
