//! Run with
//!
//! ```sh
//! cargo build -p wstd-axum --examples --target wasm32-wasip2
//! wasmtime serve -Scli target/wasm32-wasip2/debug/examples/hello-world-nomacro.wasm
//! ```

use axum::{Router, response::Html, routing::get};
use wstd::http::{Body, Error, Request, Response};

#[wstd::http_server]
async fn main(request: Request<Body>) -> Result<Response<Body>, Error> {
    let service = Router::new().route("/", get(handler));
    wstd_axum::serve(request, service).await
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}
