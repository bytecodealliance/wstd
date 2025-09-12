//! HTTP body types

use crate::http::fields::header_map_from_wasi;
use crate::io::{AsyncInputStream, AsyncOutputStream, AsyncRead, AsyncWrite, Cursor, Empty};
use crate::runtime::AsyncPollable;
use core::fmt;
use http::header::CONTENT_LENGTH;
use wasip2::http::types::IncomingBody as WasiIncomingBody;

#[cfg(feature = "json")]
use serde::de::DeserializeOwned;
#[cfg(feature = "json")]
use serde_json;

pub use super::{
    error::{Error, ErrorVariant},
    HeaderMap,
};

#[derive(Debug)]
pub(crate) enum BodyKind {
    Fixed(u64),
    Chunked,
}

impl BodyKind {
    pub(crate) fn from_headers(headers: &HeaderMap) -> Result<BodyKind, InvalidContentLength> {
        if let Some(value) = headers.get(CONTENT_LENGTH) {
            let content_length = std::str::from_utf8(value.as_ref())
                .unwrap()
                .parse::<u64>()
                .map_err(|_| InvalidContentLength)?;
            Ok(BodyKind::Fixed(content_length))
        } else {
            Ok(BodyKind::Chunked)
        }
    }
}

/// A trait representing an HTTP body.
#[doc(hidden)]
pub trait Body: AsyncRead {
    /// Returns the exact remaining length of the iterator, if known.
    fn len(&self) -> Option<usize>;

    /// Returns `true` if the body is known to be empty.
    fn is_empty(&self) -> bool {
        matches!(self.len(), Some(0))
    }
}

/// Conversion into a `Body`.
#[doc(hidden)]
pub trait IntoBody {
    /// What type of `Body` are we turning this into?
    type IntoBody: Body;
    /// Convert into `Body`.
    fn into_body(self) -> Self::IntoBody;
}
impl<T> IntoBody for T
where
    T: Body,
{
    type IntoBody = T;
    fn into_body(self) -> Self::IntoBody {
        self
    }
}

impl IntoBody for String {
    type IntoBody = BoundedBody<Vec<u8>>;
    fn into_body(self) -> Self::IntoBody {
        BoundedBody(Cursor::new(self.into_bytes()))
    }
}

impl IntoBody for &str {
    type IntoBody = BoundedBody<Vec<u8>>;
    fn into_body(self) -> Self::IntoBody {
        BoundedBody(Cursor::new(self.to_owned().into_bytes()))
    }
}

impl IntoBody for Vec<u8> {
    type IntoBody = BoundedBody<Vec<u8>>;
    fn into_body(self) -> Self::IntoBody {
        BoundedBody(Cursor::new(self))
    }
}

impl IntoBody for &[u8] {
    type IntoBody = BoundedBody<Vec<u8>>;
    fn into_body(self) -> Self::IntoBody {
        BoundedBody(Cursor::new(self.to_owned()))
    }
}

/// An HTTP body with a known length
#[derive(Debug)]
pub struct BoundedBody<T>(Cursor<T>);

impl<T: AsRef<[u8]>> AsyncRead for BoundedBody<T> {
    async fn read(&mut self, buf: &mut [u8]) -> crate::io::Result<usize> {
        self.0.read(buf).await
    }
}
impl<T: AsRef<[u8]>> Body for BoundedBody<T> {
    fn len(&self) -> Option<usize> {
        Some(self.0.get_ref().as_ref().len())
    }
}

/// An HTTP body with an unknown length
#[derive(Debug)]
pub struct StreamedBody<S: AsyncRead>(S);

impl<S: AsyncRead> StreamedBody<S> {
    /// Wrap an `AsyncRead` impl in a type that provides a [`Body`] implementation.
    pub fn new(s: S) -> Self {
        Self(s)
    }
}
impl<S: AsyncRead> AsyncRead for StreamedBody<S> {
    async fn read(&mut self, buf: &mut [u8]) -> crate::io::Result<usize> {
        self.0.read(buf).await
    }
}
impl<S: AsyncRead> Body for StreamedBody<S> {
    fn len(&self) -> Option<usize> {
        None
    }
}

impl Body for Empty {
    fn len(&self) -> Option<usize> {
        Some(0)
    }
}

/// An incoming HTTP body
#[derive(Debug)]
pub struct IncomingBody {
    kind: BodyKind,
    // IMPORTANT: the order of these fields here matters. `body_stream` must
    // be dropped before `incoming_body`.
    body_stream: AsyncInputStream,
    incoming_body: WasiIncomingBody,
}

impl IncomingBody {
    pub(crate) fn new(
        kind: BodyKind,
        body_stream: AsyncInputStream,
        incoming_body: WasiIncomingBody,
    ) -> Self {
        Self {
            kind,
            body_stream,
            incoming_body,
        }
    }

    /// Consume this `IncomingBody` and return the trailers, if present.
    pub async fn finish(self) -> Result<Option<HeaderMap>, Error> {
        // The stream is a child resource of the `IncomingBody`, so ensure that
        // it's dropped first.
        drop(self.body_stream);

        let trailers = WasiIncomingBody::finish(self.incoming_body);

        AsyncPollable::new(trailers.subscribe()).wait_for().await;

        let trailers = trailers.get().unwrap().unwrap()?;

        let trailers = match trailers {
            None => None,
            Some(trailers) => Some(header_map_from_wasi(trailers)?),
        };

        Ok(trailers)
    }

    /// Try to deserialize the incoming body as JSON. The optional
    /// `json` feature is required.
    ///
    /// Fails whenever the response body is not in JSON format,
    /// or it cannot be properly deserialized to target type `T`. For more
    /// details please see [`serde_json::from_reader`].
    ///
    /// [`serde_json::from_reader`]: https://docs.serde.rs/serde_json/fn.from_reader.html
    #[cfg(feature = "json")]
    pub async fn json<T: DeserializeOwned>(&mut self) -> Result<T, Error> {
        let buf = self.bytes().await?;
        serde_json::from_slice(&buf).map_err(|e| ErrorVariant::Other(e.to_string()).into())
    }

    /// Get the full response body as `Vec<u8>`.
    pub async fn bytes(&mut self) -> Result<Vec<u8>, Error> {
        let mut buf = match self.kind {
            BodyKind::Fixed(l) => {
                if l > (usize::MAX as u64) {
                    return Err(ErrorVariant::Other(
                        "incoming body is too large to allocate and buffer in memory".to_string(),
                    )
                    .into());
                } else {
                    Vec::with_capacity(l as usize)
                }
            }
            BodyKind::Chunked => Vec::with_capacity(4096),
        };
        self.read_to_end(&mut buf).await?;
        Ok(buf)
    }
}

impl AsyncRead for IncomingBody {
    async fn read(&mut self, out_buf: &mut [u8]) -> crate::io::Result<usize> {
        self.body_stream.read(out_buf).await
    }

    fn as_async_input_stream(&self) -> Option<&AsyncInputStream> {
        Some(&self.body_stream)
    }
}

impl Body for IncomingBody {
    fn len(&self) -> Option<usize> {
        match self.kind {
            BodyKind::Fixed(l) => {
                if l > (usize::MAX as u64) {
                    None
                } else {
                    Some(l as usize)
                }
            }
            BodyKind::Chunked => None,
        }
    }
}

#[derive(Debug)]
pub struct InvalidContentLength;

impl fmt::Display for InvalidContentLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "incoming content-length should be a u64; violates HTTP/1.1".fmt(f)
    }
}

impl std::error::Error for InvalidContentLength {}

impl From<InvalidContentLength> for Error {
    fn from(e: InvalidContentLength) -> Self {
        // TODO: What's the right error code here?
        ErrorVariant::Other(e.to_string()).into()
    }
}

/// The output stream for the body, implementing [`AsyncWrite`]. Call
/// [`Responder::start_response`] or [`Client::start_request`] to obtain
/// one. Once the body is complete, it must be declared finished, using
/// [`Finished::finish`], [`Finished::fail`], [`Client::finish`], or
/// [`Client::fail`].
///
/// [`Responder::start_response`]: crate::http::server::Responder::start_response
/// [`Client::start_request`]: crate::http::client::Client::start_request
/// [`Finished::finish`]: crate::http::server::Finished::finish
/// [`Finished::fail`]: crate::http::server::Finished::fail
/// [`Client::finish`]: crate::http::client::Client::finish
/// [`Client::fail`]: crate::http::client::Client::fail
#[must_use]
pub struct OutgoingBody {
    // IMPORTANT: the order of these fields here matters. `stream` must
    // be dropped before `body`.
    stream: AsyncOutputStream,
    body: wasip2::http::types::OutgoingBody,
    dontdrop: DontDropOutgoingBody,
}

impl OutgoingBody {
    pub(crate) fn new(stream: AsyncOutputStream, body: wasip2::http::types::OutgoingBody) -> Self {
        Self {
            stream,
            body,
            dontdrop: DontDropOutgoingBody,
        }
    }

    pub(crate) fn consume(self) -> (AsyncOutputStream, wasip2::http::types::OutgoingBody) {
        let Self {
            stream,
            body,
            dontdrop,
        } = self;

        std::mem::forget(dontdrop);

        (stream, body)
    }

    /// Return a reference to the underlying `AsyncOutputStream`.
    ///
    /// This usually isn't needed, as `OutgoingBody` implements `AsyncWrite`
    /// too, however it is useful for code that expects to work with
    /// `AsyncOutputStream` specifically.
    pub fn stream(&mut self) -> &mut AsyncOutputStream {
        &mut self.stream
    }
}

impl AsyncWrite for OutgoingBody {
    async fn write(&mut self, buf: &[u8]) -> crate::io::Result<usize> {
        self.stream.write(buf).await
    }

    async fn flush(&mut self) -> crate::io::Result<()> {
        self.stream.flush().await
    }

    fn as_async_output_stream(&self) -> Option<&AsyncOutputStream> {
        Some(&self.stream)
    }
}

/// A utility to ensure that `OutgoingBody` is either finished or failed, and
/// not implicitly dropped.
struct DontDropOutgoingBody;

impl Drop for DontDropOutgoingBody {
    fn drop(&mut self) {
        unreachable!("`OutgoingBody::drop` called; `OutgoingBody`s should be consumed with `finish` or `fail`.");
    }
}

/// A placeholder for use as the type parameter to [`Request`] and [`Response`]
/// to indicate that the body has not yet started. This is used with
/// [`Client::start_request`] and [`Responder::start_response`], which have
/// `Requeset<BodyForthcoming>` and `Response<BodyForthcoming>` arguments,
/// respectively.
///
/// To instead start the response and obtain the output stream for the body,
/// use [`Responder::respond`].
/// To instead send a request or response with an input stream for the body,
/// use [`Client::send`] or [`Responder::respond`].
///
/// [`Request`]: crate::http::Request
/// [`Response`]: crate::http::Response
/// [`Client::start_request`]: crate::http::Client::start_request
/// [`Responder::start_response`]: crate::http::server::Responder::start_response
/// [`Client::send`]: crate::http::Client::send
/// [`Responder::respond`]: crate::http::server::Responder::respond
pub struct BodyForthcoming;
