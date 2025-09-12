//! HTTP servers
//!
//! The WASI HTTP server API uses the [typed main] idiom, with a `main` function
//! that takes a [`Request`] and a [`Responder`], and responds with a [`Response`],
//! using the [`http_server`] macro:
//!
//! ```no_run
//! # use wstd::http::{Request, Response, IntoBody, server::{Finished, Responder}, body::IncomingBody};
//! #[wstd::http_server]
//! async fn main(request: Request<IncomingBody>, responder: Responder) -> Finished {
//!     responder
//!         .respond(Response::new("Hello!\n".into_body()))
//!         .await
//! }
//! ```
//!
//! [typed main]: https://sunfishcode.github.io/typed-main-wasi-presentation/chapter_1.html
//! [`Request`]: crate::http::Request
//! [`Responder`]: crate::http::server::Responder
//! [`Response`]: crate::http::Response
//! [`http_server`]: crate::http_server

use super::{
    body::{BodyForthcoming, OutgoingBody},
    error::WasiHttpErrorCode,
    fields::header_map_to_wasi,
    Body, HeaderMap, Response,
};
use crate::io::{copy, AsyncOutputStream};
use http::header::CONTENT_LENGTH;
use wasip2::exports::http::incoming_handler::ResponseOutparam;
use wasip2::http::types::OutgoingResponse;

/// This is passed into the [`http_server`] `main` function and holds the state
/// needed for a handler to produce a response, or fail. There are two ways to
/// respond, with [`Responder::start_response`] to stream the body in, or
/// [`Responder::respond`] to give the body as a string, byte array, or input
/// stream. See those functions for examples.
///
/// [`http_server`]: crate::http_server
#[must_use]
pub struct Responder {
    outparam: ResponseOutparam,
}

impl Responder {
    /// Start responding with the given `Response` and return an `OutgoingBody`
    /// stream to write the body to.
    ///
    /// # Example
    ///
    /// ```
    /// # use wstd::http::{body::{IncomingBody, BodyForthcoming}, Response, Request};
    /// # use wstd::http::server::{Finished, Responder};
    /// # use crate::wstd::io::AsyncWrite;
    /// # async fn example(responder: Responder) -> Finished {
    ///     let mut body = responder.start_response(Response::new(BodyForthcoming));
    ///     let result = body
    ///         .write_all("Hello!\n".as_bytes())
    ///         .await;
    ///     Finished::finish(body, result, None)
    /// # }
    /// # fn main() {}
    /// ```
    pub fn start_response(self, response: Response<BodyForthcoming>) -> OutgoingBody {
        let wasi_headers = header_map_to_wasi(response.headers()).expect("header error");
        let wasi_response = OutgoingResponse::new(wasi_headers);
        let wasi_status = response.status().as_u16();

        // Unwrap because `StatusCode` has already validated the status.
        wasi_response.set_status_code(wasi_status).unwrap();

        // Unwrap because we can be sure we only call these once.
        let wasi_body = wasi_response.body().unwrap();
        let wasi_stream = wasi_body.write().unwrap();

        // Tell WASI to start the show.
        ResponseOutparam::set(self.outparam, Ok(wasi_response));

        OutgoingBody::new(AsyncOutputStream::new(wasi_stream), wasi_body)
    }

    /// Respond with the given `Response` which contains the body.
    ///
    /// If the body has a known length, a Content-Length header is automatically added.
    ///
    /// To respond with trailers, use [`Responder::start_response`] instead.
    ///
    /// # Example
    ///
    /// ```
    /// # use wstd::http::{body::{IncomingBody, BodyForthcoming}, IntoBody, Response, Request};
    /// # use wstd::http::server::{Finished, Responder};
    /// #
    /// # async fn example(responder: Responder) -> Finished {
    ///     responder
    ///         .respond(Response::new("Hello!\n".into_body()))
    ///         .await
    /// # }
    /// # fn main() {}
    /// ```
    pub async fn respond<B: Body>(self, response: Response<B>) -> Finished {
        let headers = response.headers();
        let status = response.status().as_u16();

        let wasi_headers = header_map_to_wasi(headers).expect("header error");

        // Consume the `response` and prepare to write the body.
        let mut body = response.into_body();

        // Automatically add a Content-Length header.
        if let Some(len) = body.len() {
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
        let wasi_stream = wasi_body.write().unwrap();

        // Tell WASI to start the show.
        ResponseOutparam::set(self.outparam, Ok(wasi_response));

        let mut outgoing_body = OutgoingBody::new(AsyncOutputStream::new(wasi_stream), wasi_body);

        let result = copy(&mut body, &mut outgoing_body).await;
        let trailers = None;
        Finished::finish(outgoing_body, result, trailers)
    }

    /// This is used by the `http_server` macro.
    #[doc(hidden)]
    pub fn new(outparam: ResponseOutparam) -> Self {
        Self { outparam }
    }

    /// This is used by the `http_server` macro.
    #[doc(hidden)]
    pub fn fail(self, err: WasiHttpErrorCode) -> Finished {
        ResponseOutparam::set(self.outparam, Err(err));
        Finished(())
    }
}

/// An opaque value returned from a handler indicating that the body is
/// finished, either by [`Finished::finish`] or [`Finished::fail`].
pub struct Finished(pub(crate) ());

impl Finished {
    /// Finish the body, optionally with trailers, and return a `Finished`
    /// token to be returned from the [`http_server`] `main` function to indicate
    /// that the response is finished.
    ///
    /// `result` is a `std::io::Result` for reporting any I/O errors that
    /// occur while writing to the body stream.
    ///
    /// [`http_server`]: crate::http_server
    pub fn finish(
        body: OutgoingBody,
        result: std::io::Result<()>,
        trailers: Option<HeaderMap>,
    ) -> Self {
        let (stream, body) = body.consume();

        // The stream is a child resource of the `OutgoingBody`, so ensure that
        // it's dropped first.
        drop(stream);

        // If there was an I/O error, panic and don't call `OutgoingBody::finish`.
        result.expect("I/O error while writing the body");

        let wasi_trailers =
            trailers.map(|trailers| header_map_to_wasi(&trailers).expect("header error"));

        wasip2::http::types::OutgoingBody::finish(body, wasi_trailers)
            .expect("body length did not match Content-Length header value");

        Self(())
    }

    /// Return a `Finished` token that can be returned from a handler to
    /// indicate that the body is not finished and should be considered
    /// corrupted.
    pub fn fail(body: OutgoingBody) -> Self {
        let (stream, _body) = body.consume();

        // The stream is a child resource of the `OutgoingBody`, so ensure that
        // it's dropped first.
        drop(stream);

        // No need to do anything else; omitting the call to `finish` achieves
        // the desired effect.
        Self(())
    }
}
