use anyhow::Result;

#[test_log::test]
fn http_server_proxy() -> Result<()> {
    run(test_programs::HTTP_SERVER, test_programs::HTTP_SERVER_PROXY)
}

#[cfg(wstd_nightly)]
#[test_log::test]
fn http_server_proxy_p3() -> Result<()> {
    run(
        test_programs::HTTP_SERVER_P3,
        test_programs::HTTP_SERVER_PROXY_P3,
    )
}

fn run(server: &str, proxy: &str) -> Result<()> {
    // Run wasmtime serve for the proxy and the target HTTP server.
    let serve_target = test_programs::WasmtimeServe::new(server)?;
    let addr = serve_target.get_listening_address();
    let env_var = format!("TARGET_URL=http://{addr}");
    let serve_proxy = test_programs::WasmtimeServe::new_with_config(proxy, &[env_var.as_str()])?;
    let addr = serve_proxy.get_listening_address();

    // TEST / of the `http_server` example through the proxy
    let body: String = ureq::get(format!("http://{addr}/proxy/"))
        .call()?
        .body_mut()
        .read_to_string()?;
    assert_eq!(body, "Hello, wasi:http/proxy world!\n");
    Ok(())
}
