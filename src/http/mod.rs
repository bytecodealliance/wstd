//! HTTP networking support
//!
pub use http::status::StatusCode;
pub use http::uri::{Authority, PathAndQuery, Uri};

#[doc(inline)]
pub use body::{Body, util::BodyExt};
pub use crate::sys::http::client::Client;
pub use error::{Error, ErrorCode, Result};
pub use crate::sys::http::fields::{HeaderMap, HeaderName, HeaderValue};
pub use crate::sys::http::method::Method;
pub use request::Request;
pub use response::Response;
pub use crate::sys::http::scheme::{InvalidUri, Scheme};

pub mod body;

pub mod error;
pub mod request;
pub mod response;
pub mod server;
