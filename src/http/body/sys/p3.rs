use crate::http::{Error, HeaderMap, error::Context as _};

pub use ::http_body::{Body as HttpBody, Frame, SizeHint};
pub use bytes::Bytes;

use http::header::CONTENT_LENGTH;
use http_body_util::{BodyExt, combinators::UnsyncBoxBody};
use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

pub mod util {
    pub use http_body_util::*;
}

/// An HTTP body.
///
/// Bodies can be constructed from bytes, strings, streams, or any
/// [`http_body::Body`], and incoming WASI HTTP bodies use the same public type.
#[derive(Debug)]
pub struct Body(BodyInner);

#[derive(Debug)]
enum BodyInner {
    Boxed(UnsyncBoxBody<Bytes, Error>),
    Incoming {
        body: UnsyncBoxBody<Bytes, Error>,
        size_hint: BodyHint,
    },
    Complete {
        data: Bytes,
        trailers: Option<HeaderMap>,
    },
}

impl Body {
    /// Convert this body into an `http_body::Body` trait object.
    pub fn into_boxed_body(self) -> UnsyncBoxBody<Bytes, Error> {
        fn map_e(_: std::convert::Infallible) -> Error {
            unreachable!()
        }

        match self.0 {
            BodyInner::Complete { data, trailers } => http_body_util::Full::new(data)
                .map_err(map_e)
                .with_trailers(async move { Ok(trailers).transpose() })
                .boxed_unsync(),
            BodyInner::Boxed(body) => body,
            BodyInner::Incoming { body, .. } => body,
        }
    }

    /// Collect the entire contents of this body into memory.
    pub async fn contents(&mut self) -> Result<&[u8], Error> {
        match &mut self.0 {
            BodyInner::Complete { data, .. } => Ok(data),
            inner => {
                let previous = std::mem::replace(
                    inner,
                    BodyInner::Complete {
                        data: Bytes::new(),
                        trailers: None,
                    },
                );
                let body = match previous {
                    BodyInner::Boxed(body) | BodyInner::Incoming { body, .. } => body,
                    BodyInner::Complete { .. } => {
                        unreachable!("BodyInner::Complete case was already handled")
                    }
                };
                let collected = body.collect().await?;
                let trailers = collected.trailers().cloned();
                *inner = BodyInner::Complete {
                    data: collected.to_bytes(),
                    trailers,
                };
                let BodyInner::Complete { data, .. } = inner else {
                    unreachable!("State was just set to Complete")
                };
                Ok(data)
            }
        }
    }

    /// Collect the entire contents of this body as [`Bytes`].
    pub async fn bytes_contents(&mut self) -> Result<Bytes, Error> {
        self.contents().await?;
        let BodyInner::Complete { data, .. } = &self.0 else {
            unreachable!("Reading body was completed")
        };
        Ok(data.clone())
    }

    /// Return the body length when it is known.
    pub fn content_length(&self) -> Option<u64> {
        match &self.0 {
            BodyInner::Boxed(body) => body.size_hint().exact(),
            BodyInner::Incoming { size_hint, .. } => size_hint.content_length(),
            BodyInner::Complete { data, .. } => Some(data.len() as u64),
        }
    }

    /// Construct an empty body.
    pub fn empty() -> Self {
        Self(BodyInner::Complete {
            data: Bytes::new(),
            trailers: None,
        })
    }

    /// Collect the entire contents of this body as UTF-8.
    pub async fn str_contents(&mut self) -> Result<&str, Error> {
        std::str::from_utf8(self.contents().await?).context("decoding body contents as string")
    }

    /// Construct a body by serializing a value as JSON.
    #[cfg(feature = "json")]
    pub fn from_json<T: serde::Serialize>(data: &T) -> Result<Self, serde_json::Error> {
        Ok(Self::from(serde_json::to_vec(data)?))
    }

    /// Collect and deserialize this body as JSON.
    #[cfg(feature = "json")]
    pub async fn json<T: for<'a> serde::Deserialize<'a>>(&mut self) -> Result<T, Error> {
        serde_json::from_str(self.str_contents().await?).context("decoding body contents as json")
    }

    pub(crate) fn from_incoming<T>(
        body: wasip3::http_compat::IncomingBody<T>,
        size_hint: BodyHint,
    ) -> Self
    where
        T: wasip3::http_compat::IncomingMessage + Send + 'static,
    {
        Self(BodyInner::Incoming {
            body: body.map_err(Into::into).boxed_unsync(),
            size_hint,
        })
    }

    /// Construct a body from a stream of byte chunks.
    pub fn from_stream<S>(stream: S) -> Self
    where
        S: futures_lite::Stream + Send + 'static,
        S::Item: Into<Bytes>,
    {
        use futures_lite::StreamExt;
        Self::from_http_body(http_body_util::StreamBody::new(
            stream.map(|bytes| Ok::<_, Error>(Frame::data(bytes.into()))),
        ))
    }

    /// Construct a body from a fallible stream of byte chunks.
    pub fn from_try_stream<S, D, E>(stream: S) -> Self
    where
        S: futures_lite::Stream<Item = Result<D, E>> + Send + 'static,
        D: Into<Bytes>,
        E: std::error::Error + Send + Sync + 'static,
    {
        use futures_lite::StreamExt;
        Self::from_http_body(http_body_util::StreamBody::new(
            stream.map(|bytes| Ok::<_, Error>(Frame::data(bytes?.into()))),
        ))
    }

    /// Construct a body from an [`http_body::Body`].
    pub fn from_http_body<B>(body: B) -> Self
    where
        B: HttpBody + Send + 'static,
        B::Data: Into<Bytes>,
        B::Error: Into<Error>,
    {
        Self(BodyInner::Boxed(
            body.map_frame(|frame| frame.map_data(Into::into))
                .map_err(Into::into)
                .boxed_unsync(),
        ))
    }
}

impl HttpBody for Body {
    type Data = Bytes;
    type Error = Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match &mut self.get_mut().0 {
            BodyInner::Boxed(body) => Pin::new(body).poll_frame(cx),
            BodyInner::Incoming { body, .. } => Pin::new(body).poll_frame(cx),
            BodyInner::Complete { data, trailers } => {
                if !data.is_empty() {
                    Poll::Ready(Some(Ok(Frame::data(std::mem::take(data)))))
                } else if let Some(trailers) = trailers.take() {
                    Poll::Ready(Some(Ok(Frame::trailers(trailers))))
                } else {
                    Poll::Ready(None)
                }
            }
        }
    }
}

impl From<()> for Body {
    fn from(_: ()) -> Self {
        Self::empty()
    }
}
impl From<&[u8]> for Body {
    fn from(bytes: &[u8]) -> Self {
        Self::from(bytes.to_owned())
    }
}
impl From<Vec<u8>> for Body {
    fn from(bytes: Vec<u8>) -> Self {
        Self::from(Bytes::from(bytes))
    }
}
impl From<Bytes> for Body {
    fn from(data: Bytes) -> Self {
        Self(BodyInner::Complete {
            data,
            trailers: None,
        })
    }
}
impl From<&str> for Body {
    fn from(data: &str) -> Self {
        Self::from(data.as_bytes())
    }
}
impl From<String> for Body {
    fn from(data: String) -> Self {
        Self::from(data.into_bytes())
    }
}

impl From<crate::io::AsyncInputStream> for Body {
    fn from(stream: crate::io::AsyncInputStream) -> Self {
        use futures_lite::StreamExt;
        Self::from_http_body(http_body_util::StreamBody::new(stream.into_stream().map(
            |result| {
                result
                    .map(|bytes| Frame::data(Bytes::from(bytes)))
                    .map_err(Error::from)
            },
        )))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BodyHint {
    ContentLength(u64),
    Unknown,
}

impl BodyHint {
    pub fn from_headers(headers: &HeaderMap) -> Result<Self, InvalidContentLength> {
        match headers.get(CONTENT_LENGTH) {
            Some(value) => Ok(Self::ContentLength(
                std::str::from_utf8(value.as_ref())
                    .map_err(|_| InvalidContentLength)?
                    .parse()
                    .map_err(|_| InvalidContentLength)?,
            )),
            None => Ok(Self::Unknown),
        }
    }

    fn content_length(self) -> Option<u64> {
        match self {
            Self::ContentLength(length) => Some(length),
            Self::Unknown => None,
        }
    }
}

#[derive(Debug)]
pub struct InvalidContentLength;

impl fmt::Display for InvalidContentLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid Content-Length header")
    }
}

impl std::error::Error for InvalidContentLength {}
