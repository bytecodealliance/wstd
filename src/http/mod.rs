//! HTTP networking support
//!
pub use http::Method;
pub use http::header::{HeaderMap, HeaderName, HeaderValue};
pub use http::status::StatusCode;
pub use http::uri::{Authority, InvalidUri, PathAndQuery, Scheme, Uri};

#[doc(inline)]
pub use body::{Body, util::BodyExt};
pub use client::Client;
pub use error::{Error, ErrorCode, Result};
pub use request::Request;
pub use response::Response;

pub mod body {
    mod sys {
        #[cfg(target_env = "p2")]
        pub(super) mod p2;
        #[cfg(target_env = "p3")]
        pub(super) mod p3;
    }
    #[cfg(target_env = "p2")]
    pub use sys::p2::*;
    #[cfg(target_env = "p3")]
    pub use sys::p3::*;
}

pub mod request {
    mod sys {
        #[cfg(target_env = "p2")]
        pub(super) mod p2;
        #[cfg(target_env = "p3")]
        pub(super) mod p3;
    }
    #[cfg(target_env = "p2")]
    pub use sys::p2::*;
    #[cfg(target_env = "p3")]
    pub use sys::p3::*;
}

pub mod response {
    mod sys {
        #[cfg(target_env = "p2")]
        pub(super) mod p2;
        #[cfg(target_env = "p3")]
        pub(super) mod p3;
    }
    #[cfg(target_env = "p2")]
    pub use sys::p2::*;
    #[cfg(target_env = "p3")]
    pub use sys::p3::*;
}

mod client;
pub mod error;
#[cfg(target_env = "p2")]
mod fields;
#[cfg(target_env = "p2")]
mod method;
#[cfg(target_env = "p2")]
mod scheme;
#[cfg(target_env = "p2")]
pub mod server;
