use anyhow::{Context, Result};
use futures_lite::stream::once_future;
use http_body_util::{BodyExt, StreamBody};
use wstd::http::body::{Body, Bytes, Frame};
use wstd::http::{Error, HeaderMap, Request, Response, StatusCode};
use wstd::time::{Duration, Instant};

#[wstd::http_server]
async fn main(request: Request<Body>) -> Result<Response<Body>, Error> {
    let path = request.uri().path_and_query().unwrap().as_str();
    println!("serving {path}");
    match path {
        "/" => http_home(request).await,
        "/wait-response" => http_wait_response(request).await,
        "/wait-body" => http_wait_body(request).await,
        "/echo" => http_echo(request).await,
        "/echo-headers" => http_echo_headers(request).await,
        "/echo-trailers" => http_echo_trailers(request).await,
        "/response-status" => http_response_status(request).await,
        "/response-fail" => http_response_fail(request).await,
        "/response-body-fail" => http_body_fail(request).await,
        _ => http_not_found(request).await,
    }
}

async fn http_home(_request: Request<Body>) -> Result<Response<Body>> {
    // To send a single string as the response body, use `Responder::respond`.
    Ok(Response::new(
        "Hello, wasi:http/proxy world!\n".to_owned().into(),
    ))
}

async fn http_wait_response(_request: Request<Body>) -> Result<Response<Body>> {
    // Get the time now
    let now = Instant::now();

    // Sleep for one second.
    wstd::task::sleep(Duration::from_secs(1)).await;

    // Compute how long we slept for.
    let elapsed = Instant::now().duration_since(now).as_millis();

    Ok(Response::new(
        format!("slept for {elapsed} millis\n").into(),
    ))
}

async fn http_wait_body(_request: Request<Body>) -> Result<Response<Body>> {
    // Get the time now
    let now = Instant::now();

    let body = StreamBody::new(once_future(async move {
        // Sleep for one second.
        wstd::task::sleep(Duration::from_secs(1)).await;

        // Compute how long we slept for.
        let elapsed = Instant::now().duration_since(now).as_millis();
        anyhow::Ok(Frame::data(Bytes::from(format!(
            "slept for {elapsed} millis\n"
        ))))
    }));

    Ok(Response::new(body.into()))
}

async fn http_echo(request: Request<Body>) -> Result<Response<Body>> {
    let (_parts, body) = request.into_parts();
    Ok(Response::new(body))
}

async fn http_echo_headers(request: Request<Body>) -> Result<Response<Body>> {
    let mut response = Response::builder();
    *response.headers_mut().unwrap() = request.into_parts().0.headers;
    Ok(response.body("".to_owned().into())?)
}

async fn http_echo_trailers(request: Request<Body>) -> Result<Response<Body>> {
    let collected = request.into_body().into_boxed_body().collect().await?;
    let trailers = collected.trailers().cloned().unwrap_or_else(|| {
        let mut trailers = HeaderMap::new();
        trailers.insert("x-no-trailers", "1".parse().unwrap());
        trailers
    });

    let body = StreamBody::new(once_future(async move {
        anyhow::Ok(Frame::<Bytes>::trailers(trailers))
    }));
    Ok(Response::new(body.into()))
}

async fn http_response_status(request: Request<Body>) -> Result<Response<Body>> {
    let status = if let Some(header_val) = request.headers().get("x-response-status") {
        header_val
            .to_str()
            .context("contents of x-response-status")?
            .parse::<u16>()
            .context("u16 value from x-response-status")?
    } else {
        500
    };
    Ok(Response::builder()
        .status(status)
        .body(String::new().into())?)
}

async fn http_response_fail(_request: Request<Body>) -> Result<Response<Body>> {
    Err(anyhow::anyhow!("error creating response"))
}

async fn http_body_fail(_request: Request<Body>) -> Result<Response<Body>> {
    let body = StreamBody::new(once_future(async move {
        Err::<Frame<Bytes>, _>(anyhow::anyhow!("error creating body"))
    }));

    Ok(Response::new(body.into()))
}

async fn http_not_found(_request: Request<Body>) -> Result<Response<Body>> {
    let response = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap();
    Ok(response)
}
