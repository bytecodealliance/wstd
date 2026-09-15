//! Async event loop support.
//!
//! On WASI 0.2 the way to use this is to call [`block_on()`]. Inside the
//! future, [`Reactor::current`] will give an instance of the [`Reactor`]
//! running the event loop, which can be used to [`AsyncPollable::wait_for`]
//! instances of
//! [`wasip2::Pollable`](https://docs.rs/wasi/latest/wasi/io/poll/struct.Pollable.html).
//! This will automatically wait for the futures to resolve, and call the
//! necessary wakers to work.
//!
//! On WASI 0.3 [`block_on`] can be used to drive a future, but an async
//! function can also be directly exported and will be driven by the host.

#![deny(missing_debug_implementations, nonstandard_style)]
#![warn(missing_docs, unreachable_pub)]

pub use ::async_task::Task;

#[cfg(target_env = "p2")]
mod block_on;
#[cfg(target_env = "p2")]
mod reactor;

#[cfg(target_env = "p2")]
pub use block_on::block_on;
#[cfg(target_env = "p2")]
pub use reactor::{AsyncPollable, Reactor, WaitFor};
#[cfg(target_env = "p2")]
use std::cell::RefCell;

// There are no threads in WASI 0.2, so this is just a safe way to thread a single reactor to all
// use sites in the background.
#[cfg(target_env = "p2")]
std::thread_local! {
pub(crate) static REACTOR: RefCell<Option<Reactor>> = const { RefCell::new(None) };
}

/// Spawn a `Future` as a `Task` on the current `Reactor`.
///
/// Panics if called from outside `block_on`.
#[cfg(target_env = "p2")]
pub fn spawn<F, T>(fut: F) -> Task<T>
where
    F: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    Reactor::current().spawn(fut)
}

#[cfg(target_env = "p3")]
pub use ::async_task::Runnable;
#[cfg(target_env = "p3")]
pub use wasip3::wit_bindgen::block_on;

/// Spawn a `Future` as a `Task` on the WASI 0.3 async runtime.
#[cfg(target_env = "p3")]
pub fn spawn<F, T>(fut: F) -> Task<T>
where
    F: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    let (runnable, task) = async_task::spawn_local(fut, |runnable: Runnable| {
        // Scheduling the task is accomplished by spawning a future which
        // executes the `run` method.
        wasip3::spawn_local(async move {
            let _ = runnable.run();
        });
    });
    runnable.schedule();
    task
}
