use wasip2::http::types::IncomingResponse;

use super::{
    body::{BodyKind, IncomingBody},
    fields::header_map_from_wasi,
    Error, HeaderMap,
};
use crate::io::AsyncInputStream;
use http::StatusCode;

pub use http::response::{Builder, Response};

pub(crate) fn try_from_incoming(
    incoming: IncomingResponse,
) -> Result<Response<IncomingBody>, Error> {
    let headers: HeaderMap = header_map_from_wasi(incoming.headers())?;
    // TODO: Does WASI guarantee that the incoming status is valid?
    let status =
        StatusCode::from_u16(incoming.status()).map_err(|err| Error::other(err.to_string()))?;

    let kind = BodyKind::from_headers(&headers)?;
    // `body_stream` is a child of `incoming_body` which means we cannot
    // drop the parent before we drop the child
    let incoming_body = incoming
        .consume()
        .expect("cannot call `consume` twice on incoming response");
    let body_stream = incoming_body
        .stream()
        .expect("cannot call `stream` twice on an incoming body");

    let body = IncomingBody::new(kind, AsyncInputStream::new(body_stream), incoming_body);

    let mut builder = Response::builder().status(status);

    if let Some(headers_mut) = builder.headers_mut() {
        *headers_mut = headers;
    }

    builder
        .body(body)
        .map_err(|err| Error::other(err.to_string()))
}
