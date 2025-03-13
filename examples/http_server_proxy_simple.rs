// Run the example with:
// cargo build --example http_server_proxy_simple --target=wasm32-wasip2
// wasmtime serve -Scli -Shttp --env TARGET_URL=https://example.com target/wasm32-wasip2/debug/examples/http_server_proxy_simple.wasm
// Test with `curl -v 127.0.0.1:8080`
use wstd::http::body::IncomingBody;
use wstd::http::server::{Finished, Responder};
use wstd::http::{Client, Request, Response, Uri};

#[wstd::http_server]
async fn main(server_req: Request<IncomingBody>, responder: Responder) -> Finished {
    let api_prefixed_path = server_req.uri().path_and_query().unwrap().as_str();
    let target_url = std::env::var("TARGET_URL").expect("missing environment variable TARGET_URL");
    let target_url: Uri = format!("{target_url}{}", api_prefixed_path)
        .parse()
        .expect("final target url should be parseable");
    println!("Proxying to {target_url}");

    let client = Client::new();
    let mut client_req = Request::builder();
    client_req = client_req.uri(target_url).method(server_req.method());

    // Copy headers from server request to the client request.
    let (server_req_parts, server_req_body) = server_req.into_parts();
    *client_req.headers_mut().unwrap() = server_req_parts.headers;
    // Send the whole request.
    let client_req = client_req
        .body(server_req_body)
        .expect("client_req.body failed");

    let client_resp: Response<IncomingBody> =
        client.send(client_req).await.expect("client.send failed");
    let mut server_resp = Response::builder();
    let (client_resp_parts, client_resp_body) = client_resp.into_parts();
    *server_resp.headers_mut().unwrap() = client_resp_parts.headers;
    // Send the response.
    let server_resp = server_resp
        .body(client_resp_body)
        .expect("server_resp.body failed");
    responder.respond(server_resp).await
}
