// Run the example with:
// cargo build --example http_server_proxy --target=wasm32-wasip2
// wasmtime serve -Scli -Shttp --env TARGET_URL=https://example.com/ target/wasm32-wasip2/debug/examples/http_server_proxy.wasm
// Test with `curl --no-buffer -v 127.0.0.1:8080/proxy/`

use wstd::http::body::Body;
use wstd::http::{Client, Error, Request, Response, StatusCode, Uri};

const PROXY_PREFIX: &str = "/proxy/";

#[wstd::http_server]
async fn main(server_req: Request<Body>) -> Result<Response<Body>, Error> {
    match server_req.uri().path_and_query().unwrap().as_str() {
        api_prefixed_path if api_prefixed_path.starts_with(PROXY_PREFIX) => {
            // Remove PROXY_PREFIX
            let target_url =
                std::env::var("TARGET_URL").expect("missing environment variable TARGET_URL");
            let target_url: Uri = format!(
                "{target_url}{}",
                api_prefixed_path
                    .strip_prefix(PROXY_PREFIX)
                    .expect("checked above")
            )
            .parse()
            .expect("final target url should be parseable");
            println!("Proxying to {target_url}");
            proxy(server_req, target_url).await
        }
        _ => Ok(http_not_found(server_req)),
    }
}

async fn proxy(server_req: Request<Body>, target_url: Uri) -> Result<Response<Body>, Error> {
    let client = Client::new();
    let mut client_req = Request::builder();
    client_req = client_req.uri(target_url).method(server_req.method());

    // Copy headers from `server_req` to the `client_req`.
    for (key, value) in server_req.headers() {
        client_req = client_req.header(key, value);
    }

    // Stream the request body.
    let client_body = Body::from_http_body(server_req.into_body().into_boxed_body());
    let client_req = client_req.body(client_body)?;
    // Send the request.
    let client_resp = client.send(client_req).await?;
    // Copy headers from `client_resp` to `server_resp`.
    let mut server_resp = Response::builder();
    for (key, value) in client_resp.headers() {
        server_resp
            .headers_mut()
            .expect("no errors could be in ResponseBuilder")
            .append(key, value.clone());
    }
    let resp_body = server_resp
        .body(client_resp.into_body().into_boxed_body())
        .map_err(Error::from)?;
    let (resp_parts, resp_body) = resp_body.into_parts();
    let resp_body = Body::from_http_body(resp_body);
    Ok(Response::from_parts(resp_parts, resp_body))
}

fn http_not_found(_request: Request<Body>) -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap()
}
