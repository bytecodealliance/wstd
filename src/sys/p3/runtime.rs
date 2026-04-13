pub use ::async_task::Task;

use async_task::{Runnable, Task as AsyncTask};
use core::future::Future;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

// There are no threads in WASI, so this is just a safe way to thread a single reactor to all
// use sites in the background.
std::thread_local! {
    pub(crate) static REACTOR: RefCell<Option<Reactor>> = const { RefCell::new(None) };
}

/// Start the event loop. Blocks until the future completes.
///
/// Delegates to wit-bindgen's block_on which integrates with the component
/// model's async runtime (waitable-set polling) for native p3 async support.
pub fn block_on<F>(fut: F) -> F::Output
where
    F: Future + 'static,
    F::Output: 'static,
{
    // Set up the reactor for spawn support
    let reactor = Reactor::new();
    let prev = REACTOR.replace(Some(reactor));
    if prev.is_some() {
        panic!("cannot wstd::runtime::block_on inside an existing block_on!")
    }

    let result = wasip3::wit_bindgen::rt::async_support::block_on(fut);

    REACTOR.replace(None);
    result
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

/// Manage async task scheduling for WASI 0.3
#[derive(Debug, Clone)]
pub struct Reactor {
    inner: Arc<InnerReactor>,
}

#[derive(Debug)]
struct InnerReactor {
    ready_list: Mutex<VecDeque<Runnable>>,
}

impl Reactor {
    /// Return a `Reactor` for the currently running `wstd::runtime::block_on`.
    ///
    /// # Panic
    /// This will panic if called outside of `wstd::runtime::block_on`.
    pub fn current() -> Self {
        REACTOR.with(|r| {
            r.borrow()
                .as_ref()
                .expect("Reactor::current must be called within a wstd runtime")
                .clone()
        })
    }

    /// Create a new instance of `Reactor`
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(InnerReactor {
                ready_list: Mutex::new(VecDeque::new()),
            }),
        }
    }

    /// Spawn a `Task` on the `Reactor`.
    pub fn spawn<F, T>(&self, fut: F) -> AsyncTask<T>
    where
        F: Future<Output = T> + 'static,
        T: 'static,
    {
        let this = self.clone();
        let schedule = move |runnable| this.inner.ready_list.lock().unwrap().push_back(runnable);

        // Safety: 'static constraints satisfy the lifetime requirements
        #[allow(unsafe_code)]
        let (runnable, task) = unsafe { async_task::spawn_unchecked(fut, schedule) };
        self.inner.ready_list.lock().unwrap().push_back(runnable);
        task
    }
}
