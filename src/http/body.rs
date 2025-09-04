use crate::http::{
    error::Context as _,
    fields::{header_map_from_wasi, header_map_to_wasi},
    Error, HeaderMap,
};
use crate::io::{AsyncInputStream, AsyncOutputStream, AsyncWrite};
use crate::runtime::{AsyncPollable, Reactor, WaitFor};

pub use ::http_body::{Body as HttpBody, Frame, SizeHint};
pub use bytes::Bytes;

use http::header::CONTENT_LENGTH;
use http_body_util::{combinators::UnsyncBoxBody, BodyExt};
use std::fmt;
use std::future::{poll_fn, Future};
use std::pin::{pin, Pin};
use std::task::{Context, Poll};
use wasip2::http::types::{
    FutureTrailers, IncomingBody as WasiIncomingBody, OutgoingBody as WasiOutgoingBody,
};
use wasip2::io::streams::{InputStream as WasiInputStream, StreamError};

pub mod util {
    pub use http_body_util::*;
}

#[derive(Debug)]
pub struct Body(pub(crate) BodyInner);

#[derive(Debug)]
pub(crate) enum BodyInner {
    Boxed(UnsyncBoxBody<Bytes, Error>),
    Incoming(Incoming),
    Complete(Bytes),
}

impl Body {
    pub async fn send(self, outgoing_body: WasiOutgoingBody) -> Result<(), Error> {
        match self.0 {
            BodyInner::Incoming(incoming) => {
                let in_body = incoming.into_inner();
                let mut in_stream =
                    AsyncInputStream::new(in_body.stream().expect("incoming body already read"));
                let mut out_stream = AsyncOutputStream::new(
                    outgoing_body
                        .write()
                        .expect("outgoing body already written"),
                );
                crate::io::copy(&mut in_stream, &mut out_stream)
                    .await
                    .map_err(|e| {
                        Error::from(e)
                            .context("copying incoming body stream to outgoing body stream")
                    })?;
                drop(in_stream);
                drop(out_stream);
                let future_in_trailers = WasiIncomingBody::finish(in_body);
                Reactor::current()
                    .schedule(future_in_trailers.subscribe())
                    .wait_for()
                    .await;
                let in_trailers: Option<wasip2::http::types::Fields> = future_in_trailers
                    .get()
                    .expect("pollable ready")
                    .expect("got once")
                    .map_err(|e| Error::from(e).context("recieving incoming trailers"))?;
                WasiOutgoingBody::finish(outgoing_body, in_trailers)
                    .map_err(|e| Error::from(e).context("finishing outgoing body"))?;
                Ok(())
            }
            BodyInner::Boxed(box_body) => {
                let mut out_stream = AsyncOutputStream::new(
                    outgoing_body
                        .write()
                        .expect("outgoing body already written"),
                );
                let mut body = pin!(box_body);
                let mut trailers = None;
                loop {
                    match poll_fn(|cx| body.as_mut().poll_frame(cx)).await {
                        Some(Ok(frame)) if frame.is_data() => {
                            let data = frame.data_ref().unwrap();
                            out_stream.write_all(data).await?;
                        }
                        Some(Ok(frame)) if frame.is_trailers() => {
                            trailers =
                                Some(header_map_to_wasi(frame.trailers_ref().unwrap()).map_err(
                                    |e| Error::from(e).context("outoging trailers to wasi"),
                                )?);
                        }
                        Some(Err(err)) => break Err(err.context("sending outgoing body")),
                        None => {
                            drop(out_stream);
                            WasiOutgoingBody::finish(outgoing_body, trailers)
                                .map_err(|e| Error::from(e).context("finishing outgoing body"))?;
                            break Ok(());
                        }
                        _ => unreachable!(),
                    }
                }
            }
            BodyInner::Complete(bytes) => {
                let mut out_stream = AsyncOutputStream::new(
                    outgoing_body
                        .write()
                        .expect("outgoing body already written"),
                );
                out_stream.write_all(&bytes).await?;
                drop(out_stream);
                WasiOutgoingBody::finish(outgoing_body, None)
                    .map_err(|e| Error::from(e).context("finishing outgoing body"))?;
                Ok(())
            }
        }
    }

    pub fn into_boxed_body(self) -> UnsyncBoxBody<Bytes, Error> {
        match self.0 {
            BodyInner::Incoming(i) => i.into_http_body().boxed_unsync(),
            BodyInner::Complete(bytes) => http_body_util::Full::new(bytes)
                .map_err(annotate_err)
                .boxed_unsync(),
            BodyInner::Boxed(b) => b,
        }
    }

    pub fn as_boxed_body(&mut self) -> &mut UnsyncBoxBody<Bytes, Error> {
        let mut prev = Self::empty();
        std::mem::swap(self, &mut prev);
        self.0 = BodyInner::Boxed(prev.into_boxed_body());

        match &mut self.0 {
            BodyInner::Boxed(ref mut b) => b,
            _ => unreachable!(),
        }
    }

    pub async fn contents(&mut self) -> Result<&[u8], Error> {
        match &mut self.0 {
            BodyInner::Complete(ref bs) => Ok(bs.as_ref()),
            inner => {
                let mut prev = BodyInner::Complete(Bytes::new());
                std::mem::swap(inner, &mut prev);
                let boxed_body = match prev {
                    BodyInner::Incoming(i) => i.into_http_body().boxed_unsync(),
                    BodyInner::Boxed(b) => b,
                    BodyInner::Complete(_) => unreachable!(),
                };
                let collected = boxed_body.collect().await?;
                *inner = BodyInner::Complete(collected.to_bytes());
                Ok(match inner {
                    BodyInner::Complete(ref bs) => bs.as_ref(),
                    _ => unreachable!(),
                })
            }
        }
    }

    pub fn content_length(&self) -> Option<u64> {
        match &self.0 {
            BodyInner::Boxed(b) => b.size_hint().exact(),
            BodyInner::Complete(bs) => Some(bs.len() as u64),
            BodyInner::Incoming(i) => i.size_hint.content_length(),
        }
    }

    pub fn empty() -> Self {
        Body(BodyInner::Complete(Bytes::new()))
    }

    pub fn from_string(s: impl Into<String>) -> Self {
        let s = s.into();
        Body(BodyInner::Complete(Bytes::from_owner(s.into_bytes())))
    }

    pub async fn str_contents(&mut self) -> Result<&str, Error> {
        let bs = self.contents().await?;
        std::str::from_utf8(bs).context("decoding body contents as string")
    }

    pub fn from_bytes(b: impl Into<Bytes>) -> Self {
        let b = b.into();
        Body::from(http_body_util::Full::new(b))
    }

    #[cfg(feature = "json")]
    pub fn from_json<T: serde::Serialize>(data: &T) -> Result<Self, serde_json::Error> {
        Ok(Self::from_string(serde_json::to_string(data)?))
    }

    #[cfg(feature = "json")]
    pub async fn json<T: for<'a> serde::Deserialize<'a>>(&mut self) -> Result<T, Error> {
        let str = self.str_contents().await?;
        serde_json::from_str(str).context("decoding body contents as json")
    }

    pub fn from_input_stream(r: crate::io::AsyncInputStream) -> Self {
        use futures_lite::stream::StreamExt;
        Body(BodyInner::Boxed(http_body_util::BodyExt::boxed_unsync(
            http_body_util::StreamBody::new(r.into_stream().map(|res| {
                res.map(|bytevec| Frame::data(Bytes::from_owner(bytevec)))
                    .map_err(Into::into)
            })),
        )))
    }
}

fn annotate_err<E>(_: E) -> Error {
    unreachable!()
}

impl<B> From<B> for Body
where
    B: HttpBody + Send + 'static,
    <B as HttpBody>::Data: Into<Bytes>,
    <B as HttpBody>::Error: Into<Error>,
{
    fn from(http_body: B) -> Body {
        use util::BodyExt;
        Body(BodyInner::Boxed(
            http_body
                .map_frame(|f| f.map_data(Into::into))
                .map_err(Into::into)
                .boxed_unsync(),
        ))
    }
}

impl From<Incoming> for Body {
    fn from(incoming: Incoming) -> Body {
        Body(BodyInner::Incoming(incoming))
    }
}

#[derive(Debug)]
pub struct Incoming {
    body: WasiIncomingBody,
    size_hint: BodyHint,
}

impl Incoming {
    pub(crate) fn new(body: WasiIncomingBody, size_hint: BodyHint) -> Self {
        Self { body, size_hint }
    }
    /// Use with `http_body::Body` trait
    pub fn into_http_body(self) -> IncomingBody {
        IncomingBody::new(self.body, self.size_hint)
    }
    pub fn into_body(self) -> Body {
        self.into()
    }
    pub fn into_inner(self) -> WasiIncomingBody {
        self.body
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BodyHint {
    ContentLength(u64),
    Unknown,
}

impl BodyHint {
    pub fn from_headers(headers: &HeaderMap) -> Result<Self, InvalidContentLength> {
        if let Some(val) = headers.get(CONTENT_LENGTH) {
            let len = std::str::from_utf8(val.as_ref())
                .map_err(|_| InvalidContentLength)?
                .parse::<u64>()
                .map_err(|_| InvalidContentLength)?;
            Ok(BodyHint::ContentLength(len))
        } else {
            Ok(BodyHint::Unknown)
        }
    }
    fn content_length(&self) -> Option<u64> {
        match self {
            BodyHint::ContentLength(l) => Some(*l),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub struct InvalidContentLength;
impl fmt::Display for InvalidContentLength {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid Content-Length header")
    }
}
impl std::error::Error for InvalidContentLength {}

#[derive(Debug)]
pub struct IncomingBody {
    state: Option<Pin<Box<IncomingBodyState>>>,
    size_hint: BodyHint,
}

impl IncomingBody {
    fn new(body: WasiIncomingBody, size_hint: BodyHint) -> Self {
        Self {
            state: Some(Box::pin(IncomingBodyState::Body {
                read_state: BodyState {
                    wait: None,
                    subscription: None,
                    stream: body
                        .stream()
                        .expect("wasi incoming-body stream should not yet be taken"),
                },
                body: Some(body),
            })),
            size_hint,
        }
    }
}

impl HttpBody for IncomingBody {
    type Data = Bytes;
    type Error = Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        loop {
            let state = self.as_mut().state.take();
            if state.is_none() {
                return Poll::Ready(None);
            }
            let mut state = state.unwrap();
            match state.as_mut().project() {
                IBSProj::Body { read_state, body } => match read_state.poll_frame(cx) {
                    Poll::Pending => {
                        self.as_mut().state = Some(state);
                        return Poll::Pending;
                    }
                    Poll::Ready(Some(r)) => {
                        self.as_mut().state = Some(state);
                        return Poll::Ready(Some(r));
                    }
                    Poll::Ready(None) => {
                        // state contains children of the incoming-body. Must drop it
                        // in order to finish
                        let body = body.take().expect("finishing Body state");
                        drop(state);
                        let trailers_state = TrailersState::new(WasiIncomingBody::finish(body));
                        self.as_mut().state =
                            Some(Box::pin(IncomingBodyState::Trailers { trailers_state }));
                        continue;
                    }
                },
                IBSProj::Trailers { trailers_state } => match trailers_state.poll_frame(cx) {
                    Poll::Pending => {
                        self.as_mut().state = Some(state);
                        return Poll::Pending;
                    }
                    Poll::Ready(r) => return Poll::Ready(r),
                },
            }
        }
    }
    fn is_end_stream(&self) -> bool {
        self.state.is_none()
    }
    fn size_hint(&self) -> SizeHint {
        match self.size_hint {
            BodyHint::ContentLength(l) => SizeHint::with_exact(l),
            _ => Default::default(),
        }
    }
}

pin_project_lite::pin_project! {
    #[project = IBSProj]
    #[derive(Debug)]
    enum IncomingBodyState {
        Body {
            #[pin]
            read_state: BodyState,
            // body is Some until we need to remove it from a projection
            // during a state transition
            body: Option<WasiIncomingBody>
        },
        Trailers {
            #[pin]
            trailers_state: TrailersState
        },
    }
}

#[derive(Debug)]
struct BodyState {
    wait: Option<Pin<Box<WaitFor>>>,
    subscription: Option<AsyncPollable>,
    stream: WasiInputStream,
}

const MAX_FRAME_SIZE: u64 = 64 * 1024;

impl BodyState {
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Error>>> {
        loop {
            match self.stream.read(MAX_FRAME_SIZE) {
                Ok(bs) if !bs.is_empty() => {
                    return Poll::Ready(Some(Ok(Frame::data(Bytes::from(bs)))))
                }
                Err(StreamError::Closed) => return Poll::Ready(None),
                Err(StreamError::LastOperationFailed(err)) => {
                    return Poll::Ready(Some(Err(
                        Error::msg(err.to_debug_string()).context("reading incoming body stream")
                    )))
                }
                Ok(_empty) => {
                    if self.subscription.is_none() {
                        self.as_mut().subscription =
                            Some(Reactor::current().schedule(self.stream.subscribe()));
                    }
                    if self.wait.is_none() {
                        let wait = self.as_ref().subscription.as_ref().unwrap().wait_for();
                        self.as_mut().wait = Some(Box::pin(wait));
                    }
                    let mut taken_wait = self.as_mut().wait.take().unwrap();
                    match taken_wait.as_mut().poll(cx) {
                        Poll::Pending => {
                            self.as_mut().wait = Some(taken_wait);
                            return Poll::Pending;
                        }
                        // Its possible that, after returning ready, the
                        // stream does not actually provide any input. This
                        // behavior should only occur once.
                        Poll::Ready(()) => {
                            continue;
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
struct TrailersState {
    wait: Option<Pin<Box<WaitFor>>>,
    subscription: Option<AsyncPollable>,
    future_trailers: FutureTrailers,
}

impl TrailersState {
    fn new(future_trailers: FutureTrailers) -> Self {
        Self {
            wait: None,
            subscription: None,
            future_trailers,
        }
    }

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Error>>> {
        loop {
            if let Some(ready) = self.future_trailers.get() {
                return match ready {
                    Ok(Ok(Some(trailers))) => match header_map_from_wasi(trailers) {
                        Ok(header_map) => Poll::Ready(Some(Ok(Frame::trailers(header_map)))),
                        Err(e) => {
                            Poll::Ready(Some(Err(e.context("decoding incoming body trailers"))))
                        }
                    },
                    Ok(Ok(None)) => Poll::Ready(None),
                    Ok(Err(e)) => Poll::Ready(Some(Err(
                        Error::from(e).context("reading incoming body trailers")
                    ))),
                    Err(()) => unreachable!("future_trailers.get with some called at most once"),
                };
            }
            if self.subscription.is_none() {
                self.as_mut().subscription =
                    Some(Reactor::current().schedule(self.future_trailers.subscribe()));
            }
            if self.wait.is_none() {
                let wait = self.as_ref().subscription.as_ref().unwrap().wait_for();
                self.as_mut().wait = Some(Box::pin(wait));
            }
            let mut taken_wait = self.as_mut().wait.take().unwrap();
            match taken_wait.as_mut().poll(cx) {
                Poll::Pending => {
                    self.as_mut().wait = Some(taken_wait);
                    return Poll::Pending;
                }
                // Its possible that, after returning ready, the
                // future_trailers.get() does not actually provide any input. This
                // behavior should only occur once.
                Poll::Ready(()) => {
                    continue;
                }
            }
        }
    }
}
