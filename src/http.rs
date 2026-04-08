//! HTTP networking support

pub use http::status::StatusCode;
pub use http::uri::{Authority, PathAndQuery, Uri};

#[doc(inline)]
pub use body::{Body, util::BodyExt};
pub use crate::sys::http::client::Client;
pub use error::{Error, ErrorCode, Result};
pub use crate::sys::http::fields::{HeaderMap, HeaderName, HeaderValue};
pub use crate::sys::http::method::Method;
pub use request::Request;
pub use response::Response;
pub use crate::sys::http::scheme::{InvalidUri, Scheme};

pub mod body {
    //! HTTP body types.
    pub use crate::sys::http::body::*;
}

pub mod error {
    //! The http portion of wstd uses `anyhow::Error` as its `Error` type.
    //!
    //! There are various concrete error types

    pub use crate::http::body::InvalidContentLength;
    pub use anyhow::Context;
    pub use http::header::{InvalidHeaderName, InvalidHeaderValue};
    pub use http::method::InvalidMethod;
    pub use crate::sys::http::{ErrorCode, HeaderError};

    pub type Error = anyhow::Error;
    /// The `http` result type.
    pub type Result<T> = std::result::Result<T, Error>;
}

pub mod request {
    //! HTTP request types.
    pub use crate::sys::http::request::*;
}

pub mod response {
    //! HTTP response types.
    pub use crate::sys::http::response::*;
}

pub mod server {
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
}
