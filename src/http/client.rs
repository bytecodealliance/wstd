use super::{
    body::{BodyForthcoming, IncomingBody, OutgoingBody},
    fields::header_map_to_wasi,
    Body, Error, HeaderMap, Request, Response, Result,
};
use crate::http::request::try_into_outgoing;
use crate::http::response::try_from_incoming;
use crate::io::{self, AsyncOutputStream, AsyncPollable};
use crate::runtime::WaitFor;
use crate::time::Duration;
use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use wasip2::http::types::{
    FutureIncomingResponse as WasiFutureIncomingResponse, OutgoingBody as WasiOutgoingBody,
    RequestOptions as WasiRequestOptions,
};

/// An HTTP client.
// Empty for now, but permits adding support for RequestOptions soon:
#[derive(Debug)]
pub struct Client {
    options: Option<RequestOptions>,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    /// Create a new instance of `Client`
    pub fn new() -> Self {
        Self { options: None }
    }

    /// Send an HTTP request.
    ///
    /// TODO: Should this automatically add a "Content-Length" header if the
    /// body size is known?
    ///
    /// To respond with trailers, use [`Client::start_request`] instead.
    pub async fn send<B: Body>(&self, req: Request<B>) -> Result<Response<IncomingBody>> {
        // We don't use `body::OutputBody` here because we can report I/O
        // errors from the `copy` directly.
        let (wasi_req, body) = try_into_outgoing(req)?;
        let wasi_body = wasi_req.body().unwrap();
        let wasi_stream = wasi_body.write().unwrap();

        // 1. Start sending the request head
        let res = wasip2::http::outgoing_handler::handle(wasi_req, self.wasi_options()?).unwrap();

        // 2. Start sending the request body
        io::copy(body, AsyncOutputStream::new(wasi_stream)).await?;

        // 3. Finish sending the request body
        let trailers = None;
        WasiOutgoingBody::finish(wasi_body, trailers).unwrap();

        // 4. Receive the response
        AsyncPollable::new(res.subscribe()).wait_for().await;

        // NOTE: the first `unwrap` is to ensure readiness, the second `unwrap`
        // is to trap if we try and get the response more than once. The final
        // `?` is to raise the actual error if there is one.
        let res = res.get().unwrap().unwrap()?;
        try_from_incoming(res)
    }

    /// Start sending an HTTP request, and return an `OutgoingBody` stream to
    /// write the body to.
    ///
    /// The returned `OutgoingBody` must be consumed by [`Client::finish`] or
    /// [`Client::fail`].
    pub async fn start_request(
        &self,
        req: Request<BodyForthcoming>,
    ) -> Result<(
        OutgoingBody,
        impl Future<Output = Result<Response<IncomingBody>>>,
    )> {
        let (wasi_req, _body_forthcoming) = try_into_outgoing(req)?;
        let wasi_body = wasi_req.body().unwrap();
        let wasi_stream = wasi_body.write().unwrap();

        // Start sending the request head.
        let res = wasip2::http::outgoing_handler::handle(wasi_req, self.wasi_options()?).unwrap();

        let outgoing_body = OutgoingBody::new(AsyncOutputStream::new(wasi_stream), wasi_body);

        pin_project! {
            #[must_use = "futures do nothing unless polled or .awaited"]
            struct IncomingResponseFuture {
                #[pin]
                subscription: WaitFor,
                wasi: WasiFutureIncomingResponse,
            }
        }
        impl Future for IncomingResponseFuture {
            type Output = Result<Response<IncomingBody>>;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let this = self.project();
                match this.subscription.poll(cx) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(()) => Poll::Ready(
                        this.wasi
                            .get()
                            .unwrap()
                            .unwrap()
                            .map_err(Error::from)
                            .and_then(try_from_incoming),
                    ),
                }
            }
        }

        let subscription = AsyncPollable::new(res.subscribe()).wait_for();
        let future = IncomingResponseFuture {
            subscription,
            wasi: res,
        };

        Ok((outgoing_body, future))
    }

    /// Finish the body, optionally with trailers.
    ///
    /// This is used with [`Client::start_request`].
    pub fn finish(body: OutgoingBody, trailers: Option<HeaderMap>) -> Result<()> {
        let (stream, body) = body.consume();

        // The stream is a child resource of the `OutgoingBody`, so ensure that
        // it's dropped first.
        drop(stream);

        let wasi_trailers = match trailers {
            Some(trailers) => Some(header_map_to_wasi(&trailers)?),
            None => None,
        };

        wasip2::http::types::OutgoingBody::finish(body, wasi_trailers)
            .expect("body length did not match Content-Length header value");
        Ok(())
    }

    /// Consume the `OutgoingBody` and indicate that the body was not
    /// completed.
    ///
    /// This is used with [`Client::start_request`].
    pub fn fail(body: OutgoingBody) {
        let (_stream, _body) = body.consume();
    }

    /// Set timeout on connecting to HTTP server
    pub fn set_connect_timeout(&mut self, d: impl Into<Duration>) {
        self.options_mut().connect_timeout = Some(d.into());
    }

    /// Set timeout on recieving first byte of the Response body
    pub fn set_first_byte_timeout(&mut self, d: impl Into<Duration>) {
        self.options_mut().first_byte_timeout = Some(d.into());
    }

    /// Set timeout on recieving subsequent chunks of bytes in the Response body stream
    pub fn set_between_bytes_timeout(&mut self, d: impl Into<Duration>) {
        self.options_mut().between_bytes_timeout = Some(d.into());
    }

    fn options_mut(&mut self) -> &mut RequestOptions {
        match &mut self.options {
            Some(o) => o,
            uninit => {
                *uninit = Some(Default::default());
                uninit.as_mut().unwrap()
            }
        }
    }

    fn wasi_options(&self) -> Result<Option<WasiRequestOptions>> {
        self.options
            .as_ref()
            .map(RequestOptions::to_wasi)
            .transpose()
    }
}

#[derive(Default, Debug)]
struct RequestOptions {
    connect_timeout: Option<Duration>,
    first_byte_timeout: Option<Duration>,
    between_bytes_timeout: Option<Duration>,
}

impl RequestOptions {
    fn to_wasi(&self) -> Result<WasiRequestOptions> {
        let wasi = WasiRequestOptions::new();
        if let Some(timeout) = self.connect_timeout {
            wasi.set_connect_timeout(Some(timeout.0)).map_err(|()| {
                Error::other("wasi-http implementation does not support connect timeout option")
            })?;
        }
        if let Some(timeout) = self.first_byte_timeout {
            wasi.set_first_byte_timeout(Some(timeout.0)).map_err(|()| {
                Error::other("wasi-http implementation does not support first byte timeout option")
            })?;
        }
        if let Some(timeout) = self.between_bytes_timeout {
            wasi.set_between_bytes_timeout(Some(timeout.0))
                .map_err(|()| {
                    Error::other(
                        "wasi-http implementation does not support between byte timeout option",
                    )
                })?;
        }
        Ok(wasi)
    }
}
