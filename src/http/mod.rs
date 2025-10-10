//! HTTP networking support
//!
pub use http::status::StatusCode;
pub use http::uri::{Authority, PathAndQuery, Uri};

#[doc(inline)]
pub use body::{util::BodyExt, Body};
pub use client::Client;
pub use error::{Error, ErrorCode, Result};
pub use fields::{HeaderMap, HeaderName, HeaderValue};
pub use method::Method;
pub use request::Request;
pub use response::Response;
pub use scheme::{InvalidUri, Scheme};

pub mod body;

mod client;
pub mod error;
mod fields;
mod method;
pub mod request;
pub mod response;
mod scheme;
pub mod server;
