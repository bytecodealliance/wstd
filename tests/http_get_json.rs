use serde::Deserialize;
use std::error::Error;
use wstd::http::{Body, Client, Request};

#[derive(Deserialize)]
struct Echo {
    url: String,
}

#[wstd::test]
async fn main() -> Result<(), Box<dyn Error>> {
    let request = Request::get("https://postman-echo.com/get").body(Body::empty())?;

    let response = Client::new().send(request).await?;

    let content_type = response
        .headers()
        .get("Content-Type")
        .ok_or("response expected to have Content-Type header")?;
    assert_eq!(content_type, "application/json; charset=utf-8");

    let Echo { url } = response.into_body().json::<Echo>().await?;
    assert!(
        url.contains("postman-echo.com/get"),
        "expected body url to contain the authority and path, got: {url}"
    );

    Ok(())
}
