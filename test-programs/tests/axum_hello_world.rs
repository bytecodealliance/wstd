use anyhow::Result;

#[test_log::test]
fn hello_world() -> Result<()> {
    run(test_programs::axum::HELLO_WORLD)
}

#[cfg(wstd_nightly)]
#[test_log::test]
fn hello_world_p3() -> Result<()> {
    run(test_programs::axum::HELLO_WORLD_P3)
}

#[test_log::test]
fn hello_world_nomacro() -> Result<()> {
    run(test_programs::axum::HELLO_WORLD_NOMACRO)
}

#[cfg(wstd_nightly)]
#[test_log::test]
fn hello_world_nomacro_p3() -> Result<()> {
    run(test_programs::axum::HELLO_WORLD_NOMACRO_P3)
}

// The hello_world.rs and hello_world_nomacro.rs are identical in
// functionality
fn run(guest: &str) -> Result<()> {
    // Run wasmtime serve.
    let serve = test_programs::WasmtimeServe::new(guest)?;
    let addr = serve.get_listening_address();

    // Test each path in the server:

    // TEST / handler
    // Response body is the hard-coded default
    let body: String = ureq::get(format!("http://{}", addr))
        .call()?
        .body_mut()
        .read_to_string()?;
    assert!(body.contains("<h1>Hello, World!</h1>"));

    Ok(())
}
