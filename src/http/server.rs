//! HTTP servers
//!
//! The WASI HTTP server uses the [typed main] idiom, with a `main` function
//! that takes a [`Request`] and succeeds with a [`Response`], using the
//! [`http_server`] macro:
//!
//! ```no_run
//! use wstd::http::{Request, Response, Body, Error};
//! #[wstd::http_server]
//! async fn main(_request: Request<Body>) -> Result<Response<Body>, Error> {
//!     Ok(Response::new("Hello!\n".into()))
//! }
//! ```
//!
//! [typed main]: https://sunfishcode.github.io/typed-main-wasi-presentation/chapter_1.html
//! [`Request`]: crate::http::Request
//! [`Responder`]: crate::http::server::Responder
//! [`Response`]: crate::http::Response
//! [`http_server`]: crate::http_server

pub use crate::sys::http::server::*;
