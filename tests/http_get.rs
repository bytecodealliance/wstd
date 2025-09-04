use std::error::Error;
use wstd::http::{Body, Client, HeaderValue, Request};

#[wstd::test]
async fn main() -> Result<(), Box<dyn Error>> {
    let request = Request::get("https://postman-echo.com/get")
        .header("my-header", HeaderValue::from_str("my-value")?)
        .body(Body::empty())?;

    let response = Client::new().send(request).await?;

    let content_type = response
        .headers()
        .get("Content-Type")
        .ok_or("response expected to have Content-Type header")?;
    assert_eq!(content_type, "application/json; charset=utf-8");

    let mut body = response.into_body().into_body();
    let body_len = body
        .content_length()
        .ok_or("GET postman-echo.com/get is supposed to provide a content-length")?;

    let contents = body.contents().await?;

    assert_eq!(
        contents.len() as u64,
        body_len,
        "contents length should match content-length"
    );

    let val: serde_json::Value = serde_json::from_slice(contents)?;
    let body_url = val
        .get("url")
        .ok_or("body json has url")?
        .as_str()
        .ok_or("body json url is str")?;
    assert!(
        body_url.contains("postman-echo.com/get"),
        "expected body url to contain the authority and path, got: {body_url}"
    );

    assert_eq!(
        val.get("headers")
            .ok_or("body json has headers")?
            .get("my-header")
            .ok_or("headers contains my-header")?
            .as_str()
            .ok_or("my-header is a str")?,
        "my-value"
    );

    Ok(())
}
