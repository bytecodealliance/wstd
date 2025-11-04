use anyhow::Result;

#[test_log::test]
fn http_server_proxy() -> Result<()> {
    // Run wasmtime serve for the proxy and the target HTTP server.
    let _serve_target = test_programs::WasmtimeServe::new(test_programs::HTTP_SERVER)?;
    let _serve_proxy = test_programs::WasmtimeServe::new_with_config(
        test_programs::HTTP_SERVER_PROXY,
        8082,
        &["TARGET_URL=http://127.0.0.1:8081"],
    )?;

    // TEST / of the `http_server` example through the proxy
    let body: String = ureq::get("http://127.0.0.1:8082/proxy/")
        .call()?
        .body_mut()
        .read_to_string()?;
    assert_eq!(body, "Hello, wasi:http/proxy world!\n");
    Ok(())
}
