//! HTTP networking support
//!
pub use http::status::StatusCode;
pub use http::uri::{Authority, PathAndQuery, Uri};

#[doc(inline)]
pub use body::{Body, util::BodyExt};
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

// Conditionally-compiled declarative macro for HTTP server export
//
// The `#[wstd::http_server]` proc macro delegates to this declarative macro.
// Because `#[macro_export]` macros are compiled in wstd's context, the `cfg`
// checks here use wstd's own feature flags so consumers don't need to define
// `wasip2`/`wasip3` features themselves.

#[cfg(all(feature = "wasip2", not(feature = "wasip3")))]
#[macro_export]
#[doc(hidden)]
macro_rules! __http_server_export {
    (@async { $($run_fn:tt)* }) => {
        const _: () = {
            struct TheServer;

            impl $crate::__internal::wasip2::exports::http::incoming_handler::Guest for TheServer {
                fn handle(
                    wasi_request: $crate::__internal::wasip2::http::types::IncomingRequest,
                    response_out: $crate::__internal::wasip2::http::types::ResponseOutparam
                ) {
                    $($run_fn)*

                    let responder = $crate::http::server::Responder::new(response_out);
                    $crate::runtime::block_on(async move {
                        match $crate::http::request::try_from_incoming(wasi_request) {
                            ::core::result::Result::Ok(request) => match __run(request).await {
                                ::core::result::Result::Ok(response) => { responder.respond(response).await.unwrap() },
                                ::core::result::Result::Err(err) => responder.fail(err),
                            }
                            ::core::result::Result::Err(err) => responder.fail(err),
                        }
                    })
                }
            }

            $crate::__internal::wasip2::http::proxy::export!(TheServer with_types_in $crate::__internal::wasip2);
        };
    };
    (@sync { $($run_fn:tt)* }) => {
        const _: () = {
            struct TheServer;

            impl $crate::__internal::wasip2::exports::http::incoming_handler::Guest for TheServer {
                fn handle(
                    wasi_request: $crate::__internal::wasip2::http::types::IncomingRequest,
                    response_out: $crate::__internal::wasip2::http::types::ResponseOutparam
                ) {
                    $($run_fn)*

                    let responder = $crate::http::server::Responder::new(response_out);
                    $crate::runtime::block_on(async move {
                        match $crate::http::request::try_from_incoming(wasi_request) {
                            ::core::result::Result::Ok(request) => match __run(request) {
                                ::core::result::Result::Ok(response) => { responder.respond(response).await.unwrap() },
                                ::core::result::Result::Err(err) => responder.fail(err),
                            }
                            ::core::result::Result::Err(err) => responder.fail(err),
                        }
                    })
                }
            }

            $crate::__internal::wasip2::http::proxy::export!(TheServer with_types_in $crate::__internal::wasip2);
        };
    };
}

#[cfg(feature = "wasip3")]
#[macro_export]
#[doc(hidden)]
macro_rules! __http_server_export {
    (@async { $($run_fn:tt)* }) => {
        const _: () = {
            struct TheServer;

            impl $crate::__internal::wasip3::exports::http::handler::Guest for TheServer {
                async fn handle(
                    wasi_request: $crate::__internal::wasip3::http::types::Request,
                ) -> ::core::result::Result<
                    $crate::__internal::wasip3::http::types::Response,
                    $crate::__internal::wasip3::http::types::ErrorCode,
                > {
                    $($run_fn)*

                    let (_writer, completion_reader) = $crate::__internal::wasip3::wit_future::new::<
                        ::core::result::Result<(), $crate::__internal::wasip3::http::types::ErrorCode>,
                    >(|| ::core::result::Result::Ok(()));
                    ::core::mem::drop(_writer);

                    let request = $crate::http::request::try_from_wasi_request(wasi_request, completion_reader)
                        .map_err($crate::http::server::error_to_wasi)?;

                    let response = __run(request).await
                        .map_err($crate::http::server::error_to_wasi)?;

                    $crate::http::server::response_to_wasi(response).await
                }
            }

            $crate::__internal::wasip3::http::service::export!(TheServer with_types_in $crate::__internal::wasip3);
        };
    };
    (@sync { $($run_fn:tt)* }) => {
        const _: () = {
            struct TheServer;

            impl $crate::__internal::wasip3::exports::http::handler::Guest for TheServer {
                async fn handle(
                    wasi_request: $crate::__internal::wasip3::http::types::Request,
                ) -> ::core::result::Result<
                    $crate::__internal::wasip3::http::types::Response,
                    $crate::__internal::wasip3::http::types::ErrorCode,
                > {
                    $($run_fn)*

                    let (_writer, completion_reader) = $crate::__internal::wasip3::wit_future::new::<
                        ::core::result::Result<(), $crate::__internal::wasip3::http::types::ErrorCode>,
                    >(|| ::core::result::Result::Ok(()));
                    ::core::mem::drop(_writer);

                    let request = $crate::http::request::try_from_wasi_request(wasi_request, completion_reader)
                        .map_err($crate::http::server::error_to_wasi)?;

                    let response = __run(request)
                        .map_err($crate::http::server::error_to_wasi)?;

                    $crate::http::server::response_to_wasi(response).await
                }
            }

            $crate::__internal::wasip3::http::service::export!(TheServer with_types_in $crate::__internal::wasip3);
        };
    };
}
