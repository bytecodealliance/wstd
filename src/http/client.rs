use super::{Body, Error, Request, Response};
use crate::time::Duration;

/// An HTTP client.
#[derive(Debug, Clone)]
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
    #[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
    pub async fn send<B: Into<Body>>(&self, req: Request<B>) -> Result<Response<Body>, Error> {
        use crate::http::request::try_into_outgoing;
        use crate::http::response::try_from_incoming;
        use crate::io::AsyncPollable;
        let (wasi_req, body) = try_into_outgoing(req)?;
        let body = body.into();
        let wasi_body = wasi_req.body().unwrap();

        let res = wasip2::http::outgoing_handler::handle(wasi_req, self.wasi_options_p2()?)?;

        let ((), body) =
            futures_lite::future::try_zip(async move { body.send(wasi_body).await }, async move {
                AsyncPollable::new(res.subscribe()).wait_for().await;
                let res = res.get().unwrap().unwrap()?;
                try_from_incoming(res)
            })
            .await?;
        Ok(body)
    }

    /// Send an HTTP request.
    #[cfg(feature = "wasip3")]
    pub async fn send<B: Into<Body>>(&self, req: Request<B>) -> Result<Response<Body>, Error> {
        use crate::http::request::try_into_wasi_request;
        use crate::http::response::try_from_wasi_response;

        let parts = try_into_wasi_request(req, self.options.as_ref())?;

        // Send body data through the stream writer
        if let Some(mut body_writer) = parts.body_writer {
            let mut body = parts.body;
            let body_bytes = body.contents().await?;
            if !body_bytes.is_empty() {
                let remaining = body_writer.write_all(body_bytes.to_vec()).await;
                if !remaining.is_empty() {
                    return Err(anyhow::anyhow!("failed to write full request body"));
                }
            }
            drop(body_writer);
        }

        let wasi_resp = wasip3::http::client::send(parts.request).await?;

        // Create a completion future for consuming the response body
        let (_completion_writer, completion_reader) =
            wasip3::wit_future::new::<Result<(), super::ErrorCode>>(|| Ok(()));
        drop(_completion_writer);

        try_from_wasi_response(wasi_resp, completion_reader)
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
                *uninit = Some(RequestOptions::default());
                uninit.as_mut().unwrap()
            }
        }
    }

    #[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
    fn wasi_options_p2(
        &self,
    ) -> Result<Option<wasip2::http::types::RequestOptions>, crate::http::Error> {
        self.options
            .as_ref()
            .map(RequestOptions::to_wasi_p2)
            .transpose()
    }
}

#[cfg(feature = "wasip3")]
pub(crate) type P3RequestOptions = RequestOptions;

#[derive(Default, Debug, Clone)]
pub(crate) struct RequestOptions {
    pub(crate) connect_timeout: Option<Duration>,
    pub(crate) first_byte_timeout: Option<Duration>,
    pub(crate) between_bytes_timeout: Option<Duration>,
}

impl RequestOptions {
    #[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
    fn to_wasi_p2(&self) -> Result<wasip2::http::types::RequestOptions, crate::http::Error> {
        let wasi = wasip2::http::types::RequestOptions::new();
        if let Some(timeout) = self.connect_timeout {
            wasi.set_connect_timeout(Some(timeout.0)).map_err(|()| {
                anyhow::Error::msg(
                    "wasi-http implementation does not support connect timeout option",
                )
            })?;
        }
        if let Some(timeout) = self.first_byte_timeout {
            wasi.set_first_byte_timeout(Some(timeout.0)).map_err(|()| {
                anyhow::Error::msg(
                    "wasi-http implementation does not support first byte timeout option",
                )
            })?;
        }
        if let Some(timeout) = self.between_bytes_timeout {
            wasi.set_between_bytes_timeout(Some(timeout.0))
                .map_err(|()| {
                    anyhow::Error::msg(
                        "wasi-http implementation does not support between byte timeout option",
                    )
                })?;
        }
        Ok(wasi)
    }
}
