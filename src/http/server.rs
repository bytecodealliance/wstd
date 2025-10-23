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

use super::{error::ErrorCode, fields::header_map_to_wasi, Body, Error, Response};
use http::header::CONTENT_LENGTH;
use wasip2::exports::http::incoming_handler::ResponseOutparam;
use wasip2::http::types::OutgoingResponse;

/// For use by the [`http_server`] macro only.
///
/// [`http_server`]: crate::http_server
#[doc(hidden)]
#[must_use]
pub struct Responder {
    outparam: ResponseOutparam,
}

impl Responder {
    /// This is used by the `http_server` macro.
    #[doc(hidden)]
    pub async fn respond<B: Into<Body>>(self, response: Response<B>) -> Result<(), Error> {
        let headers = response.headers();
        let status = response.status().as_u16();

        let wasi_headers = header_map_to_wasi(headers).expect("header error");

        // Consume the `response` and prepare to write the body.
        let body = response.into_body().into();

        // Automatically add a Content-Length header.
        if let Some(len) = body.content_length() {
            let mut buffer = itoa::Buffer::new();
            wasi_headers
                .append(CONTENT_LENGTH.as_str(), buffer.format(len).as_bytes())
                .unwrap();
        }

        let wasi_response = OutgoingResponse::new(wasi_headers);

        // Unwrap because `StatusCode` has already validated the status.
        wasi_response.set_status_code(status).unwrap();

        // Unwrap because we can be sure we only call these once.
        let wasi_body = wasi_response.body().unwrap();

        // Set the outparam to the response, which allows wasi-http to send
        // the response status and headers.
        ResponseOutparam::set(self.outparam, Ok(wasi_response));

        // Then send the body. The response will be fully sent once this
        // future is ready.
        body.send(wasi_body).await
    }

    /// This is used by the `http_server` macro.
    #[doc(hidden)]
    pub fn new(outparam: ResponseOutparam) -> Self {
        Self { outparam }
    }

    /// This is used by the `http_server` macro.
    #[doc(hidden)]
    pub fn fail(self, err: Error) -> Result<(), Error> {
        let e = match err.downcast_ref::<ErrorCode>() {
            Some(e) => e.clone(),
            None => ErrorCode::InternalError(Some(format!("{err:?}"))),
        };
        ResponseOutparam::set(self.outparam, Err(e));
        Err(err)
    }
}
