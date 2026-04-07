use http::StatusCode;

use crate::http::body::{Body, BodyHint};
use crate::http::error::Error;
use crate::http::fields::{HeaderMap, header_map_from_wasi};

pub use http::response::{Builder, Response};


#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
use wasip2::http::types::IncomingResponse;

#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
pub(crate) fn try_from_incoming(incoming: IncomingResponse) -> Result<Response<Body>, Error> {
    let headers: HeaderMap = header_map_from_wasi(incoming.headers())?;
    let status = StatusCode::from_u16(incoming.status())
        .map_err(|err| anyhow::anyhow!("wasi provided invalid status code ({err})"))?;

    let hint = BodyHint::from_headers(&headers)?;
    let incoming_body = incoming
        .consume()
        .expect("cannot call `consume` twice on incoming response");
    let body = Body::from_incoming(incoming_body, hint);

    let mut builder = Response::builder().status(status);
    *builder.headers_mut().expect("builder has not errored") = headers;
    Ok(builder
        .body(body)
        .expect("response builder should not error"))
}


#[cfg(feature = "wasip3")]
use crate::http::error::ErrorCode;
#[cfg(feature = "wasip3")]
use wasip3::http::types::Response as WasiResponse;

#[cfg(feature = "wasip3")]
pub(crate) fn try_from_wasi_response(
    incoming: WasiResponse,
    completion: wit_bindgen::rt::async_support::FutureReader<Result<(), ErrorCode>>,
) -> Result<Response<Body>, Error> {
    let headers: HeaderMap = header_map_from_wasi(incoming.get_headers())?;
    let status = StatusCode::from_u16(incoming.get_status_code())
        .map_err(|err| anyhow::anyhow!("wasi provided invalid status code ({err})"))?;

    let hint = BodyHint::from_headers(&headers)?;

    // Consume the response body
    let (body_stream, _trailers_future) = WasiResponse::consume_body(incoming, completion);
    let body = Body::from_p3_stream(body_stream, hint);

    let mut builder = Response::builder().status(status);
    *builder.headers_mut().expect("builder has not errored") = headers;
    Ok(builder
        .body(body)
        .expect("response builder should not error"))
}
