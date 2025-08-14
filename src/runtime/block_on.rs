use super::{Reactor, REACTOR};

use std::future::Future;
use std::pin::pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

/// Start the event loop
pub fn block_on<Fut>(fut: Fut) -> Fut::Output
where
    Fut: Future,
{
    // Construct the reactor
    let reactor = Reactor::new();
    // Store a copy as a singleton to be used elsewhere:
    let prev = REACTOR.replace(Some(reactor.clone()));
    if prev.is_some() {
        panic!("cannot wstd::runtime::block_on inside an existing block_on!")
    }

    // Pin the future so it can be polled
    let mut fut = pin!(fut);

    // Create a new context to be passed to the future.
    let root = Arc::new(RootWaker::new());
    let waker = Waker::from(root.clone());
    let mut cx = Context::from_waker(&waker);

    // Either the future completes and we return, or some IO is happening
    // and we wait.
    let res = loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(res) => break res,
            Poll::Pending => {
                // If some non-pollable based future has marked the root task
                // as awake, reset and poll again. otherwise, block until a
                // pollable wakes a future.
                if root.is_awake() {
                    reactor.nonblock_check_pollables();
                    root.reset()
                } else {
                    // If there are no futures awake or waiting on a WASI
                    // pollable, its impossible for the reactor to make
                    // progress, and the only valid behaviors are to sleep
                    // forever or panic. This should only be reachable if the
                    // user's Futures are implemented incorrectly.
                    if !reactor.nonempty_pending_pollables() {
                        panic!("reactor has no futures which are awake, or are waiting on a WASI pollable to be ready")
                    }
                    reactor.block_on_pollables()
                }
            }
        }
    };
    // Clear the singleton
    REACTOR.replace(None);
    res
}

/// This waker is used in the Context of block_on. If a Future executing in
/// the block_on calls context.wake(), it sets this boolean state so that
/// block_on's Future is polled again immediately, rather than waiting for
/// an external (WASI pollable) event before polling again.
struct RootWaker {
    wake: AtomicBool,
}
impl RootWaker {
    fn new() -> Self {
        Self {
            wake: AtomicBool::new(false),
        }
    }
    fn is_awake(&self) -> bool {
        self.wake.load(Ordering::Relaxed)
    }
    fn reset(&self) {
        self.wake.store(false, Ordering::Relaxed);
    }
}
impl Wake for RootWaker {
    fn wake(self: Arc<Self>) {
        self.wake.store(true, Ordering::Relaxed);
    }
}
