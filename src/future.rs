//! Asynchronous values.
//!
//! # Cancellation
//!
//! Futures can be cancelled by dropping them before they finish executing. This
//! is useful when we're no longer interested in the result of an operation, as
//! it allows us to stop doing needless work. This also means that a future may cancel at any `.await` point, and so just
//! like with `?` we have to be careful to roll back local state if our future
//! halts there.
//!
//!
//! ```no_run
//! use futures_lite::prelude::*;
//! use wstd::prelude::*;
//! use wstd::time::Duration;
//!
//! #[wstd::main]
//! async fn main() {
//!     let mut counter = 0;
//!     let value = async { "meow" }
//!         .delay(Duration::from_millis(100))
//!         .timeout(Duration::from_millis(200))
//!         .await;
//!
//!     assert_eq!(value.unwrap(), "meow");
//! }
//! ```

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use pin_project_lite::pin_project;

use crate::time::utils::timeout_err;

pub use self::future_ext::FutureExt;

// ---- Delay ----

pin_project! {
    /// Suspends a future until the specified deadline.
    ///
    /// This `struct` is created by the [`delay`] method on [`FutureExt`]. See its
    /// documentation for more.
    ///
    /// [`delay`]: crate::future::FutureExt::delay
    /// [`FutureExt`]: crate::future::futureExt
    #[must_use = "futures do nothing unless polled or .awaited"]
    pub struct Delay<F, D> {
        #[pin]
        future: F,
        #[pin]
        deadline: D,
        state: State,
    }
}

/// The internal state
#[derive(Debug)]
enum State {
    Started,
    PollFuture,
    Completed,
}

impl<F, D> Delay<F, D> {
    fn new(future: F, deadline: D) -> Self {
        Self {
            future,
            deadline,
            state: State::Started,
        }
    }
}

impl<F: Future, D: Future> Future for Delay<F, D> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        loop {
            match this.state {
                State::Started => {
                    ready!(this.deadline.as_mut().poll(cx));
                    *this.state = State::PollFuture;
                }
                State::PollFuture => {
                    let value = ready!(this.future.as_mut().poll(cx));
                    *this.state = State::Completed;
                    return Poll::Ready(value);
                }
                State::Completed => panic!("future polled after completing"),
            }
        }
    }
}

// ---- Timeout ----

pin_project! {
    /// A future that times out after a duration of time.
    ///
    /// This `struct` is created by the [`timeout`] method on [`FutureExt`]. See its
    /// documentation for more.
    ///
    /// [`timeout`]: crate::future::FutureExt::timeout
    /// [`FutureExt`]: crate::future::futureExt
    #[must_use = "futures do nothing unless polled or .awaited"]
    pub struct Timeout<F, D> {
        #[pin]
        future: F,
        #[pin]
        deadline: D,
        completed: bool,
    }
}

impl<F, D> Timeout<F, D> {
    fn new(future: F, deadline: D) -> Self {
        Self {
            future,
            deadline,
            completed: false,
        }
    }
}

impl<F: Future, D: Future> Future for Timeout<F, D> {
    type Output = io::Result<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        assert!(!*this.completed, "future polled after completing");

        match this.future.poll(cx) {
            Poll::Ready(v) => {
                *this.completed = true;
                Poll::Ready(Ok(v))
            }
            Poll::Pending => match this.deadline.poll(cx) {
                Poll::Ready(_) => {
                    *this.completed = true;
                    Poll::Ready(Err(timeout_err("future timed out")))
                }
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

// ---- FutureExt ----

mod future_ext {
    use super::{Delay, Timeout};
    use std::future::{Future, IntoFuture};

    /// Extend `Future` with time-based operations.
    pub trait FutureExt: Future {
        /// Return an error if a future does not complete within a given time span.
        ///
        /// Typically timeouts are, as the name implies, based on _time_. However
        /// this method can time out based on any future. This can be useful in
        /// combination with channels, as it allows (long-lived) futures to be
        /// cancelled based on some external event.
        ///
        /// When a timeout is returned, the future will be dropped and destructors
        /// will be run.
        ///
        /// # Example
        ///
        /// ```no_run
        /// use wstd::prelude::*;
        /// use wstd::time::{Instant, Duration};
        /// use std::io;
        ///
        /// #[wstd::main]
        /// async fn main() {
        ///     let res = async { "meow" }
        ///         .delay(Duration::from_millis(100))  // longer delay
        ///         .timeout(Duration::from_millis(50)) // shorter timeout
        ///         .await;
        ///     assert_eq!(res.unwrap_err().kind(), io::ErrorKind::TimedOut); // error
        ///
        ///     let res = async { "meow" }
        ///         .delay(Duration::from_millis(50))    // shorter delay
        ///         .timeout(Duration::from_millis(100)) // longer timeout
        ///         .await;
        ///     assert_eq!(res.unwrap(), "meow"); // success
        /// }
        /// ```
        fn timeout<D>(self, deadline: D) -> Timeout<Self, D::IntoFuture>
        where
            Self: Sized,
            D: IntoFuture,
        {
            Timeout::new(self, deadline.into_future())
        }

        /// Delay resolving the future until the given deadline.
        ///
        /// The underlying future will not be polled until the deadline has expired. In addition
        /// to using a time source as a deadline, any future can be used as a
        /// deadline too. When used in combination with a multi-consumer channel,
        /// this method can be used to synchronize the start of multiple futures and streams.
        ///
        /// # Example
        ///
        /// ```no_run
        /// use wstd::prelude::*;
        /// use wstd::time::{Instant, Duration};
        ///
        /// #[wstd::main]
        /// async fn main() {
        ///     let now = Instant::now();
        ///     let delay = Duration::from_millis(100);
        ///     let _ = async { "meow" }.delay(delay).await;
        ///     assert!(now.elapsed() >= delay);
        /// }
        /// ```
        fn delay<D>(self, deadline: D) -> Delay<Self, D::IntoFuture>
        where
            Self: Sized,
            D: IntoFuture,
        {
            Delay::new(self, deadline.into_future())
        }
    }

    impl<T> FutureExt for T where T: Future {}
}
