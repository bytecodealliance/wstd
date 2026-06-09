//! The http portion of wstd uses `anyhow::Error` as its `Error` type.
//!
//! There are various concrete error types

pub use crate::http::body::InvalidContentLength;
pub use anyhow::Context;
pub use http::header::{InvalidHeaderName, InvalidHeaderValue};
pub use http::method::InvalidMethod;
pub use wasip2::http::types::{ErrorCode, HeaderError};

pub type Error = anyhow::Error;
/// The `http` result type.
pub type Result<T> = std::result::Result<T, Error>;
