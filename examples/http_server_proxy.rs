// Run the example with:
// cargo build --example http_server_proxy --target=wasm32-wasip2
// wasmtime serve -Scli -Shttp --env TARGET_URL=https://example.com/ target/wasm32-wasip2/debug/examples/http_server_proxy.wasm
// Test with `curl --no-buffer -v 127.0.0.1:8080/proxy/`
use futures_concurrency::prelude::*;
use wstd::http::body::{BodyForthcoming, IncomingBody};
use wstd::http::server::{Finished, Responder};
use wstd::http::{Client, Request, Response, StatusCode, Uri};
use wstd::io::{copy, empty};

const PROXY_PREFIX: &str = "/proxy/";

#[wstd::http_server]
async fn main(mut server_req: Request<IncomingBody>, responder: Responder) -> Finished {
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

            let client = Client::new();
            let mut client_req = Request::builder();
            client_req = client_req.uri(target_url).method(server_req.method());

            // Copy headers from server request to the client request.
            for (key, value) in server_req.headers() {
                client_req = client_req.header(key, value);
            }

            // Send the request.
            let client_req = client_req
                .body(BodyForthcoming)
                .expect("client_req.body failed");
            let (mut client_request_body, client_resp) = client
                .start_request(client_req)
                .await
                .expect("client.start_request failed");

            // Copy the server request body to client's request body.
            let server_req_to_client_req = async {
                let res = copy(server_req.body_mut(), &mut client_request_body).await;
                Client::finish(client_request_body, None)
                    .map_err(|_http_err| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Failed to read HTTP request body",
                        )
                    })
                    .and(res)
            };

            // Copy the client response headers to server response.
            let client_resp_to_server_resp = async {
                let client_resp = client_resp.await.unwrap();
                let mut server_resp = Response::builder();
                for (key, value) in client_resp.headers() {
                    server_resp
                        .headers_mut()
                        .unwrap()
                        .append(key, value.clone());
                }
                // Start sending the server response.
                let server_resp = server_resp.body(BodyForthcoming).unwrap();
                let mut server_resp = responder.start_response(server_resp);

                (
                    copy(client_resp.into_body(), &mut server_resp).await,
                    server_resp,
                )
            };

            let (server_req_to_client_req, (client_resp_to_server_resp, server_resp)) =
                (server_req_to_client_req, client_resp_to_server_resp)
                    .join()
                    .await;
            let is_success = server_req_to_client_req.and(client_resp_to_server_resp);
            Finished::finish(server_resp, is_success, None)
        }
        _ => http_not_found(server_req, responder).await,
    }
}

async fn http_not_found(_request: Request<IncomingBody>, responder: Responder) -> Finished {
    let response = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(empty())
        .unwrap();
    responder.respond(response).await
}
