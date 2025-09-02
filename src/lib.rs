#![allow(async_fn_in_trait)]
#![warn(future_incompatible, unreachable_pub)]
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

// We need unsafe code in the runtime.
pub mod runtime;

// All other mods do not require unsafe.
#[forbid(unsafe_code)]
pub mod future;
#[forbid(unsafe_code)]
#[macro_use]
pub mod http;
#[forbid(unsafe_code)]
pub mod io;
#[forbid(unsafe_code)]
pub mod iter;
#[forbid(unsafe_code)]
pub mod net;
#[forbid(unsafe_code)]
pub mod rand;
#[forbid(unsafe_code)]
pub mod task;
#[forbid(unsafe_code)]
pub mod time;

pub use wstd_macro::attr_macro_http_server as http_server;
pub use wstd_macro::attr_macro_main as main;
pub use wstd_macro::attr_macro_test as test;

// Re-export the wasi crate for use by the `http_server` macro.
#[doc(hidden)]
pub use wasi;

pub mod prelude {
    pub use crate::future::FutureExt as _;
    pub use crate::http::Body as _;
    pub use crate::io::AsyncRead as _;
    pub use crate::io::AsyncWrite as _;
}
