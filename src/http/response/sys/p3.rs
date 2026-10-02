use crate::http::body::Body;
use crate::http::error::{Error, ErrorCode};

pub use http::response::{Builder, Response};

pub(crate) fn try_from_incoming(
    incoming: wasip3::http::types::Response,
) -> Result<Response<Body>, Error> {
    let http_response = wasip3::http_compat::http_from_wasi_response(incoming)?;
    let hint = BodyHint::from_headers(http_response.headers())?;
    Ok(http_response.map(|b| Body::from_incoming(b, hint)))
}

/// Convert a wstd response into a WASI 0.3 response.
#[doc(hidden)]
pub fn try_into_outgoing<B>(response: Response<B>) -> Result<wasip3::http::types::Response, Error>
where
    B: Into<Body>,
{
    wasip3::http_compat::http_into_wasi_response(response.map(Into::into)).map_err(Into::into)
}

/// Convert an application error into a WASI HTTP error code.
#[doc(hidden)]
pub fn error_code(error: Error) -> ErrorCode {
    error
        .downcast_ref::<ErrorCode>()
        .cloned()
        .unwrap_or_else(|| ErrorCode::InternalError(Some(format!("{error:?}"))))
}
