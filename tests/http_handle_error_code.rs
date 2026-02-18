use wstd::http::{Body, Client, Request, error::ErrorCode};

/// Test that `outgoing_handler::handle` errors are properly propagated.
#[wstd::test]
async fn handle_returns_error_code() -> Result<(), Box<dyn std::error::Error>> {
    let request = Request::get("ftp://example.com/").body(Body::empty())?;

    let result = Client::new().send(request).await;

    assert!(
        result.is_err(),
        "request with unsupported scheme should fail"
    );
    let error = result.unwrap_err();
    assert!(
        error.downcast_ref::<ErrorCode>().is_some(),
        "expected an ErrorCode, got: {error:?}"
    );

    Ok(())
}
