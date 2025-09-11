use super::{
    body::{BodyKind, IncomingBody},
    error::WasiHttpErrorCode,
    fields::{header_map_from_wasi, header_map_to_wasi},
    method::{from_wasi_method, to_wasi_method},
    scheme::{from_wasi_scheme, to_wasi_scheme},
    Authority, Error, HeaderMap, PathAndQuery, Uri,
};
use crate::io::AsyncInputStream;
use wasip2::http::outgoing_handler::OutgoingRequest;
use wasip2::http::types::IncomingRequest;

pub use http::request::{Builder, Request};

#[cfg(feature = "json")]
use super::{
    body::{BoundedBody, IntoBody},
    error::ErrorVariant,
};
#[cfg(feature = "json")]
use http::header::{HeaderValue, CONTENT_TYPE};
#[cfg(feature = "json")]
use serde::Serialize;
#[cfg(feature = "json")]
use serde_json;

#[cfg(feature = "json")]
pub trait JsonRequest {
    fn json<T: Serialize + ?Sized>(self, json: &T) -> Result<Request<BoundedBody<Vec<u8>>>, Error>;
}

#[cfg(feature = "json")]
impl JsonRequest for Builder {
    /// Send a JSON body. Requires optional `json` feature.
    ///
    /// Serialization can fail if `T`'s implementation of `Serialize` decides to
    /// fail.
    #[cfg(feature = "json")]
    fn json<T: Serialize + ?Sized>(self, json: &T) -> Result<Request<BoundedBody<Vec<u8>>>, Error> {
        let encoded = serde_json::to_vec(json).map_err(|e| ErrorVariant::Other(e.to_string()))?;
        let builder = if !self
            .headers_ref()
            .is_some_and(|headers| headers.contains_key(CONTENT_TYPE))
        {
            self.header(
                CONTENT_TYPE,
                HeaderValue::from_static("application/json; charset=utf-8"),
            )
        } else {
            self
        };
        builder
            .body(encoded.into_body())
            .map_err(|e| ErrorVariant::Other(e.to_string()).into())
    }
}

pub(crate) fn try_into_outgoing<T>(request: Request<T>) -> Result<(OutgoingRequest, T), Error> {
    let wasi_req = OutgoingRequest::new(header_map_to_wasi(request.headers())?);

    let (parts, body) = request.into_parts();

    // Set the HTTP method
    let method = to_wasi_method(parts.method);
    wasi_req
        .set_method(&method)
        .map_err(|()| Error::other(format!("method rejected by wasi-http: {method:?}",)))?;

    // Set the url scheme
    let scheme = parts
        .uri
        .scheme()
        .map(to_wasi_scheme)
        .unwrap_or(wasip2::http::types::Scheme::Https);
    wasi_req
        .set_scheme(Some(&scheme))
        .map_err(|()| Error::other(format!("scheme rejected by wasi-http: {scheme:?}")))?;

    // Set authority
    let authority = parts.uri.authority().map(Authority::as_str);
    wasi_req
        .set_authority(authority)
        .map_err(|()| Error::other(format!("authority rejected by wasi-http {authority:?}")))?;

    // Set the url path + query string
    if let Some(p_and_q) = parts.uri.path_and_query() {
        wasi_req
            .set_path_with_query(Some(p_and_q.as_str()))
            .map_err(|()| {
                Error::other(format!("path and query rejected by wasi-http {p_and_q:?}"))
            })?;
    }

    // All done; request is ready for send-off
    Ok((wasi_req, body))
}

/// This is used by the `http_server` macro.
#[doc(hidden)]
pub fn try_from_incoming(
    incoming: IncomingRequest,
) -> Result<Request<IncomingBody>, WasiHttpErrorCode> {
    // TODO: What's the right error code to use for invalid headers?
    let headers: HeaderMap = header_map_from_wasi(incoming.headers())
        .map_err(|e| WasiHttpErrorCode::InternalError(Some(e.to_string())))?;

    let method = from_wasi_method(incoming.method())
        .map_err(|_| WasiHttpErrorCode::HttpRequestMethodInvalid)?;
    let scheme = incoming.scheme().map(|scheme| {
        from_wasi_scheme(scheme).expect("TODO: what shall we do with an invalid uri here?")
    });
    let authority = incoming.authority().map(|authority| {
        Authority::from_maybe_shared(authority)
            .expect("TODO: what shall we do with an invalid uri authority here?")
    });
    let path_and_query = incoming.path_with_query().map(|path_and_query| {
        PathAndQuery::from_maybe_shared(path_and_query)
            .expect("TODO: what shall we do with an invalid uri path-and-query here?")
    });

    // TODO: What's the right error code to use for invalid headers?
    let kind = BodyKind::from_headers(&headers)
        .map_err(|e| WasiHttpErrorCode::InternalError(Some(e.to_string())))?;
    // `body_stream` is a child of `incoming_body` which means we cannot
    // drop the parent before we drop the child
    let incoming_body = incoming
        .consume()
        .expect("cannot call `consume` twice on incoming request");
    let body_stream = incoming_body
        .stream()
        .expect("cannot call `stream` twice on an incoming body");
    let body_stream = AsyncInputStream::new(body_stream);

    let body = IncomingBody::new(kind, body_stream, incoming_body);

    let mut uri = Uri::builder();
    if let Some(scheme) = scheme {
        uri = uri.scheme(scheme);
    }
    if let Some(authority) = authority {
        uri = uri.authority(authority);
    }
    if let Some(path_and_query) = path_and_query {
        uri = uri.path_and_query(path_and_query);
    }
    // TODO: What's the right error code to use for an invalid uri?
    let uri = uri
        .build()
        .map_err(|e| WasiHttpErrorCode::InternalError(Some(e.to_string())))?;

    let mut request = Request::builder().method(method).uri(uri);
    if let Some(headers_mut) = request.headers_mut() {
        *headers_mut = headers;
    }
    // TODO: What's the right error code to use for an invalid request?
    request
        .body(body)
        .map_err(|e| WasiHttpErrorCode::InternalError(Some(e.to_string())))
}
