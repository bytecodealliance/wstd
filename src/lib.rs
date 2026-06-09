#![allow(async_fn_in_trait)]
#![warn(future_incompatible, unreachable_pub)]
#![deny(unsafe_code)]
//#![deny(missing_debug_implementations)]
//#![warn(missing_docs)]
//#![forbid(rustdoc::missing_doc_code_examples)]

//! An async standard library for Wasm Components and WASI 0.2
//!
//! This is a minimal async standard library written exclusively to support Wasm
//! Components. It exists primarily to enable people to write async-based
//! applications in Rust before async-std, smol, or tokio land support for Wasm
//! Components and WASI 0.2. Once those runtimes land support, it is recommended
//! users switch to use those instead.
//!
//! # Examples
//!
//! **TCP echo server**
//!
//! ```rust,no_run
#![doc = include_str!("../examples/tcp_echo_server.rs")]
//! ```
//!
//! **HTTP Client**
//!
//! ```rust,ignore
#![doc = include_str!("../tests/http_get.rs")]
//! ```
//!
//! **HTTP Server**
//!
//! ```rust,no_run
#![doc = include_str!("../examples/http_server.rs")]
//! ```
//!
//! # Design Decisions
//!
//! This library is entirely self-contained. This means that it does not share
//! any traits or types with any other async runtimes. This means we're trading
//! in some compatibility for ease of maintenance. Because this library is not
//! intended to be maintained in the long term, this seems like the right
//! tradeoff to make.
//!
//! WASI 0.2 does not yet support multi-threading. For that reason this library
//! does not provide any multi-threaded primitives, and is free to make liberal
//! use of Async Functions in Traits since no `Send` bounds are required. This
//! makes for a simpler end-user experience, again at the cost of some
//! compatibility. Though ultimately we do believe that using Async Functions is
//! the right foundation for the standard library abstractions - meaning we may
//! be trading in backward-compatibility for forward-compatibility.
//!
//! This library also supports slightly more interfaces than the stdlib does.
//! For example `wstd::rand` is a new module that provides access to random
//! bytes. And `wstd::runtime` provides access to async runtime primitives.
//! These are unique capabilities provided by WASI 0.2, and because this library
//! is specific to that are exposed from here.

#[allow(unreachable_pub)]
mod sys;

pub mod future;
#[macro_use]
pub mod http;
pub mod io;
pub mod iter;
pub mod net;
pub mod rand;
pub mod runtime;
pub mod task;
pub mod time;

pub use wstd_macro::attr_macro_http_server as http_server;
pub use wstd_macro::attr_macro_main as main;
pub use wstd_macro::attr_macro_test as test;

// Re-export the wasi bindings crate for use only by `wstd-macro` macros. The proc
// macros need to generate code that uses these definitions, but we don't want
// to treat it as part of our public API with regards to semver, so we keep it
// under `__internal` as well as doc(hidden) to indicate it is private.
#[cfg(wstd_p3)]
#[doc(hidden)]
pub mod __internal {
    pub use wasip3;
}

#[cfg(wstd_p2)]
#[doc(hidden)]
pub mod __internal {
    pub use wasip2;
}

// Conditionally-compiled declarative macro for the `#[wstd::main]` entry point.
//
// The `#[wstd::main]` proc macro delegates to this declarative macro so the
// `wstd_p2` / `wstd_p3` cfg aliases (defined in build.rs) are evaluated in
// wstd's own context. Consumers don't need to define any features themselves.
//
// p2: the standard bin `fn main` is lifted to a synchronous `wasi:cli/run` by
// the target's command adapter, and `block_on` drives the future to completion.
//
// p3: `block_on` cannot be used, because a synchronous `wasi:cli/run` task may
// not block on async-lowered imports (it traps with "cannot block a synchronous
// task before returning"). Instead we async-lift the export by implementing the
// async `wasi:cli/run` guest directly, mirroring `__http_server_export!`.

#[cfg(wstd_p2)]
#[macro_export]
#[doc(hidden)]
macro_rules! __main_export {
    (output { $($output:tt)* } run { $($run_fn:tt)* }) => {
        pub fn main() $($output)* {
            $($run_fn)*

            $crate::runtime::block_on(async { __run().await })
        }
    };
}

#[cfg(wstd_p3)]
#[macro_export]
#[doc(hidden)]
macro_rules! __main_export {
    (output { $($output:tt)* } run { $($run_fn:tt)* }) => {
        const _: () = {
            $($run_fn)*

            struct __WstdMain;

            impl $crate::__internal::wasip3::exports::cli::run::Guest for __WstdMain {
                async fn run() -> ::core::result::Result<(), ()> {
                    $crate::runtime::__finish_main(__run().await)
                }
            }

            $crate::__internal::wasip3::cli::command::export!(__WstdMain with_types_in $crate::__internal::wasip3);
        };

        // The bin target still requires a `fn main`; the real entry point is the
        // async-lifted `wasi:cli/run` export above, so this is never invoked.
        fn main() {}
    };
}

pub mod prelude {
    pub use crate::future::FutureExt as _;
    pub use crate::io::AsyncRead as _;
    pub use crate::io::AsyncWrite as _;
}
