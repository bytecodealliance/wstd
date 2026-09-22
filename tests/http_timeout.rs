use wstd::future::FutureExt;
use wstd::http::{Body, Client, Request};
use wstd::time::Duration;

#[wstd::test]
async fn http_timeout() -> Result<(), Box<dyn std::error::Error>> {
    // This get request will connect to the server, which will then wait 1 second before
    // returning a response.
    let request = Request::get("https://postman-echo.com/delay/1").body(Body::empty())?;
    let result = Client::new()
        .send(request)
        .timeout(Duration::from_millis(500))
        .await;

    assert!(result.is_err(), "response should be an error");
    let error = result.unwrap_err();
    assert!(
        matches!(error.kind(), std::io::ErrorKind::TimedOut),
        "expected TimedOut error, got: {error:?>}"
    );

    Ok(())
}
