use std::error::Error;
use wstd::http::{Body, Client, HeaderValue, Request};

#[wstd::test]
async fn main() -> Result<(), Box<dyn Error>> {
    let request = Request::post("https://postman-echo.com/post")
        .header(
            "content-type",
            HeaderValue::from_str("application/json; charset=utf-8")?,
        )
        .body(Body::from_string("{\"test\": \"data\"}"))?;

    let response = Client::new().send(request).await?;

    let content_type = response
        .headers()
        .get("Content-Type")
        .ok_or("response expected to have Content-Type header")?;
    assert_eq!(content_type, "application/json; charset=utf-8");

    let mut body = response.into_body().into_body();
    let body_buf = body.contents().await?;

    let val: serde_json::Value = serde_json::from_slice(body_buf)?;
    let body_url = val
        .get("url")
        .ok_or("body json has url")?
        .as_str()
        .ok_or("body json url is str")?;
    assert!(
        body_url.contains("postman-echo.com/post"),
        "expected body url to contain the authority and path, got: {body_url}"
    );

    let posted_json = val
        .get("json")
        .ok_or("body json has 'json' key")?
        .as_object()
        .ok_or_else(|| format!("body json 'json' is object. got {val:?}"))?;

    assert_eq!(posted_json.len(), 1);
    assert_eq!(
        posted_json
            .get("test")
            .ok_or("returned json has 'test' key")?
            .as_str()
            .ok_or("returned json 'test' key should be str value")?,
        "data"
    );

    Ok(())
}
