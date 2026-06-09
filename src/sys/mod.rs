//! Platform-specific backends.
//!
//! Each supported target provides an implementation under `src/sys/`, selected
//! here by a single `cfg-if`. The rest of the crate is written against the
//! backend-agnostic `crate::sys::*` re-exports below, mirroring the layout used
//! by the `polling` crate.
//!
//! # Backend contract
//!
//! The crate-root modules (`crate::time`, `crate::io`, `crate::net`, …) are
//! *facades*: target-agnostic code, free of `#[cfg]`, written once against the
//! items a backend promises to provide. A backend is a module under `src/sys/`
//! (today only [`p2`]; a `p3` backend is planned) that supplies the following
//! duck-typed items, in the `std::sys` style — there is no shared `trait`, the
//! facade simply names `crate::sys::<module>::<item>` and the compiler checks
//! that the selected backend provides it.
//!
//! - `sys::time`
//!   - `type MonotonicInstant` and `type MonotonicDuration`: nanosecond counts
//!     on the monotonic clock. The facade owns all `Duration`/`Instant`
//!     arithmetic, so these are plain integers (`u64` on p2).
//!   - `fn now() -> MonotonicInstant`: read the monotonic clock.
//!   - `struct Sleep`: a concrete `Future<Output = ()>` that resolves at a
//!     deadline. Backends may box internally (a native component-model future
//!     cannot always be named), so the facade makes no assumptions about
//!     `Sleep` beyond what it observes here.
//!   - `fn sleep_until(deadline: MonotonicInstant) -> Sleep`.
//!   - `struct SystemTime` with `fn now()` and `From<SystemTime> for
//!     std::time::SystemTime`.
//! - `sys::io`
//!   - `struct AsyncInputStream: AsyncRead` and
//!     `struct AsyncOutputStream: AsyncWrite`.
//!   - `Stdin`/`Stdout`/`Stderr` and `stdin()`/`stdout()`/`stderr()`.
//! - `sys::net`: `TcpStream`, `TcpListener`.
//! - `sys::http`: `client::Client`, plus the `request`/`response`/`body`/
//!   `fields`/`method`/`scheme`/`server` modules and error types.
//! - `sys::rand`: `get_random_bytes`, `get_insecure_random_bytes`.
//! - `sys::runtime`: `block_on`, `spawn`, `Reactor`, `Task`. The reified
//!   pollable types (`AsyncPollable`, `WaitFor`) are **p2-only**: they model
//!   WASI 0.2's `pollable` resources and have no portable equivalent, so they
//!   are intentionally left out of the common contract. While only one backend
//!   exists they are re-exported with no `#[cfg]`; once a second backend lands
//!   they become a single localized escape hatch rather than facade `#[cfg]`s.

cfg_if::cfg_if! {
    if #[cfg(all(target_os = "wasi", target_env = "p2"))] {
        mod p2;
        use p2 as backend;
    } else {
        compile_error!("unsupported target: wstd only compiles on `wasm32-wasip2`");
    }
}

pub use backend::*;

/// Compile-time assertions that the selected backend satisfies the parts of the
/// contract the facades rely on. These fail fast, with a clear message pointing
/// at the missing or mistyped item, instead of surfacing as a confusing error
/// deep inside a facade.
const _: fn() = || {
    fn assert_async_read<T: crate::io::AsyncRead>() {}
    fn assert_async_write<T: crate::io::AsyncWrite>() {}

    assert_async_read::<crate::sys::io::AsyncInputStream>();
    assert_async_write::<crate::sys::io::AsyncOutputStream>();
};
