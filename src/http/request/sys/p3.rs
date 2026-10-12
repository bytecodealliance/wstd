use crate::http::{
    body::{Body, BodyHint},
    error::Error,
};

pub use http::request::{Builder, Request};

// TODO: go back and add json stuff???

pub(crate) fn try_into_outgoing<T>(
    request: Request<T>,
) -> Result<wasip3::http::types::Request, Error>
where
    T: http_body::Body + std::any::Any,
    T::Data: Into<Vec<u8>>,
    T::Error: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
{
    wasip3::http_compat::http_into_wasi_request(request).map_err(Into::into)
}

/// Convert an incoming WASI HTTP request into a wstd request.
#[doc(hidden)]
pub fn try_from_incoming(incoming: wasip3::http::types::Request) -> Result<Request<Body>, Error> {
    let request = wasip3::http_compat::http_from_wasi_request(incoming)?;
    let hint = BodyHint::from_headers(request.headers())?;
    Ok(request.map(|body| Body::from_incoming(body, hint)))
}
