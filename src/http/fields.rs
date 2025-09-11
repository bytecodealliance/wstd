pub use http::header::{HeaderMap, HeaderName, HeaderValue};

use super::Error;
use wasip2::http::types::{Fields, HeaderError as WasiHttpHeaderError};

pub(crate) fn header_map_from_wasi(wasi_fields: Fields) -> Result<HeaderMap, Error> {
    let mut output = HeaderMap::new();
    for (key, value) in wasi_fields.entries() {
        let key = HeaderName::from_bytes(key.as_bytes())
            .map_err(|e| Error::from(e).context("header name {key}"))?;
        let value = HeaderValue::from_bytes(&value)
            .map_err(|e| Error::from(e).context("header value for {key}"))?;
        output.append(key, value);
    }
    Ok(output)
}

pub(crate) fn header_map_to_wasi(header_map: &HeaderMap) -> Result<Fields, ToWasiHeaderError> {
    let wasi_fields = Fields::new();
    for (key, value) in header_map {
        // Unwrap because `HeaderMap` has already validated the headers.
        wasi_fields
            .append(key.as_str(), value.as_bytes())
            .map_err(|error| ToWasiHeaderError {
                error,
                context: format!("header {key}: {value:?}"),
            })?;
    }
    Ok(wasi_fields)
}

#[derive(Debug)]
pub(crate) struct ToWasiHeaderError {
    pub(crate) error: WasiHttpHeaderError,
    pub(crate) context: String,
}
