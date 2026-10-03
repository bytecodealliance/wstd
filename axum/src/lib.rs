//! Support for the [`axum`] web server framework in wasi-http components, via
//! [`wstd`].
//!
//! This crate is a pretty thin wrapper on [`wstd`] that allows users to
//! use the [`axum`] crate on top of wstd's http support. This means that
//! axum services can run anywhere the [wasi-http proxy world] is supported,
//! e.g. in [`wasmtime serve`].
//!
//! Users of this crate should depend on `axum` with `default-features =
//! false`, and opt in to any features that they require (e.g. form, json,
//! matched-path, original-uri, query, tower-log, tracing). The axum crate
//! features that require `hyper` or `tokio` are NOT supported (e.g. http1,
//! http2, ws), because unlike in native applications, wasi-http components
//! have an http implementation provided as imported interfaces (i.e.
//! implemented the Wasm host), and do not use raw sockets inside of this
//! program.
//!
//! # Examples
//!
//! The simplest use is via the `wstd_axum::http_server` proc macro.
//! This macro can be applied to a sync or `async` `fn main` which returns
//! an impl of the `tower_service::Service` trait, typically an
//! `axum::Router`:
//!
//! ```rust,no_run
#![doc = include_str!("../examples/hello_world.rs")]
//! ```
//!
//! If users desire, they can instead use a `wstd::http_server` entry point
//! and then use `wstd_axum::serve` directly. The following is equivelant
//! to the above example:
//!
//! ```rust,no_run
#![doc = include_str!("../examples/hello_world_nomacro.rs")]
//! ```
//!
//! [`axum`]: https://docs.rs/axum/latest/axum/
//! [`wstd`]: https://docs.rs/wstd/latest/wstd/
//! [wasi-http proxy world]: https://github.com/WebAssembly/wasi-http
//! [`wasmtime serve`]: https://wasmtime.dev/

use axum::extract::Request;
use axum::response::Response;
use std::convert::Infallible;
use tower_service::Service;

pub use wstd_axum_macro::attr_macro_http_server as http_server;

pub async fn serve<S>(
    request: wstd::http::Request<wstd::http::Body>,
    mut service: S,
) -> wstd::http::error::Result<wstd::http::Response<wstd::http::Body>>
where
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
{
    let resp = service
        .call(
            request.map(|incoming: wstd::http::Body| -> axum::body::Body {
                axum::body::Body::new(incoming.into_boxed_body())
            }),
        )
        .await
        .unwrap_or_else(|err| match err {});
    Ok(resp.map(|body: axum::body::Body| -> wstd::http::Body {
        wstd::http::Body::from_http_body(body)
    }))
}
