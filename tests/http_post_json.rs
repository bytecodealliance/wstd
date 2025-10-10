use serde::{Deserialize, Serialize};
use std::error::Error;
use wstd::http::{Body, Client, HeaderValue, Request};

#[derive(Serialize)]
struct TestData {
    test: String,
}

#[derive(Deserialize)]
struct Echo {
    url: String,
}

#[wstd::test]
async fn main() -> Result<(), Box<dyn Error>> {
    let test_data = TestData {
        test: "data".to_string(),
    };
    let mut request =
        Request::post("https://postman-echo.com/post").body(Body::from_json(&test_data)?)?;

    request.headers_mut().insert(
        "Content-Type",
        HeaderValue::from_static("application/json; charset=utf-8"),
    );

    let response = Client::new().send(request).await?;

    let content_type = response
        .headers()
        .get("Content-Type")
        .ok_or("response expected to have Content-Type header")?;
    assert_eq!(content_type, "application/json; charset=utf-8");

    let Echo { url } = response.into_body().json::<Echo>().await?;
    assert!(
        url.contains("postman-echo.com/post"),
        "expected body url to contain the authority and path, got: {url}"
    );

    Ok(())
}
