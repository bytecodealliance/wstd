//! Run with
//!
//! ```not_rust
//! cargo build -p example-hello-world --target wasm32-wasip2
//! wasmtime serve -Scli target/wasm32-wasip2/debug/example-hello-world.wasm
//! ```

use axum::{response::Html, routing::get, Router};

#[wstd_axum::http_server]
fn main() -> Router {
    // build our application with a route
    Router::new().route("/", get(handler))
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}
