use http::StatusCode;
use wasip2::http::types::IncomingResponse;

use crate::http::body::{BodyHint, Incoming};
use crate::http::error::{Context, Error};
use crate::http::fields::{header_map_from_wasi, HeaderMap};

pub use http::response::{Builder, Response};

pub(crate) fn try_from_incoming(incoming: IncomingResponse) -> Result<Response<Incoming>, Error> {
    let headers: HeaderMap = header_map_from_wasi(incoming.headers())?;
    // TODO: Does WASI guarantee that the incoming status is valid?
    let status = StatusCode::from_u16(incoming.status())
        .map_err(|err| anyhow::anyhow!("wasi provided invalid status code ({err})"))?;

    let hint = BodyHint::from_headers(&headers)?;
    // `body_stream` is a child of `incoming_body` which means we cannot
    // drop the parent before we drop the child
    let incoming_body = incoming
        .consume()
        .expect("cannot call `consume` twice on incoming response");
    let body = Incoming::new(incoming_body, hint);

    let mut builder = Response::builder().status(status);

    if let Some(headers_mut) = builder.headers_mut() {
        *headers_mut = headers;
    }

    builder.body(body).context("building response")
}
