use anyhow::Result;
use wstd::http::body::{BodyForthcoming, BoxBody, IncomingBody, OutgoingBody};
use wstd::http::{Body, IntoBody, Request, Response, StatusCode};
use wstd::io::{copy, empty, AsyncWrite};
use wstd::time::{Duration, Instant};

#[wstd::http_server]
async fn main(request: Request<IncomingBody>) -> Result<Response<BoxBody>> {
    match request.uri().path_and_query().unwrap().as_str() {
        /*
        "/wait" => http_wait(request, responder).await,
        "/echo" => http_echo(request, responder).await,
        "/echo-headers" => http_echo_headers(request, responder).await,
        "/echo-trailers" => http_echo_trailers(request, responder).await,
        "/fail" => http_fail(request, responder).await,
        "/bigfail" => http_bigfail(request, responder).await,
        */
        "/" => http_home(request).await,
        _ => http_not_found(request).await,
    }
}

async fn http_home(_request: Request<IncomingBody>) -> Result<Response<BoxBody>> {
    // To send a single string as the response body, use `Responder::respond`.
    Ok(Response::new(
        "Hello, wasi:http/proxy world!\n".into_boxed_body(),
    ))
}

/*
async fn http_wait(_request: Request<IncomingBody>, responder: Responder) -> Finished {
    // Get the time now
    let now = Instant::now();

    // Sleep for one second.
    wstd::task::sleep(Duration::from_secs(1)).await;

    // Compute how long we slept for.
    let elapsed = Instant::now().duration_since(now).as_millis();

    // To stream data to the response body, use `Responder::start_response`.
    let mut body = responder.start_response(Response::new(BodyForthcoming));
    let result = body
        .write_all(format!("slept for {elapsed} millis\n").as_bytes())
        .await;
    Finished::finish(body, result, None)
}

async fn http_echo(mut request: Request<IncomingBody>, responder: Responder) -> Finished {
    // Stream data from the request body to the response body.
    let mut body = responder.start_response(Response::new(BodyForthcoming));
    let result = copy(request.body_mut(), &mut body).await;
    Finished::finish(body, result, None)
}

async fn http_fail(_request: Request<IncomingBody>, responder: Responder) -> Finished {
    let body = responder.start_response(Response::new(BodyForthcoming));
    Finished::fail(body)
}

async fn http_bigfail(_request: Request<IncomingBody>, responder: Responder) -> Finished {
    async fn write_body(body: &mut OutgoingBody) -> wstd::io::Result<()> {
        for _ in 0..0x10 {
            body.write_all("big big big big\n".as_bytes()).await?;
        }
        body.flush().await?;
        Ok(())
    }

    let mut body = responder.start_response(Response::new(BodyForthcoming));
    let _ = write_body(&mut body).await;
    Finished::fail(body)
}

async fn http_echo_headers(request: Request<IncomingBody>, responder: Responder) -> Finished {
    let mut response = Response::builder();
    *response.headers_mut().unwrap() = request.into_parts().0.headers;
    let response = response.body(empty()).unwrap();
    responder.respond(response).await
}

async fn http_echo_trailers(request: Request<IncomingBody>, responder: Responder) -> Finished {
    let body = responder.start_response(Response::new(BodyForthcoming));
    let (trailers, result) = match request.into_body().finish().await {
        Ok(trailers) => (trailers, Ok(())),
        Err(err) => (Default::default(), Err(std::io::Error::other(err))),
    };
    Finished::finish(body, result, trailers)
}
*/

async fn http_not_found(_request: Request<IncomingBody>) -> Result<Response<BoxBody>> {
    let response = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(empty().into_boxed_body())
        .unwrap();
    Ok(response)
}
